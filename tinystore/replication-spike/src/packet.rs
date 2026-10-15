use std::io::{Cursor, Read};

use crate::pages::{Image, MAX_IMAGE, Pages};
use crate::{Result, require};

const MAGIC: &[u8; 8] = b"TSREPL01";
pub(super) const MAX_PACKET: usize = MAX_IMAGE + 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Position {
    pub epoch: u64,
    pub sequence: u64,
    pub checksum: [u8; 32],
}

#[derive(Clone, Debug)]
pub(super) struct Packet {
    pub stream: String,
    pub snapshot: bool,
    pub position: Position,
    pub previous: [u8; 32],
    pub page_size: u32,
    pub page_count: u32,
    pub pages: Pages,
}

pub(super) fn check_stream(stream: &str) -> Result<()> {
    require(
        !stream.is_empty()
            && stream.len() <= 64
            && stream
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
        "stream id must be 1..64 ASCII letters, digits, hyphens or underscores",
    )
}

impl Packet {
    pub(super) fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.push(u8::from(self.snapshot));
        bytes.push(self.stream.len() as u8);
        bytes.extend_from_slice(self.stream.as_bytes());
        bytes.extend_from_slice(&self.position.epoch.to_be_bytes());
        bytes.extend_from_slice(&self.position.sequence.to_be_bytes());
        bytes.extend_from_slice(&self.position.checksum);
        bytes.extend_from_slice(&self.previous);
        for value in [self.page_size, self.page_count, self.pages.len() as u32] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        for (number, page) in &self.pages {
            bytes.extend_from_slice(&number.to_be_bytes());
            bytes.extend_from_slice(page);
        }
        require(
            bytes.len() <= MAX_PACKET,
            "packet exceeds the decoded byte limit",
        )?;
        Ok(zstd::stream::encode_all(bytes.as_slice(), 3)?)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self> {
        require(
            bytes.len() <= MAX_PACKET,
            "packet exceeds the compressed byte limit",
        )?;
        let mut decoder = zstd::stream::read::Decoder::new(bytes)?;
        decoder.window_log_max(25)?;
        let mut plain = Vec::new();
        decoder
            .take(MAX_PACKET as u64 + 1)
            .read_to_end(&mut plain)?;
        require(
            plain.len() <= MAX_PACKET,
            "packet exceeds the decoded byte limit",
        )?;
        let mut cursor = Cursor::new(plain.as_slice());
        require(
            &take::<8>(&mut cursor)? == MAGIC,
            "unknown replication packet version",
        )?;
        let kind = take::<1>(&mut cursor)?[0];
        require(kind <= 1, "unknown replication packet kind")?;
        let length = take::<1>(&mut cursor)?[0] as usize;
        require(length <= 64, "stream id exceeds the length limit")?;
        let mut stream = vec![0; length];
        cursor.read_exact(&mut stream)?;
        let stream = String::from_utf8(stream)?;
        let position = Position {
            epoch: u64::from_be_bytes(take(&mut cursor)?),
            sequence: u64::from_be_bytes(take(&mut cursor)?),
            checksum: take(&mut cursor)?,
        };
        let previous = take(&mut cursor)?;
        let page_size = u32::from_be_bytes(take(&mut cursor)?);
        let page_count = u32::from_be_bytes(take(&mut cursor)?);
        let changed = u32::from_be_bytes(take(&mut cursor)?);
        let remaining = plain.len().saturating_sub(cursor.position() as usize);
        require(
            u64::from(changed) * (u64::from(page_size) + 4) == remaining as u64,
            "packet page count does not match its byte length",
        )?;
        let pages = decode_pages(&mut cursor, changed, page_size)?;
        let packet = Self {
            stream,
            snapshot: kind == 1,
            position,
            previous,
            page_size,
            page_count,
            pages,
        };
        packet.validate()?;
        Ok(packet)
    }

    pub(super) fn image(&self) -> Image {
        Image {
            page_size: self.page_size,
            pages: self.pages.clone(),
        }
    }

    fn validate(&self) -> Result<()> {
        check_stream(&self.stream)?;
        require(
            self.position.epoch > 0 && self.position.sequence > 0,
            "zero stream position",
        )?;
        require(
            self.position.epoch <= i64::MAX as u64 && self.position.sequence <= i64::MAX as u64,
            "stream position exceeds the journal's integer limit",
        )?;
        require(
            (512..=65536).contains(&self.page_size) && self.page_size.is_power_of_two(),
            "invalid SQLite page size",
        )?;
        require(
            self.page_count > 0
                && u64::from(self.page_count) * u64::from(self.page_size) <= MAX_IMAGE as u64,
            "file exceeds the experiment's 16 MiB limit",
        )?;
        require(
            self.pages.iter().all(|(n, p)| {
                *n > 0 && *n <= self.page_count && p.len() == self.page_size as usize
            }),
            "packet contains an invalid page number or page length",
        )?;
        require(
            !self.snapshot
                || (self.pages.len() == self.page_count as usize && self.previous == [0; 32]),
            "snapshot must contain every page and have no predecessor",
        )
    }
}

fn take<const N: usize>(cursor: &mut Cursor<&[u8]>) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    cursor.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn decode_pages(cursor: &mut Cursor<&[u8]>, count: u32, size: u32) -> Result<Pages> {
    require(size <= 65536, "page size exceeds the limit")?;
    let mut pages = Pages::new();
    let mut previous = 0;
    for _ in 0..count {
        let number = u32::from_be_bytes(take(cursor)?);
        require(
            number > previous,
            "page numbers must be strictly increasing",
        )?;
        previous = number;
        let mut page = vec![0; size as usize];
        cursor.read_exact(&mut page)?;
        pages.insert(number, page);
    }
    Ok(pages)
}
