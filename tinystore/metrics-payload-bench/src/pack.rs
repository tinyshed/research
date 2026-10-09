use crate::{Result, codec};
use std::collections::BTreeMap;
#[path = "compact.rs"]
mod compact;
pub use compact::wrap_compact;

pub const HEADER: usize = 24;
pub const MAX_PACK: usize = 16 * 1024;
pub const MAX_DIRECTORY: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Address {
    pub pack: i64,
    pub offset: usize,
    pub expected_crc: u32,
}

pub fn crc(parts: &[&[u8]]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    for part in parts {
        h.update(part);
    }
    h.finalize()
}

fn put(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        out.push(value as u8 | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn get(input: &[u8], at: &mut usize) -> Result<u64> {
    let mut value = 0;
    for shift in (0..70).step_by(7) {
        let b = *input.get(*at).ok_or("truncated pack metadata")?;
        *at += 1;
        if shift == 63 && b > 1 {
            return Err("pack varint overflow".into());
        }
        value |= u64::from(b & 127) << shift;
        if b < 128 {
            return Ok(value);
        }
    }
    Err("pack varint overflow".into())
}

/// Entire envelope, including original addresses and new addresses, is charged
/// in groups.directory. Keeping old identifiers makes this conservative.
pub fn wrap(
    series: i64,
    start: i64,
    end: i64,
    original: &[u8],
    addresses: &[Address],
) -> Result<Vec<u8>> {
    let mut out = vec![0x50, 1];
    put(&mut out, original.len() as u64);
    out.extend_from_slice(original);
    let mut unique: Vec<i64> = addresses.iter().map(|a| a.pack).collect();
    unique.sort_unstable();
    unique.dedup();
    put(&mut out, unique.len() as u64);
    for id in &unique {
        put(&mut out, *id as u64);
    }
    put(&mut out, addresses.len() as u64);
    for address in addresses {
        if address.pack <= 0 || address.offset < HEADER || address.offset >= MAX_PACK {
            return Err("pack address extent".into());
        }
        put(
            &mut out,
            unique.binary_search(&address.pack).unwrap() as u64,
        );
        put(&mut out, address.offset as u64);
        out.extend_from_slice(&address.expected_crc.to_le_bytes());
    }
    let sum = crc(&[
        &series.to_le_bytes(),
        &start.to_le_bytes(),
        &end.to_le_bytes(),
        &out,
    ]);
    out.extend_from_slice(&sum.to_le_bytes());
    if out.len() > MAX_DIRECTORY {
        return Err("pack directory budget".into());
    }
    Ok(out)
}

pub fn unwrap(series: i64, start: i64, end: i64, data: &[u8]) -> Result<(Vec<u8>, Vec<Address>)> {
    if data.first() == Some(&0x51) {
        return compact::unwrap_compact(series, start, end, data);
    }
    if data.first() == Some(&4) {
        return Ok((data.to_vec(), vec![]));
    }
    if data.len() < 12 || data.len() > MAX_DIRECTORY || data[..2] != [0x50, 1] {
        return Err("pack directory version/size".into());
    }
    let content = &data[..data.len() - 4];
    if crc(&[
        &series.to_le_bytes(),
        &start.to_le_bytes(),
        &end.to_le_bytes(),
        content,
    ]) != u32::from_le_bytes(data[data.len() - 4..].try_into()?)
    {
        return Err("pack directory checksum".into());
    }
    let mut at = 2;
    let length = usize::try_from(get(content, &mut at)?)?;
    if length > MAX_DIRECTORY || length > content.len() - at {
        return Err("original directory extent".into());
    }
    let original = content[at..at + length].to_vec();
    at += length;
    let count = usize::try_from(get(content, &mut at)?)?;
    if count > 32 {
        return Err("pack index size".into());
    }
    let mut unique = Vec::with_capacity(count);
    for _ in 0..count {
        let id = i64::try_from(get(content, &mut at)?)?;
        if id <= 0 || unique.last().is_some_and(|previous| *previous >= id) {
            return Err("pack index ordering".into());
        }
        unique.push(id);
    }
    let count = usize::try_from(get(content, &mut at)?)?;
    if count > 32 {
        return Err("pack address count".into());
    }
    let mut addresses = Vec::with_capacity(count);
    for _ in 0..count {
        let pack = *unique
            .get(usize::try_from(get(content, &mut at)?)?)
            .ok_or("pack index bounds")?;
        let offset = usize::try_from(get(content, &mut at)?)?;
        if offset < HEADER || offset >= MAX_PACK || content.len() - at < 4 {
            return Err("pack address bounds".into());
        }
        let expected_crc = u32::from_le_bytes(content[at..at + 4].try_into()?);
        at += 4;
        addresses.push(Address {
            pack,
            offset,
            expected_crc,
        });
    }
    if at != content.len() {
        return Err("trailing pack metadata".into());
    }
    Ok((original, addresses))
}

pub fn finish_pack(id: i64, count: usize, mut data: Vec<u8>) -> Result<Vec<u8>> {
    if data.len() > MAX_PACK || data.len() <= HEADER || count == 0 || count > 32 || id <= 0 {
        return Err("pack extent".into());
    }
    let length = data.len() as u32;
    data[..4].copy_from_slice(b"MPK1");
    data[4..12].copy_from_slice(&id.to_le_bytes());
    data[12..16].copy_from_slice(&(count as u32).to_le_bytes());
    data[16..20].copy_from_slice(&length.to_le_bytes());
    let sum = crc(&[&data[..20]]);
    data[20..24].copy_from_slice(&sum.to_le_bytes());
    Ok(data)
}

pub fn check_header(id: i64, length: usize, header: &[u8]) -> Result<()> {
    if header.len() != HEADER
        || length > MAX_PACK
        || length <= HEADER
        || &header[..4] != b"MPK1"
        || i64::from_le_bytes(header[4..12].try_into()?) != id
        || u32::from_le_bytes(header[16..20].try_into()?) as usize != length
        || crc(&[&header[..20]]) != u32::from_le_bytes(header[20..24].try_into()?)
    {
        return Err("pack header/identity/size".into());
    }
    let count = u32::from_le_bytes(header[12..16].try_into()?);
    if count == 0 || count > 32 {
        return Err("pack block count".into());
    }
    Ok(())
}

pub fn check_body(address: Address, length: usize, pack_bytes: usize, body: &[u8]) -> Result<()> {
    if length <= codec::INLINE_BYTES
        || length > codec::MAX_PAYLOAD_BYTES
        || address.offset < HEADER
        || address
            .offset
            .checked_add(length)
            .is_none_or(|end| end > pack_bytes)
        || body.len() != length
    {
        return Err("selected body bounds".into());
    }
    if u32::from_le_bytes(body[body.len() - 4..].try_into()?) != address.expected_crc {
        return Err("selected body address swap".into());
    }
    Ok(())
}

pub fn set_live(series: i64, start: i64, end: i64, original: &mut [u8], live: u32) -> Result<()> {
    if original.len() < 23 || original[0] != 4 {
        return Err("original directory version".into());
    }
    original[2..6].copy_from_slice(&live.to_le_bytes());
    let length = original.len();
    let sum = crc(&[
        &series.to_le_bytes(),
        &start.to_le_bytes(),
        &end.to_le_bytes(),
        &original[..length - 4],
    ]);
    original[length - 4..].copy_from_slice(&sum.to_le_bytes());
    Ok(())
}

pub fn pack_bodies(
    bodies: &[(i64, Vec<u8>)],
    spec: &str,
) -> Result<(Vec<(i64, Vec<u8>, usize, usize)>, BTreeMap<i64, Address>)> {
    let (maximum_count, target) = if let Some(s) = spec.strip_prefix("count") {
        (s.parse::<usize>()?, MAX_PACK)
    } else if let Some(s) = spec.strip_prefix("bytes") {
        (32, s.parse::<usize>()?)
    } else {
        return Err("unknown pack spec".into());
    };
    if maximum_count == 0 || maximum_count > 32 || target == 0 || target > MAX_PACK {
        return Err("invalid bounded pack spec".into());
    }
    let mut packs = Vec::new();
    let mut addresses = BTreeMap::new();
    let mut data = vec![0; HEADER];
    let mut count = 0;
    let mut live_bytes = 0;
    let mut id = 1;
    for (payload, body) in bodies {
        if body.len() <= 16 || body.len() > 8200 {
            return Err("external body size".into());
        }
        if count > 0
            && (count == maximum_count
                || data.len() + body.len() > target
                || data.len() + body.len() > MAX_PACK)
        {
            packs.push((id, finish_pack(id, count, data)?, count, live_bytes));
            id += 1;
            data = vec![0; HEADER];
            count = 0;
            live_bytes = 0;
        }
        let address = Address {
            pack: id,
            offset: data.len(),
            expected_crc: u32::from_le_bytes(body[body.len() - 4..].try_into()?),
        };
        if addresses.insert(*payload, address).is_some() {
            return Err("duplicate source payload id".into());
        }
        data.extend_from_slice(body);
        count += 1;
        live_bytes += body.len();
    }
    if count > 0 {
        packs.push((id, finish_pack(id, count, data)?, count, live_bytes));
    }
    Ok((packs, addresses))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body(byte: u8) -> Vec<u8> {
        let mut body = vec![byte; 32];
        let sum = crc(&[&body]);
        body.extend(sum.to_le_bytes());
        body
    }
    #[test]
    fn addresses_are_charged_and_guarded() -> Result<()> {
        let bodies = vec![(1, body(1)), (2, body(2)), (3, body(3))];
        let (packs, addresses) = pack_bodies(&bodies, "count2")?;
        assert_eq!(packs.len(), 2);
        assert!(packs.iter().all(|p| p.1.len() <= MAX_PACK));
        let refs: Vec<_> = addresses.values().copied().collect();
        let encoded = wrap(8, 10, 90, &[4; 23], &refs)?;
        assert!(encoded.len() > 23 + 3 * 6);
        assert_eq!(unwrap(8, 10, 90, &encoded)?.1, refs);
        assert!(unwrap(9, 10, 90, &encoded).is_err());
        for i in 0..encoded.len() {
            let mut mutant = encoded.clone();
            mutant[i] ^= 1;
            assert!(unwrap(8, 10, 90, &mutant).is_err(), "unguarded byte {i}");
        }
        let a = addresses[&1];
        let data = &packs[0].1;
        check_header(1, data.len(), &data[..HEADER])?;
        assert!(check_header(2, data.len(), &data[..HEADER]).is_err());
        check_body(a, bodies[0].1.len(), data.len(), &bodies[0].1)?;
        assert!(check_body(a, bodies[0].1.len(), data.len(), &bodies[1].1).is_err());
        assert!(
            check_body(
                Address {
                    offset: usize::MAX,
                    ..a
                },
                36,
                data.len(),
                &bodies[0].1
            )
            .is_err()
        );
        assert!(check_body(a, 35, data.len(), &bodies[0].1).is_err());
        Ok(())
    }
    #[test]
    fn byte_target_is_bounded_even_for_large_body() -> Result<()> {
        let bodies = (1..50).map(|i| (i, vec![3; 8200])).collect::<Vec<_>>();
        let (packs, addresses) = pack_bodies(&bodies, "bytes1024")?;
        assert_eq!(packs.len(), 49);
        assert_eq!(addresses.len(), 49);
        assert!(packs.iter().all(|p| p.1.len() == 8224));
        Ok(())
    }
}
