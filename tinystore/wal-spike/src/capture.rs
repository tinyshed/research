use crate::feed::Feed;
use crate::packet::{Packet, Position};
use crate::{Result, pages, require, wal};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use tinystore::capture_experiment::Observer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    LogFull,
    WalGuarded,
}

#[derive(Debug)]
pub(super) struct Source {
    pub root: PathBuf,
    pub mode: Mode,
    files: Mutex<BTreeMap<PathBuf, Arc<Mutex<Feed>>>>,
    pub changed: Condvar,
    pub generation: Mutex<u64>,
    phase: Mutex<Option<String>>,
    pub armed: AtomicBool,
}

impl Source {
    pub(super) fn new(root: &Path, mode: Mode) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root.join("feed"))?;
        std::fs::File::open(root)?.sync_all()?;
        if let Some(parent) = root.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(Arc::new(Self {
            root: root.canonicalize()?,
            mode,
            files: Mutex::default(),
            changed: Condvar::new(),
            generation: Mutex::new(0),
            phase: Mutex::new(None),
            armed: AtomicBool::new(false),
        }))
    }

    pub(super) fn feed(&self, path: &Path) -> Result<Arc<Mutex<Feed>>> {
        self.files
            .lock()
            .map_err(|_| "source lock poisoned")?
            .get(path)
            .cloned()
            .ok_or_else(|| "unknown captured file".into())
    }

    #[cfg(test)]
    pub(super) fn arm(&self, phase: &str) {
        *self.phase.lock().expect("fault lock") = Some(phase.to_owned());
        self.armed.store(true, Ordering::SeqCst);
    }

    fn fault(&self, phase: &str) {
        if self.armed.load(Ordering::SeqCst)
            && self.phase.lock().expect("fault lock").as_deref() == Some(phase)
        {
            std::process::exit(86);
        }
    }

    fn opened_inner(&self, path: &Path) -> Result<()> {
        let relative = path.strip_prefix(&self.root)?;
        let name = Sha256::digest(relative.to_string_lossy().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut feed = Feed::open(&self.root.join("feed").join(format!("{name}.log")))?;
        if feed.entries.is_empty() {
            let image = pages::Image::read(&mut pages::reader(path)?)?;
            let commits = wal::scan(path, wal::Cursor::default(), None)?;
            let cursor = commits.last().map_or(wal::Cursor::default(), |c| c.cursor);
            let mut tree = crate::tree::Tree::default();
            tree.apply(&image.pages, image.pages.len() as u32, image.page_size)?;
            let packet = Packet {
                stream: "file".into(),
                snapshot: true,
                position: Position {
                    epoch: 1,
                    sequence: 1,
                    checksum: tree.root(),
                },
                previous: [0; 32],
                page_size: image.page_size,
                page_count: image.pages.len() as u32,
                pages: image.pages,
            };
            feed.append(packet, cursor)?;
            feed.sync()?;
        } else {
            self.recover(path, &mut feed, None)?;
            let image = pages::Image::read(&mut pages::reader(path)?)?;
            let mut actual = crate::tree::Tree::default();
            actual.apply(&image.pages, image.pages.len() as u32, image.page_size)?;
            require(
                actual.root() == feed.tree.root(),
                "source and retained history diverged; refusing writes",
            )?;
            feed.sync()?;
        }
        self.files
            .lock()
            .map_err(|_| "source lock poisoned")?
            .insert(path.to_owned(), Arc::new(Mutex::new(feed)));
        Ok(())
    }

    fn recover(&self, path: &Path, feed: &mut Feed, published: Option<u32>) -> Result<()> {
        for commit in wal::scan(path, feed.cursor, published)? {
            let previous = feed.tree.root();
            let sequence = feed
                .entries
                .last()
                .map_or(1, |e| e.packet.position.sequence + 1);
            let packet = Packet {
                stream: "file".into(),
                snapshot: false,
                position: Position {
                    epoch: 1,
                    sequence,
                    checksum: [0; 32],
                },
                previous,
                page_size: commit.page_size,
                page_count: commit.count,
                pages: commit.pages,
            };
            feed.append(packet, commit.cursor)?;
            self.fault("after_log_append");
            if self.mode == Mode::LogFull {
                feed.sync()?;
                self.fault("after_log_sync");
            }
        }
        Ok(())
    }

    fn committed_inner(&self, path: &Path, frames: u32) -> Result<bool> {
        self.fault("after_sqlite_commit");
        let handle = self.feed(path)?;
        let mut feed = handle.lock().map_err(|_| "feed lock poisoned")?;
        self.recover(path, &mut feed, Some(frames))?;
        let checkpoint = frames >= 1024 || feed.dirty >= 256 * 1024;
        if checkpoint {
            feed.sync()?;
            self.fault("before_checkpoint");
        }
        drop(feed);
        *self.generation.lock().map_err(|_| "generation poisoned")? += 1;
        self.changed.notify_all();
        Ok(checkpoint)
    }
}

fn core_error(error: Box<dyn std::error::Error + Send + Sync>) -> tinystore::Error {
    tinystore::Error::new(tinystore::ErrorKind::Unavailable, "WAL capture")
        .with_source(std::io::Error::other(error))
}

impl Observer for Source {
    fn checkpointed(&self, _path: &Path, _connection: &Connection) -> tinystore::Result<()> {
        self.fault("after_checkpoint");
        Ok(())
    }
    fn opened(&self, path: &Path, connection: &Connection) -> tinystore::Result<()> {
        let result = (|| -> Result<()> {
            let synchronous: u32 =
                connection.pragma_query_value(None, "synchronous", |row| row.get(0))?;
            require(synchronous == 2, "capture requires SQLite synchronous=FULL")?;
            self.opened_inner(path)
        })();
        result.map_err(core_error)
    }
    fn committed(
        &self,
        path: &Path,
        _connection: &Connection,
        frames: u32,
    ) -> tinystore::Result<bool> {
        self.committed_inner(path, frames).map_err(core_error)
    }
    fn closing(&self, path: &Path, _connection: &Connection) -> tinystore::Result<()> {
        let result =
            (|| -> Result<()> { self.feed(path)?.lock().map_err(|_| "feed poisoned")?.sync() })()
                .map_err(core_error);
        if result.is_ok() {
            self.fault("before_close_checkpoint");
        }
        result
    }
}
