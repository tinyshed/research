use crate::feed::Feed;
use crate::packet::{Packet, Position};
use crate::stats::elapsed;
use crate::{Result, pages, require, wal};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tinystore::capture_experiment::{Observer, checkpoint};

#[derive(Clone, Debug)]
pub(super) struct Config {
    pub batched: bool,
    pub owned: bool,
    pub cache_bytes: usize,
    pub passive: bool,
    pub flush_bytes: usize,
    pub frames: u32,
    pub wal_limit: u64,
    pub reserve: u64,
    pub disk_limit: u64,
    pub log_full: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            batched: true,
            owned: true,
            cache_bytes: 8 * 1024 * 1024,
            passive: true,
            flush_bytes: 4 * 1024 * 1024,
            frames: 1024,
            wal_limit: 8 * 1024 * 1024,
            reserve: 1024 * 1024,
            disk_limit: 2 * 1024 * 1024 * 1024,
            log_full: false,
        }
    }
}
#[derive(Debug)]
pub(super) struct Source {
    pub root: PathBuf,
    pub config: Config,
    fault: Option<String>,
    files: Mutex<BTreeMap<PathBuf, Arc<Mutex<Feed>>>>,
}
impl Source {
    pub(super) fn new(root: &Path, config: Config) -> Result<Arc<Self>> {
        require(
            config.wal_limit > config.reserve && config.disk_limit > 2 * config.reserve,
            "invalid reserve",
        )?;
        std::fs::create_dir_all(root.join("feed"))?;
        std::fs::File::open(root)?.sync_all()?;
        if let Some(parent) = root.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(Arc::new(Self {
            root: root.canonicalize()?,
            config,
            fault: std::env::var("WAL_LOAD_FAULT").ok(),
            files: Mutex::default(),
        }))
    }
    pub(super) fn feed(&self, path: &Path) -> Result<Arc<Mutex<Feed>>> {
        self.files
            .lock()
            .map_err(|_| "source poisoned")?
            .get(path)
            .cloned()
            .ok_or_else(|| "unknown file".into())
    }
    pub(super) fn wal_bytes(path: &Path) -> u64 {
        let mut name = path.as_os_str().to_os_string();
        name.push("-wal");
        std::fs::metadata(name).map_or(0, |m| m.len())
    }
    fn point(&self, name: &str) {
        if self.fault.as_deref() == Some(name) {
            std::process::exit(86);
        }
    }
    fn opened(&self, path: &Path, connection: &Connection) -> Result<()> {
        require(
            connection.pragma_query_value::<u32, _>(None, "synchronous", |r| r.get(0))? == 2,
            "FULL required",
        )?;
        let relative = path.strip_prefix(&self.root)?;
        let name = Sha256::digest(relative.to_string_lossy().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut feed = Feed::open(
            &self.root.join("feed").join(format!("{name}.log")),
            self.config.cache_bytes,
            self.config.owned,
            self.config.batched,
        )?;
        if feed.sequence == 0 {
            let image = pages::Image::read(&mut pages::reader(path)?)?;
            let commits = wal::scan(path, wal::Cursor::default(), None)?;
            let cursor = commits.last().map_or(wal::Cursor::default(), |c| c.cursor);
            feed.append(
                Packet {
                    stream: "file".into(),
                    snapshot: true,
                    position: Position {
                        epoch: 1,
                        sequence: 1,
                        checksum: [0; 32],
                    },
                    previous: [0; 32],
                    page_size: image.page_size,
                    page_count: image.pages.len() as u32,
                    pages: image.pages,
                },
                cursor,
            )?;
        } else {
            self.recover(path, &mut feed, None)?;
        }
        let image = pages::Image::read(&mut pages::reader(path)?)?;
        let mut actual = crate::tree::Tree::default();
        actual.apply(&image.pages, image.pages.len() as u32, image.page_size)?;
        require(actual.root() == feed.tree.root(), "source history mismatch")?;
        feed.sync()?;
        feed.reset_stats();
        self.files
            .lock()
            .map_err(|_| "files poisoned")?
            .insert(path.to_owned(), Arc::new(Mutex::new(feed)));
        Ok(())
    }
    fn recover(&self, path: &Path, feed: &mut Feed, published: Option<u32>) -> Result<()> {
        let start = Instant::now();
        let commits = wal::scan(path, feed.cursor, published)?;
        feed.stats.wal_ns += elapsed(start);
        for commit in commits {
            feed.append(
                Packet {
                    stream: "file".into(),
                    snapshot: false,
                    position: Position {
                        epoch: 1,
                        sequence: feed.sequence + 1,
                        checksum: [0; 32],
                    },
                    previous: feed.tree.root(),
                    page_size: commit.page_size,
                    page_count: commit.count,
                    pages: commit.pages,
                },
                commit.cursor,
            )?;
            self.point("append");
        }
        Ok(())
    }
    fn checkpoint(
        &self,
        feed: &mut Feed,
        connection: &Connection,
        truncate: bool,
    ) -> Result<(i64, i64, i64)> {
        feed.sync()?;
        self.point("before_checkpoint");
        let start = Instant::now();
        // TRUNCATE at the hard watermark must not inherit a multi-second busy wait.
        let saved: i64 = connection.pragma_query_value(None, "busy_timeout", |r| r.get(0))?;
        if truncate && self.config.passive {
            connection.busy_timeout(std::time::Duration::ZERO)?;
        }
        let result = checkpoint(connection, truncate);
        if truncate && self.config.passive {
            connection.busy_timeout(std::time::Duration::from_millis(saved as u64))?;
        }
        feed.stats.checkpoint_ns += elapsed(start);
        feed.stats.checkpoints += 1;
        self.point("after_checkpoint");
        Ok(result?)
    }
    fn admit(&self, path: &Path, connection: &Connection) -> Result<()> {
        let handle = self.feed(path)?;
        let mut feed = handle.lock().map_err(|_| "feed poisoned")?;
        let start = Instant::now();
        let bytes = Self::wal_bytes(path);
        feed.stats.max_wal = feed.stats.max_wal.max(bytes);
        if feed.length + 2 * self.config.reserve > self.config.disk_limit {
            feed.stats.denied += 1;
            feed.stats.admission_ns += elapsed(start);
            return Err("feed disk admission limit".into());
        }
        if bytes + self.config.reserve > self.config.wal_limit {
            let _ = self.checkpoint(&mut feed, connection, true)?;
            if Self::wal_bytes(path) + self.config.reserve > self.config.wal_limit {
                feed.stats.denied += 1;
                feed.stats.admission_ns += elapsed(start);
                return Err("WAL admission limit; reader must advance".into());
            }
        }
        feed.stats.admission_ns += elapsed(start);
        Ok(())
    }
    fn committed(&self, path: &Path, connection: &Connection, frames: u32) -> Result<bool> {
        self.point("sqlite_commit");
        let start = Instant::now();
        let handle = self.feed(path)?;
        let mut feed = handle.lock().map_err(|_| "feed poisoned")?;
        let previous = feed.cursor;
        self.recover(path, &mut feed, Some(frames))?;
        let bytes = Self::wal_bytes(path);
        feed.stats.max_wal = feed.stats.max_wal.max(bytes);
        let added = if previous.salts == feed.cursor.salts {
            frames.saturating_sub(previous.frame)
        } else {
            frames
        };
        require(
            u64::from(added) * (u64::from(feed.tree.page_size) + 24) <= self.config.reserve,
            "transaction exceeded declared WAL reserve; reopen required",
        )?;
        if self.config.log_full {
            feed.sync()?;
            self.point("sync");
        }
        if previous.salts != feed.cursor.salts {
            feed.next_checkpoint = self.config.frames;
        }
        let due = if self.config.passive {
            frames >= feed.next_checkpoint
        } else {
            frames >= self.config.frames
        };
        if due || feed.dirty >= self.config.flush_bytes {
            feed.next_checkpoint = frames.saturating_add(self.config.frames);
            let _ = self.checkpoint(&mut feed, connection, !self.config.passive)?;
        }
        feed.stats.capture_ns += elapsed(start);
        Ok(false)
    }
}
fn error(e: Box<dyn std::error::Error + Send + Sync>) -> tinystore::Error {
    tinystore::Error::new(tinystore::ErrorKind::Unavailable, "WAL load capture")
        .with_source(std::io::Error::other(e))
}
impl Observer for Source {
    fn opened(&self, p: &Path, c: &Connection) -> tinystore::Result<()> {
        Source::opened(self, p, c).map_err(error)
    }
    fn before_write(&self, p: &Path, c: &Connection) -> tinystore::Result<()> {
        self.admit(p, c).map_err(error)
    }
    fn committed(&self, p: &Path, c: &Connection, n: u32) -> tinystore::Result<bool> {
        Source::committed(self, p, c, n).map_err(error)
    }
    fn closing(&self, p: &Path, _: &Connection) -> tinystore::Result<()> {
        self.feed(p)
            .and_then(|f| f.lock().map_err(|_| "feed poisoned")?.sync())
            .map_err(error)
    }
}
