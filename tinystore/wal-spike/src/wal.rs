use crate::pages::Pages;
use crate::{Result, require};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct Cursor {
    pub salts: [u8; 8],
    pub frame: u32,
    pub checksum: [u32; 2],
}

#[derive(Debug)]
pub(super) struct Commit {
    pub cursor: Cursor,
    pub page_size: u32,
    pub count: u32,
    pub pages: Pages,
}

pub(super) fn scan(path: &Path, mut cursor: Cursor, published: Option<u32>) -> Result<Vec<Commit>> {
    let mut name = path.as_os_str().to_os_string();
    name.push("-wal");
    let mut file = match File::open(name) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            require(published.is_none(), "published WAL file missing")?;
            return Ok(Vec::new());
        }
        Err(e) => return Err(e.into()),
    };
    if file.metadata()?.len() < 32 {
        require(published.is_none(), "published WAL header missing")?;
        return Ok(Vec::new());
    }
    let mut header = [0; 32];
    file.read_exact(&mut header)?;
    let magic = u32_at(&header, 0);
    require(
        magic == 0x377f0682 || magic == 0x377f0683,
        "invalid WAL magic",
    )?;
    require(u32_at(&header, 4) == 3007000, "unsupported WAL version")?;
    let size = u32_at(&header, 8);
    require(
        (512..=65536).contains(&size) && size.is_power_of_two(),
        "invalid WAL page size",
    )?;
    let big = magic & 1 == 1;
    let seed = checksum(&header[..24], [0; 2], big);
    require(
        seed == [u32_at(&header, 24), u32_at(&header, 28)],
        "WAL header checksum mismatch",
    )?;
    let salts: [u8; 8] = header[16..24].try_into()?;
    if cursor.salts != salts {
        cursor = Cursor {
            salts,
            frame: 0,
            checksum: seed,
        };
    }
    if cursor.frame == 0 {
        cursor.checksum = seed;
    }
    let available = ((file.metadata()?.len() - 32) / (24 + u64::from(size))) as u32;
    let limit = published.unwrap_or(available);
    require(
        limit <= available && cursor.frame <= limit,
        "published WAL position is unavailable",
    )?;
    file.seek(SeekFrom::Start(
        32 + u64::from(cursor.frame) * (24 + u64::from(size)),
    ))?;
    let mut pages = Pages::new();
    let mut commits = Vec::new();
    let mut bytes = vec![0; 24 + size as usize];
    for frame in cursor.frame + 1..=limit {
        file.read_exact(&mut bytes)?;
        let valid_salt = bytes[8..16] == salts;
        let sum = checksum(&bytes[..8], cursor.checksum, big);
        let sum = checksum(&bytes[24..], sum, big);
        let valid =
            valid_salt && sum == [u32_at(&bytes, 16), u32_at(&bytes, 20)] && u32_at(&bytes, 0) > 0;
        if !valid {
            require(published.is_none(), "published WAL frame checksum mismatch")?;
            break;
        }
        cursor = Cursor {
            salts,
            frame,
            checksum: sum,
        };
        let number = u32_at(&bytes, 0);
        let count = u32_at(&bytes, 4);
        require(
            number as usize * size as usize <= crate::pages::MAX_IMAGE,
            "WAL page exceeds image limit",
        )?;
        pages.insert(number, bytes[24..].to_vec());
        if count > 0 {
            pages.retain(|n, _| *n <= count);
            commits.push(Commit {
                cursor,
                page_size: size,
                count,
                pages: std::mem::take(&mut pages),
            });
        }
    }
    if published.is_some() {
        require(
            pages.is_empty(),
            "published position ends inside a transaction",
        )?;
    }
    Ok(commits)
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed WAL field"),
    )
}

fn checksum(bytes: &[u8], mut sum: [u32; 2], big: bool) -> [u32; 2] {
    for pair in bytes.as_chunks::<8>().0 {
        let read = |b: &[u8]| {
            if big {
                u32::from_be_bytes(b.try_into().expect("checksum word"))
            } else {
                u32::from_le_bytes(b.try_into().expect("checksum word"))
            }
        };
        sum[0] = sum[0].wrapping_add(read(&pair[..4])).wrapping_add(sum[1]);
        sum[1] = sum[1].wrapping_add(read(&pair[4..])).wrapping_add(sum[0]);
    }
    sum
}
