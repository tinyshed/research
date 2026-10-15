use crate::packet::{MAX_PACKET, Packet};
use crate::tree::Tree;
use crate::wal::Cursor;
use crate::{Result, require};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct Entry {
    pub cursor: Cursor,
    pub packet: Packet,
    pub raw: Arc<Vec<u8>>,
}

#[derive(Debug)]
pub(super) struct Feed {
    file: File,
    pub entries: Vec<Arc<Entry>>,
    pub tree: Tree,
    pub dirty: usize,
    pub cursor: Cursor,
}

impl Feed {
    pub(super) fn open(path: &Path) -> Result<Self> {
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
            File::open(path.parent().ok_or("feed parent")?)?.sync_all()?;
        }
        file.seek(SeekFrom::Start(0))?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        require(&magic == b"TSFEED02", "invalid feed version")?;
        let mut feed = Self {
            file,
            entries: Vec::new(),
            tree: Tree::default(),
            dirty: 0,
            cursor: Cursor::default(),
        };
        let end = feed.file.metadata()?.len();
        let mut offset = 8;
        while offset < end {
            let mut length = [0; 4];
            if end - offset < 4 {
                feed.file.set_len(offset)?;
                break;
            }
            feed.file.read_exact(&mut length)?;
            let length = u32::from_be_bytes(length) as usize;
            require(length <= MAX_PACKET + 20, "feed record limit exceeded")?;
            if offset + 36 + length as u64 > end {
                feed.file.set_len(offset)?;
                break;
            }
            let mut raw = vec![0; length];
            feed.file.read_exact(&mut raw)?;
            let mut hash = [0; 32];
            feed.file.read_exact(&mut hash)?;
            if Sha256::digest(&raw).as_slice() != hash {
                require(
                    offset + 36 + length as u64 == end,
                    "feed corruption inside committed prefix",
                )?;
                feed.file.set_len(offset)?;
                break;
            }
            feed.remember(decode(raw)?)?;
            offset += 36 + length as u64;
        }
        feed.file.seek(SeekFrom::End(0))?;
        Ok(feed)
    }

    pub(super) fn append(&mut self, mut packet: Packet, cursor: Cursor) -> Result<()> {
        require(
            packet.previous == self.tree.root(),
            "capture predecessor mismatch",
        )?;
        require(
            packet.position.sequence == self.entries.len() as u64 + 1,
            "capture sequence mismatch",
        )?;
        self.tree
            .apply(&packet.pages, packet.page_count, packet.page_size)?;
        if packet.position.checksum != [0; 32] {
            require(
                packet.position.checksum == self.tree.root(),
                "capture tree mismatch",
            )?;
        }
        packet.position.checksum = self.tree.root();
        let mut raw = Vec::new();
        raw.extend_from_slice(&cursor.salts);
        raw.extend_from_slice(&cursor.frame.to_be_bytes());
        raw.extend_from_slice(&cursor.checksum[0].to_be_bytes());
        raw.extend_from_slice(&cursor.checksum[1].to_be_bytes());
        raw.extend_from_slice(&packet.encode()?);
        let entry = decode(raw)?;
        let length = entry.raw.len();
        require(
            self.file.stream_position()? + length as u64 + 36 <= 256 * 1024 * 1024,
            "feed disk bound reached",
        )?;
        self.file.write_all(&(length as u32).to_be_bytes())?;
        self.file.write_all(&entry.raw)?;
        self.file.write_all(&Sha256::digest(entry.raw.as_slice()))?;
        self.dirty += length + 36;
        self.cursor = entry.cursor;
        self.entries.push(Arc::new(entry));
        Ok(())
    }

    fn remember(&mut self, entry: Entry) -> Result<()> {
        let expected = self
            .entries
            .last()
            .map_or(1, |e| e.packet.position.sequence + 1);
        require(
            entry.packet.position.sequence == expected,
            "feed sequence gap",
        )?;
        require(
            entry.packet.previous == self.tree.root(),
            "feed predecessor mismatch",
        )?;
        self.tree.apply(
            &entry.packet.pages,
            entry.packet.page_count,
            entry.packet.page_size,
        )?;
        require(
            self.tree.root() == entry.packet.position.checksum,
            "feed tree mismatch",
        )?;
        self.cursor = entry.cursor;
        self.entries.push(Arc::new(entry));
        Ok(())
    }

    pub(super) fn sync(&mut self) -> Result<()> {
        self.file.sync_data()?;
        self.dirty = 0;
        Ok(())
    }
}

pub(super) fn decode(raw: Vec<u8>) -> Result<Entry> {
    require(raw.len() >= 20, "short feed cursor")?;
    let cursor = Cursor {
        salts: raw[..8].try_into()?,
        frame: u32::from_be_bytes(raw[8..12].try_into()?),
        checksum: [
            u32::from_be_bytes(raw[12..16].try_into()?),
            u32::from_be_bytes(raw[16..20].try_into()?),
        ],
    };
    let packet = Packet::decode(&raw[20..])?;
    Ok(Entry {
        cursor,
        packet,
        raw: Arc::new(raw),
    })
}
