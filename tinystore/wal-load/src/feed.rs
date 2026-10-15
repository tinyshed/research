use crate::{Result, require};
use crate::{
    packet::{MAX_PACKET, Packet},
    stats::{Stats, elapsed},
    tree::Tree,
    wal::Cursor,
};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

#[derive(Debug)]
pub(super) struct Entry {
    pub packet: Packet,
    pub raw: Vec<u8>,
    pub charge: usize,
}
#[derive(Debug)]
pub(super) struct Feed {
    file: File,
    pub tree: Tree,
    pub cursor: Cursor,
    pub sequence: u64,
    pub next_checkpoint: u32,
    pub length: u64,
    pub dirty: usize,
    entries: VecDeque<Entry>,
    pub cache: usize,
    pub cache_limit: usize,
    pub owned: bool,
    pub stats: Stats,
}
impl Feed {
    pub(super) fn open(
        path: &Path,
        cache_limit: usize,
        owned: bool,
        batched: bool,
    ) -> Result<Self> {
        let new = !path.exists();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        if new {
            file.write_all(b"TSFEED02")?;
            file.sync_all()?;
            File::open(path.parent().ok_or("parent")?)?.sync_all()?;
        }
        file.rewind()?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        require(&magic == b"TSFEED02", "feed version")?;
        let mut f = Self {
            file,
            tree: Tree::default(),
            cursor: Cursor::default(),
            sequence: 0,
            next_checkpoint: 0,
            length: 8,
            dirty: 0,
            entries: VecDeque::new(),
            cache: 0,
            cache_limit,
            owned,
            stats: Stats::default(),
        };
        f.tree.batched = batched;
        let end = f.file.metadata()?.len();
        while f.length < end {
            if end - f.length < 4 {
                f.file.set_len(f.length)?;
                break;
            }
            let mut len = [0; 4];
            f.file.read_exact(&mut len)?;
            let len = u32::from_be_bytes(len) as usize;
            require(len <= MAX_PACKET + 20, "record limit")?;
            if f.length + len as u64 + 36 > end {
                f.file.set_len(f.length)?;
                break;
            }
            let mut raw = vec![0; len];
            f.file.read_exact(&mut raw)?;
            let mut hash = [0; 32];
            f.file.read_exact(&mut hash)?;
            if Sha256::digest(&raw).as_slice() != hash {
                require(f.length + len as u64 + 36 == end, "interior corruption")?;
                f.file.set_len(f.length)?;
                break;
            }
            let (cursor, packet) = decode(&raw)?;
            f.validate(&packet)?;
            f.tree
                .apply(&packet.pages, packet.page_count, packet.page_size)?;
            require(f.tree.root() == packet.position.checksum, "recovery root")?;
            f.sequence = packet.position.sequence;
            f.cursor = cursor;
            f.length += len as u64 + 36;
            f.remember(packet, raw);
        }
        f.file.seek(SeekFrom::End(0))?;
        f.reset_stats();
        Ok(f)
    }
    fn validate(&self, p: &Packet) -> Result<()> {
        require(
            p.position.sequence == self.sequence + 1 && p.previous == self.tree.root(),
            "feed continuity",
        )
    }
    fn remember(&mut self, packet: Packet, raw: Vec<u8>) {
        // Conservative reservation for payload capacities and map/Arc/node metadata.
        let charge = raw.capacity()
            + packet
                .pages
                .values()
                .map(|p| p.capacity() + 512)
                .sum::<usize>()
            + 512;
        if charge > self.cache_limit {
            return;
        }
        while self.cache + charge > self.cache_limit {
            if let Some(e) = self.entries.pop_front() {
                self.cache -= e.charge;
            }
        }
        self.cache += charge;
        self.stats.max_cache = self.stats.max_cache.max(self.cache);
        self.entries.push_back(Entry {
            packet,
            raw,
            charge,
        });
    }
    pub(super) fn append(&mut self, mut packet: Packet, cursor: Cursor) -> Result<()> {
        self.validate(&packet)?;
        let started = Instant::now();
        self.tree
            .apply(&packet.pages, packet.page_count, packet.page_size)?;
        if packet.position.checksum != [0; 32] {
            require(packet.position.checksum == self.tree.root(), "append root")?;
        }
        packet.position.checksum = self.tree.root();
        self.stats.tree_ns += elapsed(started);
        let bytes = packet.pages.len() * packet.page_size as usize;
        self.stats.copied_bytes += bytes as u64;
        let started = Instant::now();
        let mut raw = if self.owned {
            Vec::with_capacity(20 + packet.size())
        } else {
            Vec::new()
        };
        raw.extend_from_slice(&cursor.salts);
        raw.extend_from_slice(&cursor.frame.to_be_bytes());
        for value in cursor.checksum {
            raw.extend_from_slice(&value.to_be_bytes());
        }
        let packet = if self.owned {
            packet.encode_into(&mut raw)?;
            packet
        } else {
            raw.extend_from_slice(&packet.encode()?);
            self.stats.copied_bytes += 2 * bytes as u64;
            decode(&raw)?.1
        };
        self.stats.encode_ns += elapsed(started);
        let started = Instant::now();
        let hash = Sha256::digest(&raw);
        self.stats.digest_ns += elapsed(started);
        let started = Instant::now();
        self.file.write_all(&(raw.len() as u32).to_be_bytes())?;
        self.file.write_all(&raw)?;
        self.file.write_all(&hash)?;
        self.stats.append_ns += elapsed(started);
        self.length += raw.len() as u64 + 36;
        self.dirty += raw.len() + 36;
        self.stats.raw_bytes += raw.len() as u64;
        self.stats.pages += packet.pages.len() as u64;
        self.stats.commits += 1;
        self.sequence = packet.position.sequence;
        self.cursor = cursor;
        self.remember(packet, raw);
        Ok(())
    }
    pub(super) fn sync(&mut self) -> Result<()> {
        if self.dirty > 0 {
            let start = Instant::now();
            self.file.sync_data()?;
            self.stats.sync_ns += elapsed(start);
            self.stats.syncs += 1;
            self.dirty = 0;
        }
        Ok(())
    }
    pub(super) fn reset_stats(&mut self) {
        self.stats = Stats {
            max_cache: self.cache,
            ..Default::default()
        };
        self.tree.leaves = 0;
        self.tree.branches = 0;
    }
    pub(super) fn cached_payload(&self) -> usize {
        self.entries
            .iter()
            .map(|e| e.raw.capacity() + e.packet.pages.values().map(Vec::capacity).sum::<usize>())
            .sum()
    }
}
fn decode(raw: &[u8]) -> Result<(Cursor, Packet)> {
    require(raw.len() >= 20, "short cursor")?;
    Ok((
        Cursor {
            salts: raw[..8].try_into()?,
            frame: u32::from_be_bytes(raw[8..12].try_into()?),
            checksum: [
                u32::from_be_bytes(raw[12..16].try_into()?),
                u32::from_be_bytes(raw[16..20].try_into()?),
            ],
        },
        Packet::decode(&raw[20..])?,
    ))
}
