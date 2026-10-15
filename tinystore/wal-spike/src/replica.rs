use crate::feed::{self, Entry, Feed};
use crate::pages::{Image, MAX_IMAGE, Pages};
use crate::{Result, require};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const LIMIT: usize = 32 * 1024 * 1024;

pub(super) fn pack(entries: &[Arc<Entry>]) -> Result<Vec<u8>> {
    require(
        !entries.is_empty() && entries.len() <= 1024,
        "batch record limit",
    )?;
    let mut raw = b"TSBATCH2".to_vec();
    raw.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for entry in entries {
        raw.extend_from_slice(&(entry.raw.len() as u32).to_be_bytes());
        raw.extend_from_slice(&entry.raw);
    }
    require(raw.len() <= LIMIT, "batch byte limit")?;
    Ok(zstd::stream::encode_all(raw.as_slice(), 1)?)
}

pub(super) fn unpack(bytes: &[u8]) -> Result<Vec<Entry>> {
    require(bytes.len() <= LIMIT, "compressed batch limit")?;
    let mut raw = Vec::new();
    zstd::stream::read::Decoder::new(bytes)?
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut raw)?;
    require(
        raw.len() <= LIMIT && raw.starts_with(b"TSBATCH2"),
        "invalid batch",
    )?;
    let mut cursor = Cursor::new(&raw[8..]);
    let count = number(&mut cursor)?;
    require(count > 0 && count <= 1024, "batch record limit")?;
    let mut entries = Vec::new();
    for _ in 0..count {
        let len = number(&mut cursor)? as usize;
        require(
            len <= MAX_IMAGE + 65556 && len <= raw.len(),
            "record length limit",
        )?;
        let mut bytes = vec![0; len];
        cursor.read_exact(&mut bytes)?;
        entries.push(feed::decode(bytes)?);
    }
    require(
        cursor.position() as usize == raw.len() - 8,
        "trailing batch bytes",
    )?;
    Ok(entries)
}
fn number(cursor: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut bytes = [0; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

pub(super) struct Replica {
    pub feed: Feed,
    pub path: PathBuf,
}
impl Replica {
    pub(super) fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let mut replica = Self {
            feed: Feed::open(&root.join("receive.log"))?,
            path: root.join("app.db"),
        };
        if !replica.feed.entries.is_empty() {
            replica.materialize_all()?;
        }
        Ok(replica)
    }
    pub(super) fn receive(&mut self, bytes: &[u8]) -> Result<()> {
        for entry in unpack(bytes)? {
            let seq = entry.packet.position.sequence;
            if let Some(known) = self.feed.entries.get(seq.saturating_sub(1) as usize) {
                require(
                    known.raw.as_slice() == entry.raw.as_slice(),
                    "conflicting duplicate",
                )?;
                continue;
            }
            require(entry.packet.position.epoch == 1, "unfenced epoch change")?;
            self.feed.append(entry.packet, entry.cursor)?;
        }
        self.feed.sync()
    }
    pub(super) fn apply_since(&mut self, sequence: usize) -> Result<()> {
        let entries = &self.feed.entries[sequence..];
        if entries.is_empty() {
            return Ok(());
        }
        let last = &entries.last().ok_or("empty batch")?.packet;
        let mut pages = Pages::new();
        for entry in entries {
            pages.extend(entry.packet.pages.clone());
        }
        pages.retain(|n, _| *n <= last.page_count);
        Image {
            page_size: last.page_size,
            pages,
        }
        .apply(&self.path, last.page_count)?;
        Ok(())
    }
    fn materialize_all(&mut self) -> Result<()> {
        self.apply_since(0)?;
        self.verify()
    }
    pub(super) fn verify(&self) -> Result<()> {
        let image = Image::read(&mut crate::pages::reader(&self.path)?)?;
        let mut tree = crate::tree::Tree::default();
        tree.apply(&image.pages, image.pages.len() as u32, image.page_size)?;
        require(
            tree.root() == self.feed.tree.root(),
            "replica integrity mismatch",
        )?;
        let check: String =
            crate::pages::reader(&self.path)?
                .query_row("pragma integrity_check", [], |r| r.get(0))?;
        require(check == "ok", "replica SQLite integrity check")
    }
}
