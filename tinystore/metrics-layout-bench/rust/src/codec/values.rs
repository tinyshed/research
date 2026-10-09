use super::{Block, Head, Result, Sample, binary, checksum, envelope, residuals};
use binary::{Reader, fold, put_unsigned, unfold};

fn constant(points: &[Sample]) -> bool {
    points[1..]
        .iter()
        .all(|p| p.value.to_bits() == points[0].value.to_bits())
}
fn factor(kind: u8) -> f64 {
    if kind == 1 { 100.0 } else { 1.0 }
}
fn changes(points: &[Sample]) -> Vec<u8> {
    if constant(points) {
        return Vec::new();
    }
    let mut best: Option<Vec<u8>> = None;
    for kind in 0..3 {
        let mut integers = vec![0i64; points.len()];
        let mut fits = true;
        if kind != 2 {
            for (i, point) in points.iter().enumerate() {
                let q = (point.value * factor(kind)).round();
                if !q.is_finite()
                    || q.abs() >= 2f64.powi(60)
                    || (q as i64 as f64 / factor(kind)).to_bits() != point.value.to_bits()
                {
                    fits = false;
                    break;
                }
                integers[i] = q as i64;
            }
        }
        if !fits {
            continue;
        }
        let mut out = vec![1, kind];
        let mut previous = 0;
        for i in 1..points.len() {
            if points[i].value.to_bits() != points[i - 1].value.to_bits() {
                put_unsigned(&mut out, (i - previous) as u64);
                previous = i;
                if kind == 2 {
                    out.extend_from_slice(&points[i].value.to_bits().to_le_bytes());
                } else {
                    put_unsigned(&mut out, fold(integers[i] - integers[i - 1]));
                }
            }
        }
        if best.as_ref().is_none_or(|b| out.len() < b.len()) {
            best = Some(out);
        }
    }
    best.unwrap()
}
fn ordinary(points: &[Sample]) -> Result<Vec<u8>> {
    let mut out = vec![0];
    out.extend(envelope::encode_value_stream(points)?);
    Ok(out)
}
fn ordered(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits >> 63 != 0 {
        !bits
    } else {
        bits ^ (1u64 << 63)
    }
}
fn unorder(bits: u64) -> f64 {
    f64::from_bits(if bits >> 63 == 0 {
        !bits
    } else {
        bits ^ (1u64 << 63)
    })
}
fn grid(points: &[Sample], scale: i32) -> Result<Option<Vec<u8>>> {
    let factor = 10f64.powi(scale);
    let mut quantized = Vec::with_capacity(points.len());
    let mut errors = Vec::with_capacity(points.len() - 1);
    for (i, point) in points.iter().enumerate() {
        let q = (point.value * factor).round();
        if !q.is_finite() || q.abs() > 2f64.powi(53) {
            return Ok(None);
        }
        quantized.push(Sample {
            at: i as i64,
            value: q,
        });
        if i > 0 {
            errors.push(ordered(point.value).wrapping_sub(ordered(q / factor)) as i64);
        }
    }
    let base = ordinary(&quantized)?;
    let residual = residuals::encode(&errors)?;
    let mut out = vec![2, scale as u8];
    put_unsigned(&mut out, base.len() as u64);
    out.extend(base);
    out.extend(residual);
    Ok(Some(out))
}
pub fn encode(points: &[Sample], hint: i32) -> Result<(Vec<u8>, i32)> {
    let changes = changes(points);
    if changes.is_empty() {
        return Ok((Vec::new(), hint));
    }
    let mut best = ordinary(points)?;
    if changes.len() < best.len() {
        best = changes;
    }
    let mut selected = if hint == -2 { -1 } else { hint };
    if hint >= 0 || hint == -2 {
        let (first, last) = if hint == -2 { (0, 15) } else { (hint, hint) };
        for scale in first..=last {
            if let Some(candidate) = grid(points, scale)? {
                if candidate.len() < best.len() {
                    best = candidate;
                    selected = scale;
                }
            }
        }
    }
    Ok((best, selected))
}
pub fn value_checksum(block: &Block, body: &[u8]) -> u32 {
    if crate::tuning::buffers() {
        let mut key = [0; 26];
        key[..8].copy_from_slice(&block.head.start.to_le_bytes());
        key[8..16].copy_from_slice(&block.head.end.to_le_bytes());
        key[16..18].copy_from_slice(&(block.head.count as u16).to_le_bytes());
        key[18..].copy_from_slice(&block.head.first.to_bits().to_le_bytes());
        return checksum(&[&key, &block.clock, body]);
    }
    let mut key = Vec::with_capacity(26);
    key.extend_from_slice(&block.head.start.to_le_bytes());
    key.extend_from_slice(&block.head.end.to_le_bytes());
    key.extend_from_slice(&(block.head.count as u16).to_le_bytes());
    key.extend_from_slice(&block.head.first.to_bits().to_le_bytes());
    checksum(&[&key, &block.clock, body])
}
pub fn seal(block: &Block, mut body: Vec<u8>) -> Vec<u8> {
    if !body.is_empty() {
        body.extend_from_slice(&value_checksum(block, &body).to_le_bytes());
    }
    body
}
fn read_ordinary(head: Head, body: &[u8]) -> Result<Vec<Sample>> {
    if body.len() < 2 || body[0] != 0 {
        return Err("ordinary value flags".into());
    }
    envelope::decode_value_stream(head.first, head.count, &body[1..])
}
fn read_ordinary_into(head: Head, body: &[u8], out: &mut Vec<Sample>) -> Result<()> {
    if body.len() < 2 || body[0] != 0 {
        return Err("ordinary value flags".into());
    }
    envelope::decode_value_stream_into(head.first, head.count, &body[1..], out)
}
fn read_changes(head: Head, data: &[u8]) -> Result<Vec<Sample>> {
    let mut out = Vec::with_capacity(head.count);
    read_changes_into(head, data, &mut out)?;
    Ok(out)
}
fn read_changes_into(head: Head, data: &[u8], out: &mut Vec<Sample>) -> Result<()> {
    let mut r = Reader::new(data);
    let kind = r.byte()?;
    if kind > 2 {
        return Err("change representation".into());
    }
    let mut integer = 0i64;
    let mut value = head.first;
    if kind != 2 {
        let q = (value * factor(kind)).round();
        if !q.is_finite()
            || q.abs() >= 2f64.powi(60)
            || (q as i64 as f64 / factor(kind)).to_bits() != value.to_bits()
        {
            return Err("first change value".into());
        }
        integer = q as i64;
    }
    let mut next = r.size(head.count - 1)?;
    if next == 0 {
        return Err("first change position".into());
    }
    out.reserve(head.count);
    for i in 0..head.count {
        if i == next {
            if kind == 2 {
                value = f64::from_bits(r.word()?);
            } else {
                integer = integer
                    .checked_add(unfold(r.unsigned()?))
                    .ok_or("change integer overflow")?;
                if integer <= -(1i64 << 60) || integer >= 1i64 << 60 {
                    return Err("change integer range".into());
                }
                value = integer as f64 / factor(kind);
            }
            if r.remaining().is_empty() {
                next = head.count;
            } else {
                let gap = r.size(head.count - 1 - i)?;
                if gap == 0 {
                    return Err("change position".into());
                }
                next = i + gap;
            }
        }
        out.push(Sample {
            at: i as i64,
            value,
        });
    }
    r.finish()?;
    Ok(())
}
fn read_grid(head: Head, data: &[u8]) -> Result<Vec<Sample>> {
    let mut out = Vec::with_capacity(head.count);
    read_grid_into(head, data, &mut out)?;
    Ok(out)
}
fn read_grid_into(mut head: Head, data: &[u8], out: &mut Vec<Sample>) -> Result<()> {
    let mut r = Reader::new(data);
    let scale = r.byte()? as i32;
    if scale > 15 {
        return Err("grid scale".into());
    }
    let length = r.size(8200)?;
    let base = r.take(length)?;
    let factor = 10f64.powi(scale);
    let first = head.first;
    head.first = (first * factor).round();
    if !head.first.is_finite() || head.first.abs() > 2f64.powi(53) {
        return Err("grid seed".into());
    }
    let before = out.len();
    read_ordinary_into(head, base, out)?;
    let errors = residuals::decode(r.remaining(), head.count - 1)?;
    let points = &mut out[before..];
    points[0].value = first;
    for i in 1..points.len() {
        points[i].value =
            unorder(ordered(points[i].value / factor).wrapping_add(errors[i - 1] as u64));
    }
    Ok(())
}
pub fn decode(block: &Block) -> Result<Vec<Sample>> {
    let stored = block.value_body();
    if stored.is_empty() {
        return Ok((0..block.head.count)
            .map(|i| Sample {
                at: i as i64,
                value: block.head.first,
            })
            .collect());
    }
    if stored.len() < 5 {
        return Err("value body size".into());
    }
    let end = stored.len() - 4;
    let body = &stored[..end];
    if value_checksum(block, body) != u32::from_le_bytes(stored[end..].try_into().unwrap()) {
        return Err("value body checksum".into());
    }
    match body[0] {
        0 => read_ordinary(block.head, body),
        1 => read_changes(block.head, &body[1..]),
        2 => read_grid(block.head, &body[1..]),
        _ => Err("value representation".into()),
    }
}

/// Fill caller-owned sample storage. The clock layer validates the head before
/// invoking this function and checks timestamps before consumers see the slice.
pub(super) fn decode_into(block: &Block, out: &mut Vec<Sample>) -> Result<()> {
    let before = out.len();
    let result = decode_into_inner(block, out);
    if result.is_err() {
        out.truncate(before);
    }
    result
}

fn decode_into_inner(block: &Block, out: &mut Vec<Sample>) -> Result<()> {
    let stored = block.value_body();
    if stored.is_empty() {
        out.extend((0..block.head.count).map(|i| Sample {
            at: i as i64,
            value: block.head.first,
        }));
        return Ok(());
    }
    if stored.len() < 5 {
        return Err("value body size".into());
    }
    let end = stored.len() - 4;
    let body = &stored[..end];
    if value_checksum(block, body) != u32::from_le_bytes(stored[end..].try_into().unwrap()) {
        return Err("value body checksum".into());
    }
    match body[0] {
        0 => read_ordinary_into(block.head, body, out),
        1 => read_changes_into(block.head, &body[1..], out),
        2 => read_grid_into(block.head, &body[1..], out),
        _ => Err("value representation".into()),
    }
}
