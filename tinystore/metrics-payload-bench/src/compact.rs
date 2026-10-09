use super::{Address, HEADER, MAX_PACK, crc, get, put};
use crate::Result;

fn metadata(data: &[u8]) -> Result<Vec<u8>> {
    match data.first() {
        Some(0) => Ok(data[1..].to_vec()),
        Some(1) => {
            let mut decoder = zstd::bulk::Decompressor::new()?;
            decoder.set_parameter(zstd::zstd_safe::DParameter::WindowLogMax(13))?;
            Ok(decoder.decompress(&data[1..], 8192)?)
        }
        _ => Err("compact metadata compression tag".into()),
    }
}
fn encode_metadata(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() > 8192 {
        return Err("compact expanded metadata bound".into());
    }
    let mut compressor = zstd::bulk::Compressor::new(3)?;
    compressor.set_parameter(zstd::zstd_safe::CParameter::WindowLog(13))?;
    let compressed = compressor.compress(raw)?;
    let mut out = Vec::new();
    if compressed.len() < raw.len() {
        out.push(1);
        out.extend(compressed);
    } else {
        out.push(0);
        out.extend_from_slice(raw);
    }
    Ok(out)
}
fn skip_scalar(data: &[u8], at: &mut usize) -> Result<()> {
    if get(data, at)? == 2 {
        *at = at.checked_add(8).ok_or("scalar offset")?;
        if *at > data.len() {
            return Err("truncated scalar".into());
        }
    }
    Ok(())
}
fn skip_exact(data: &[u8], at: &mut usize) -> Result<()> {
    let length = usize::try_from(get(data, at)?)?;
    if length > 264 {
        return Err("exact summary length".into());
    }
    if length != 0 {
        get(data, at)?;
        *at = at.checked_add(length).ok_or("exact offset")?;
        if *at > data.len() {
            return Err("truncated exact".into());
        }
    }
    Ok(())
}
/// Span through body length, preserving every original scalar and exact token.
fn summary_prefix(data: &[u8], at: &mut usize) -> Result<usize> {
    skip_scalar(data, at)?;
    let flags = *data.get(*at).ok_or("summary flags")?;
    *at += 1;
    if flags & 0x80 != 0 {
        return Err("summary flags".into());
    }
    for bit in 0..5 {
        if flags & (1 << bit) == 0 {
            skip_scalar(data, at)?;
        }
    }
    get(data, at)?;
    if flags & 0x40 != 0 {
        skip_exact(data, at)?;
        skip_exact(data, at)?;
    }
    let length = usize::try_from(get(data, at)?)?;
    if length > 8200 || (length > 0 && length < 6) {
        return Err("compact body length".into());
    }
    Ok(length)
}
fn directory_crc(series: i64, start: i64, end: i64, data: &[u8]) -> u32 {
    crc(&[
        &series.to_le_bytes(),
        &start.to_le_bytes(),
        &end.to_le_bytes(),
        data,
    ])
}

/// Experimental 0x51 directly substitutes addresses for old ids in one
/// compressed stream. Summary bytes/inline bodies are copied, never recomputed.
/// Offset zero is the same-format unpacked control using original payload rows.
pub fn wrap_compact(
    series: i64,
    start: i64,
    end: i64,
    original: &[u8],
    addresses: &[Address],
) -> Result<Vec<u8>> {
    if original.len() < 23 || original[0] != 4 || original[1] == 0 || original[1] > 32 {
        return Err("compact original directory".into());
    }
    let raw = metadata(&original[18..original.len() - 4])?;
    let allocation = u32::from_le_bytes(original[6..10].try_into()?);
    let mut unique = addresses.iter().map(|a| a.pack).collect::<Vec<_>>();
    unique.sort_unstable();
    unique.dedup();
    let mut rewritten = Vec::new();
    put(&mut rewritten, unique.len() as u64);
    for id in &unique {
        if *id <= 0 {
            return Err("compact pack id".into());
        }
        put(&mut rewritten, *id as u64);
    }
    let mut at = 0;
    let mut external = 0;
    for slot in 0..original[1] as usize {
        let before = at;
        let length = summary_prefix(&raw, &mut at)?;
        rewritten.extend_from_slice(&raw[before..at]);
        if allocation & (1 << slot) != 0 {
            get(&raw, &mut at)?;
            let a = *addresses.get(external).ok_or("compact address count")?;
            external += 1;
            if a.offset != 0
                && (a.offset < HEADER
                    || a.offset
                        .checked_add(length)
                        .is_none_or(|end| end > MAX_PACK))
            {
                return Err("compact address extent".into());
            }
            put(
                &mut rewritten,
                unique.binary_search(&a.pack).unwrap() as u64,
            );
            put(&mut rewritten, a.offset as u64);
            rewritten.extend(a.expected_crc.to_le_bytes());
        } else {
            if length > 16 || length > raw.len() - at {
                return Err("compact inline length".into());
            }
            rewritten.extend_from_slice(&raw[at..at + length]);
            at += length;
        }
    }
    if at != raw.len() || external != addresses.len() {
        return Err("compact trailing fields".into());
    }
    let mut out = original[..18].to_vec();
    out[0] = 0x51;
    out.extend(encode_metadata(&rewritten)?);
    let sum = directory_crc(series, start, end, &out);
    out.extend(sum.to_le_bytes());
    if out.len() > 8192 {
        return Err("compact directory budget".into());
    }
    Ok(out)
}

pub fn unwrap_compact(
    series: i64,
    start: i64,
    end: i64,
    data: &[u8],
) -> Result<(Vec<u8>, Vec<Address>)> {
    if data.len() < 23 || data.len() > 8192 || data[1] == 0 || data[1] > 32 {
        return Err("compact directory version/size".into());
    }
    if directory_crc(series, start, end, &data[..data.len() - 4])
        != u32::from_le_bytes(data[data.len() - 4..].try_into()?)
    {
        return Err("compact directory checksum".into());
    }
    let raw = metadata(&data[18..data.len() - 4])?;
    let mut at = 0;
    let count = usize::try_from(get(&raw, &mut at)?)?;
    if count > 32 {
        return Err("compact pack index size".into());
    }
    let mut unique = Vec::new();
    for _ in 0..count {
        let id = i64::try_from(get(&raw, &mut at)?)?;
        if id <= 0 || unique.last().is_some_and(|old| *old >= id) {
            return Err("compact pack index order".into());
        }
        unique.push(id);
    }
    let allocation = u32::from_le_bytes(data[6..10].try_into()?);
    let mut restored = Vec::new();
    let mut addresses = Vec::new();
    for slot in 0..data[1] as usize {
        let before = at;
        let length = summary_prefix(&raw, &mut at)?;
        restored.extend_from_slice(&raw[before..at]);
        if allocation & (1 << slot) != 0 {
            let id = *unique
                .get(usize::try_from(get(&raw, &mut at)?)?)
                .ok_or("compact pack index bounds")?;
            let offset = usize::try_from(get(&raw, &mut at)?)?;
            if (offset != 0
                && (offset < HEADER || offset.checked_add(length).is_none_or(|end| end > MAX_PACK)))
                || raw.len() - at < 4
            {
                return Err("compact offset/length".into());
            }
            let expected_crc = u32::from_le_bytes(raw[at..at + 4].try_into()?);
            at += 4;
            addresses.push(Address {
                pack: id,
                offset,
                expected_crc,
            });
            put(&mut restored, (slot + 1) as u64);
        } else {
            if length > 16 || length > raw.len() - at {
                return Err("compact inline length".into());
            }
            restored.extend_from_slice(&raw[at..at + length]);
            at += length;
        }
    }
    if at != raw.len() {
        return Err("compact trailing fields".into());
    }
    let mut original = data[..18].to_vec();
    original[0] = 4;
    original.push(0);
    original.extend(restored);
    let sum = directory_crc(series, start, end, &original);
    original.extend(sum.to_le_bytes());
    if original.len() > 8192 {
        return Err("restored directory budget".into());
    }
    Ok((original, addresses))
}
