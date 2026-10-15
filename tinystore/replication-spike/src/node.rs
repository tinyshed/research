use std::fs::{self, File, OpenOptions};
use std::path::{Component, Path, PathBuf};

use rusqlite::Connection;

use crate::journal::{Head, Journal};
use crate::packet::{Packet, Position, check_stream};
use crate::pages::{self, Image};
use crate::{Result, require};

pub(super) struct Node {
    root: PathBuf,
    journal: Journal,
    _lock: File,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Limits {
    pub entries: usize,
    pub bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: 32,
            bytes: 256 * 1024,
        }
    }
}

impl Node {
    pub(super) fn streams(&self) -> Result<Vec<String>> {
        let streams = self.journal.streams()?;
        for stream in &streams {
            self.journal.ready(stream)?;
        }
        Ok(streams)
    }

    pub(super) fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root.join("replicas"))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("REPLICATION_LOCK"))?;
        lock.try_lock()?;
        let journal = Journal::open(&root.join("replication.db"))?;
        let mut node = Self {
            root: root.to_owned(),
            journal,
            _lock: lock,
        };
        for bytes in node.journal.pending()? {
            node.apply_staged(&Packet::decode(&bytes)?, &bytes)?;
        }
        Ok(node)
    }

    pub(super) fn publish(&self, stream: &str, path: &str) -> Result<()> {
        check_stream(stream)?;
        let engine = ["kv.db", "jobs.db", "records.db", "metrics.db"].contains(&path);
        let database = path.starts_with("sql/") && path.ends_with(".db");
        require(
            Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
                && (engine || database),
            "publish expects a supported engine file or sql/<name>.db; blobs need object shipping",
        )?;
        require(
            self.root.join(path).is_file(),
            "publish source does not exist",
        )?;
        self.journal.publish(stream, path)
    }

    pub(super) fn position(&self, stream: &str) -> Result<Option<Position>> {
        check_stream(stream)?;
        self.journal.ready(stream)?;
        Ok(self.journal.head(stream)?.map(|head| head.position))
    }

    pub(super) fn capture(&mut self, stream: &str) -> Result<Option<Position>> {
        let head = self.head(stream)?;
        require(head.owned, "only the owner captures writes")?;
        let image = Image::read(&mut pages::reader(&self.root.join(&head.path))?)?;
        if image.checksum() == head.position.checksum {
            return Ok(None);
        }
        let old = self.journal.image(stream, head.page_size)?;
        let packet = Packet {
            stream: stream.to_owned(),
            snapshot: head.position.sequence == 0,
            position: Position {
                epoch: head.position.epoch,
                sequence: head.position.sequence + 1,
                checksum: image.checksum(),
            },
            previous: head.position.checksum,
            page_size: image.page_size,
            page_count: image.pages.len() as u32,
            pages: image
                .pages
                .into_iter()
                .filter(|(n, page)| old.pages.get(n) != Some(page))
                .collect(),
        };
        let bytes = packet.encode()?;
        self.journal.finish(&packet, &bytes, true)?;
        Ok(Some(packet.position))
    }

    pub(super) fn receive(&mut self, bytes: &[u8]) -> Result<Position> {
        let packet = Packet::decode(bytes)?;
        self.journal.ready(&packet.stream)?;
        if let Some(head) = self.journal.head(&packet.stream)? {
            require(
                !head.owned,
                "a primary refuses incoming changes for its own stream",
            )?;
            if packet.position.sequence <= head.position.sequence {
                let same_head = packet.position == head.position;
                let same_entry = self
                    .journal
                    .entry(&packet.stream, packet.position.sequence)?
                    .is_some_and(|kept| kept == bytes);
                require(same_head || same_entry, "conflicting or expired duplicate")?;
                return Ok(head.position);
            }
            require(
                packet.position.epoch >= head.position.epoch,
                "stale primary epoch",
            )?;
            if !packet.snapshot {
                require(
                    packet.position.sequence == head.position.sequence + 1
                        && packet.previous == head.position.checksum,
                    "stream gap or divergent predecessor: request a snapshot",
                )?;
            }
        } else {
            require(packet.snapshot, "new follower needs a snapshot")?;
        }
        self.expected_image(&packet)?;
        self.journal.stage(&packet, bytes)?;
        self.apply_staged(&packet, bytes)?;
        Ok(packet.position)
    }

    pub(super) fn changes(&self, stream: &str, after: u64, limits: Limits) -> Result<Vec<Vec<u8>>> {
        require(
            limits.entries > 0 && limits.bytes > 0,
            "batch limits must be positive",
        )?;
        let head = self.head(stream)?;
        require(
            after <= head.position.sequence,
            "consumer is ahead of this upstream",
        )?;
        let mut batch = Vec::new();
        let mut bytes = 0usize;
        for sequence in after + 1..=head.position.sequence {
            let Some(packet) = self.journal.entry(stream, sequence)? else {
                let snapshot = self.snapshot(stream)?.encode()?;
                require(
                    snapshot.len() <= limits.bytes,
                    "snapshot exceeds the batch byte limit",
                )?;
                return Ok(vec![snapshot]);
            };
            if batch.len() == limits.entries || bytes + packet.len() > limits.bytes {
                require(!batch.is_empty(), "next entry exceeds the batch byte limit")?;
                break;
            }
            bytes += packet.len();
            batch.push(packet);
        }
        Ok(batch)
    }

    pub(super) fn read<T>(
        &self,
        stream: &str,
        read: impl FnOnce(&Connection) -> Result<T>,
    ) -> Result<T> {
        let head = self.head(stream)?;
        read(&pages::reader(&self.root.join(head.path))?)
    }

    /// The caller must fence the former primary before invoking this.
    pub(super) fn promote(&mut self, stream: &str) -> Result<Position> {
        let head = self.head(stream)?;
        require(!head.owned, "stream is already primary")?;
        let image = Image::read(&mut pages::reader(&self.root.join(&head.path))?)?;
        require(
            image.checksum() == head.position.checksum,
            "cannot promote a damaged copy",
        )?;
        let mut packet = self.snapshot(stream)?;
        packet.snapshot = false;
        packet.previous = head.position.checksum;
        packet.position.epoch += 1;
        packet.position.sequence += 1;
        packet.pages.clear();
        let bytes = packet.encode()?;
        self.journal.finish(&packet, &bytes, true)?;
        Ok(packet.position)
    }

    pub(super) fn write(&mut self, stream: &str, sql: &str) -> Result<Position> {
        let head = self.head(stream)?;
        require(
            head.owned,
            "replica is read-only; queue an intent outside its copied database",
        )?;
        pages::writer(&self.root.join(&head.path))?.execute_batch(sql)?;
        self.capture(stream)?;
        Ok(self.head(stream)?.position)
    }

    pub(super) fn prune(&self, stream: &str, through: u64) -> Result<()> {
        self.journal.prune(stream, through)
    }

    fn head(&self, stream: &str) -> Result<Head> {
        check_stream(stream)?;
        self.journal.ready(stream)?;
        self.journal
            .head(stream)?
            .ok_or_else(|| std::io::Error::other("unknown stream").into())
    }

    fn snapshot(&self, stream: &str) -> Result<Packet> {
        let head = self.head(stream)?;
        let image = self.journal.image(stream, head.page_size)?;
        Ok(Packet {
            stream: stream.to_owned(),
            snapshot: true,
            position: head.position,
            previous: [0; 32],
            page_size: image.page_size,
            page_count: image.pages.len() as u32,
            pages: image.pages,
        })
    }

    fn expected_image(&self, packet: &Packet) -> Result<Image> {
        let mut image = if packet.snapshot {
            Image {
                page_size: packet.page_size,
                pages: Default::default(),
            }
        } else {
            self.journal.image(&packet.stream, packet.page_size)?
        };
        image.pages.extend(packet.pages.clone());
        image.pages.retain(|number, _| *number <= packet.page_count);
        require(
            image.pages.len() == packet.page_count as usize
                && image.checksum() == packet.position.checksum,
            "page checksum mismatch: no changes were applied",
        )?;
        Ok(image)
    }

    fn apply_staged(&mut self, packet: &Packet, bytes: &[u8]) -> Result<()> {
        self.expected_image(packet)?;
        let path = self.root.join(format!("replicas/{}.db", packet.stream));
        packet.image().apply(&path, packet.page_count)?;
        require(
            Image::read(&mut pages::reader(&path)?)?.checksum() == packet.position.checksum,
            "post-apply checksum mismatch: stream is blocked until repaired from a snapshot",
        )?;
        self.journal.finish(packet, bytes, false)
    }

    #[cfg(test)]
    pub(super) fn interrupt_apply(&mut self, bytes: &[u8], after_pages: bool) -> Result<()> {
        let packet = Packet::decode(bytes)?;
        self.journal.stage(&packet, bytes)?;
        if after_pages {
            let path = self.root.join(format!("replicas/{}.db", packet.stream));
            packet.image().apply(&path, packet.page_count)?;
        }
        Ok(())
    }
}
