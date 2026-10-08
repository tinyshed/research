use super::{BLOCK_SAMPLES, Head, Result, Sample, advance, binary, distance, huffman};
use binary::{Reader, fold, put_unsigned, unfold};
use std::borrow::Cow;

const COUNTS: [usize; 16] = [240, 120, 60, 30, 20, 15, 12, 10, 8, 7, 6, 5, 4, 3, 2, 1];
const WIDTHS: [u32; 16] = [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 15, 20, 30, 60];
const POW10: [f64; 10] = [
    1.0,
    10.0,
    100.0,
    1000.0,
    10000.0,
    100000.0,
    1000000.0,
    10000000.0,
    100000000.0,
    1000000000.0,
];

#[derive(Default)]
struct BitsOut {
    data: Vec<u8>,
    used: u32,
}
impl BitsOut {
    fn put(&mut self, value: u64, mut width: u32) {
        while width > 0 {
            if self.used == 0 {
                self.data.push(0);
            }
            let take = width.min(8 - self.used);
            let part = ((value >> (width - take)) & ((1 << take) - 1)) as u8;
            let last = self.data.len() - 1;
            self.data[last] |= part << (8 - self.used - take);
            self.used = (self.used + take) % 8;
            width -= take;
        }
    }
}
struct BitsIn<'a> {
    data: &'a [u8],
    position: usize,
}
impl<'a> BitsIn<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    fn get(&mut self, mut width: u32) -> Result<u64> {
        if width as usize > self.data.len() * 8 - self.position {
            return Err("truncated XOR bits".into());
        }
        let mut value = 0;
        while width > 0 {
            let used = self.position % 8;
            let take = width.min(8 - used as u32);
            let part = (self.data[self.position / 8] >> (8 - used - take as usize))
                & ((1u16 << take) - 1) as u8;
            value = (value << take) | u64::from(part);
            self.position += take as usize;
            width -= take;
        }
        Ok(value)
    }
    fn finish(&mut self) -> Result<()> {
        let left = self.data.len() * 8 - self.position;
        if left >= 8 || self.get(left as u32)? != 0 {
            return Err("trailing XOR bits".into());
        }
        Ok(())
    }
}

fn header_bytes(head: Head) -> Vec<u8> {
    let mut out = Vec::with_capacity(26);
    out.extend_from_slice(&head.start.to_le_bytes());
    out.extend_from_slice(&head.end.to_le_bytes());
    out.extend_from_slice(&(head.count as u16).to_le_bytes());
    out.extend_from_slice(&head.first.to_bits().to_le_bytes());
    out
}
fn head_step(head: Head) -> Option<u64> {
    if head.count < 2 {
        return None;
    }
    let span = distance(head.start, head.end);
    let divisor = (head.count - 1) as u64;
    if span == 0 || span % divisor != 0 {
        None
    } else {
        Some(span / divisor)
    }
}

fn encode_times(head: Head, points: &[Sample]) -> (u8, Vec<u8>) {
    if points.len() == 1 {
        return (0, Vec::new());
    }
    if let Some(step) = head_step(head) {
        if points
            .windows(2)
            .all(|p| distance(p[0].at, p[1].at) == step)
        {
            return (0, Vec::new());
        }
    }
    let mut deltas = Vec::new();
    let mut second = Vec::new();
    let mut previous = distance(points[0].at, points[1].at);
    put_unsigned(&mut second, previous);
    let mut second_ok = previous <= i64::MAX as u64;
    for (i, pair) in points.windows(2).enumerate() {
        let delta = distance(pair[0].at, pair[1].at);
        put_unsigned(&mut deltas, delta);
        second_ok &= delta <= i64::MAX as u64;
        if i > 0 && second_ok {
            put_unsigned(&mut second, fold(delta as i64 - previous as i64));
        }
        previous = delta;
    }
    if second_ok && second.len() < deltas.len() {
        (2, second)
    } else {
        (1, deltas)
    }
}

fn raw_values(points: &[Sample]) -> Vec<u8> {
    let mut out = Vec::with_capacity((points.len() - 1) * 8);
    for point in &points[1..] {
        out.extend_from_slice(&point.value.to_bits().to_le_bytes());
    }
    out
}
fn pack_words(prefix: &[u8], values: &[u64]) -> Vec<u8> {
    let mut out = prefix.to_vec();
    let mut at = 0;
    while at < values.len() {
        for selector in 0..16 {
            let width = WIDTHS[selector];
            let count = COUNTS[selector].min(values.len() - at);
            let fits = if width == 0 {
                values.len() - at >= COUNTS[selector]
                    && values[at..at + count].iter().all(|&v| v == 1)
            } else {
                values[at..at + count].iter().all(|&v| v < (1u64 << width))
            };
            if !fits {
                continue;
            }
            let mut word = (selector as u64) << 60;
            if width > 0 {
                for (i, &v) in values[at..at + count].iter().enumerate() {
                    word |= v << (i as u32 * width);
                }
            }
            out.extend_from_slice(&word.to_le_bytes());
            at += count;
            break;
        }
    }
    out
}
fn pack_deltas(prefix: &[u8], deltas: &[u64]) -> Vec<u8> {
    let mut word_prefix = vec![0];
    word_prefix.extend_from_slice(prefix);
    let words = pack_words(&word_prefix, deltas);
    let mut symbols = Vec::with_capacity(deltas.len());
    for &delta in deltas {
        put_unsigned(&mut symbols, delta);
    }
    if let Some(packed) = huffman::compress(&symbols) {
        if 1 + prefix.len() + packed.len() < words.len() {
            let mut out = vec![1];
            out.extend_from_slice(prefix);
            out.extend(packed);
            return out;
        }
    }
    words
}
fn integer_candidate(points: &[Sample], scale: Option<usize>) -> Option<Vec<u8>> {
    let factor = scale.map_or(1.0, |s| POW10[s]);
    let mut deltas = Vec::with_capacity(points.len() - 1);
    let mut previous = 0i64;
    for (i, point) in points.iter().enumerate() {
        let converted = if scale.is_some() {
            (point.value * factor).round()
        } else {
            point.value
        };
        if !converted.is_finite()
            || converted < -((1u64 << 63) as f64)
            || converted >= (1u64 << 63) as f64
        {
            return None;
        }
        let current = converted as i64;
        if (current as f64 / factor).to_bits() != point.value.to_bits() {
            return None;
        }
        if i > 0 {
            let delta = current.checked_sub(previous)?;
            let zigzag = fold(delta);
            if zigzag >= 1u64 << 60 {
                return None;
            }
            deltas.push(zigzag);
        }
        previous = current;
    }
    Some(pack_deltas(
        &scale.map_or(Vec::new(), |s| vec![s as u8]),
        &deltas,
    ))
}
fn scaled_candidate(points: &[Sample]) -> Option<Vec<u8>> {
    let mut scale = 0;
    for point in points {
        if !point.value.is_finite() {
            return None;
        }
        let mut needed = None;
        for (s, factor) in POW10.iter().enumerate() {
            let product = (point.value * factor).round();
            if product < -((1u64 << 63) as f64) || product >= (1u64 << 63) as f64 {
                return None;
            }
            if ((product as i64 as f64) / factor).to_bits() == point.value.to_bits() {
                needed = Some(s);
                break;
            }
        }
        scale = scale.max(needed?);
    }
    if scale == 0 {
        None
    } else {
        integer_candidate(points, Some(scale))
    }
}
fn encode_xor(points: &[Sample]) -> Vec<u8> {
    let mut previous = points[0].value.to_bits();
    let mut bits = BitsOut::default();
    let (mut leading, mut trailing, mut window) = (0, 0, false);
    for point in &points[1..] {
        let current = point.value.to_bits();
        let difference = current ^ previous;
        if difference == 0 {
            bits.put(0, 1);
        } else {
            bits.put(1, 1);
            let left = difference.leading_zeros().min(31);
            let right = difference.trailing_zeros();
            if window && left >= leading && right >= trailing {
                bits.put(0, 1);
                bits.put(difference >> trailing, 64 - leading - trailing);
            } else {
                bits.put(1, 1);
                bits.put(u64::from(left), 5);
                let width = 64 - left - right;
                bits.put(u64::from(width) & 63, 6);
                bits.put(difference >> right, width);
                leading = left;
                trailing = right;
                window = true;
            }
        }
        previous = current;
    }
    bits.data
}
fn encode_values(points: &[Sample]) -> (u8, Vec<u8>) {
    if points[1..]
        .iter()
        .all(|p| p.value.to_bits() == points[0].value.to_bits())
    {
        return (1, Vec::new());
    }
    let mut mode = 0;
    let mut best = raw_values(points);
    let integer = integer_candidate(points, None);
    let has_integer = integer.is_some();
    if let Some(candidate) = integer {
        if candidate.len() < best.len() {
            mode = 2;
            best = candidate;
        }
    }
    if !has_integer {
        if let Some(candidate) = scaled_candidate(points) {
            if candidate.len() < best.len() {
                mode = 4;
                best = candidate;
            }
        }
    }
    let xor = encode_xor(points);
    if xor.len() < best.len() {
        mode = 3;
        best = xor;
    }
    (mode, best)
}
fn envelope(
    head: Head,
    time_mode: u8,
    value_mode: u8,
    times: &[u8],
    values: &[u8],
) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(times.len() + values.len());
    body.extend_from_slice(times);
    body.extend_from_slice(values);
    let mut flags = 0;
    if body.len() >= 48 {
        let packed = binary::compress(&body)?;
        if packed.len() < body.len() {
            body = packed;
            flags = 1;
        }
    }
    let mut out = vec![1, time_mode | (value_mode << 2) | (flags << 5)];
    out.extend_from_slice(&(times.len() as u16).to_le_bytes());
    out.extend(body);
    let crc = binary::crc32c(&[&header_bytes(head), &out]);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(out)
}
pub fn encode(points: &[Sample]) -> Result<(Head, Vec<u8>)> {
    if points.is_empty()
        || points.len() > BLOCK_SAMPLES
        || points.windows(2).any(|p| p[1].at <= p[0].at)
    {
        return Err("encode sample count/ordering".into());
    }
    let head = Head {
        start: points[0].at,
        end: points.last().unwrap().at,
        count: points.len(),
        first: points[0].value,
    };
    let (time_mode, times) = encode_times(head, points);
    let (value_mode, values) = encode_values(points);
    let mut best = envelope(head, time_mode, value_mode, &times, &values)?;
    if (value_mode == 3 && points.len() >= 8)
        || ((value_mode == 2 || value_mode == 4) && points.len() >= 32)
    {
        let alternative = envelope(head, time_mode, 0, &times, &raw_values(points))?;
        if alternative.len() < best.len() {
            best = alternative;
        }
    }
    Ok((head, best))
}
pub fn encode_value_stream(points: &[Sample]) -> Result<Vec<u8>> {
    let indexed: Vec<Sample> = points
        .iter()
        .enumerate()
        .map(|(i, p)| Sample {
            at: i as i64,
            value: p.value,
        })
        .collect();
    let (_, body) = encode(&indexed)?;
    let mut out = vec![body[1]];
    out.extend_from_slice(&body[4..body.len() - 4]);
    Ok(out)
}

fn check_flags(flags: u8) -> Result<()> {
    if flags & 3 > 2 || flags >> 2 & 7 > 4 || flags >> 5 > 1 {
        return Err("format header".into());
    }
    Ok(())
}
pub fn decode(head: Head, data: &[u8]) -> Result<Vec<Sample>> {
    if head.count == 0
        || head.count > 240
        || head.end < head.start
        || (head.count == 1 && head.end != head.start)
    {
        return Err("codec head".into());
    }
    if data.len() < 8 || data.len() > 8200 || data[0] != 1 {
        return Err("codec body version/length".into());
    }
    check_flags(data[1])?;
    let end = data.len() - 4;
    if binary::crc32c(&[&header_bytes(head), &data[..end]])
        != u32::from_le_bytes(data[end..].try_into().unwrap())
    {
        return Err("codec checksum".into());
    }
    decode_stream(
        head,
        data[1],
        u16::from_le_bytes(data[2..4].try_into().unwrap()) as usize,
        &data[4..end],
    )
}
pub fn decode_value_stream(first: f64, count: usize, data: &[u8]) -> Result<Vec<Sample>> {
    if count == 0 || count > 240 || data.is_empty() || data.len() > 8193 {
        return Err("value stream size".into());
    }
    check_flags(data[0])?;
    if data[0] & 3 != 0 {
        return Err("value stream timestamps".into());
    }
    decode_stream(
        Head {
            start: 0,
            end: count as i64 - 1,
            count,
            first,
        },
        data[0],
        0,
        &data[1..],
    )
}
fn decode_stream(head: Head, flags: u8, time_bytes: usize, payload: &[u8]) -> Result<Vec<Sample>> {
    let stream = payload_stream(flags, payload, super::optimized())?;
    if stream.len() > 8192 || time_bytes > stream.len() {
        return Err("stream lengths".into());
    }
    let mut times = Reader::new(&stream[..time_bytes]);
    let mut values = Reader::new(&stream[time_bytes..]);
    let time_mode = flags & 3;
    let value_mode = flags >> 2 & 7;
    let mut delta = if time_mode == 0 && head.count > 1 {
        head_step(head).ok_or("fixed step")?
    } else {
        0
    };
    let mut scale = None;
    let mut packing = 0;
    let mut integer = 0i64;
    let huff;
    if value_mode == 2 || value_mode == 4 {
        packing = values.byte()?;
        if packing > 1 {
            return Err("delta packing".into());
        }
        if value_mode == 4 {
            let s = values.byte()? as usize;
            if s > 9 {
                return Err("scale".into());
            }
            scale = Some(s);
        }
        let factor = scale.map_or(1.0, |s| POW10[s]);
        let seed = if scale.is_some() {
            (head.first * factor).round()
        } else {
            head.first
        };
        if !seed.is_finite() || seed < -((1u64 << 63) as f64) || seed >= (1u64 << 63) as f64 {
            return Err("first integer".into());
        }
        integer = seed as i64;
        if (integer as f64 / factor).to_bits() != head.first.to_bits() {
            return Err("first integer".into());
        }
        if packing == 1 && head.count > 1 {
            huff = huffman::decode(values.remaining())?;
            values = Reader::new(&huff);
        }
    }
    let mut bits = BitsIn::new(values.remaining());
    let (mut leading, mut trailing, mut window) = (0, 0, false);
    let (mut at, mut value) = (head.start, head.first.to_bits());
    let (mut word, mut word_left, mut word_bits) = (0u64, 0usize, 0u32);
    let mut out = Vec::with_capacity(head.count);
    for index in 0..head.count {
        if index > 0 {
            if time_mode == 1 {
                delta = times.unsigned()?;
            } else if time_mode == 2 {
                if index == 1 {
                    delta = times.unsigned()?;
                } else {
                    let change = unfold(times.unsigned()?);
                    if delta > i64::MAX as u64 {
                        return Err("timestamp delta overflow".into());
                    }
                    let next = (delta as i64)
                        .checked_add(change)
                        .ok_or("timestamp delta overflow")?;
                    if next <= 0 {
                        return Err("timestamp delta overflow".into());
                    }
                    delta = next as u64;
                }
            }
            if delta == 0 || delta > distance(at, i64::MAX) {
                return Err("timestamp ordering".into());
            }
            at = advance(at, delta);
            match value_mode {
                0 => value = values.word()?,
                1 => {}
                2 | 4 => {
                    let zigzag = if packing == 1 {
                        values.unsigned()?
                    } else {
                        if word_left == 0 {
                            if word != 0 {
                                return Err("integer word padding".into());
                            }
                            let stored = values.word()?;
                            let selector = (stored >> 60) as usize;
                            word = stored & ((1u64 << 60) - 1);
                            word_left = COUNTS[selector];
                            word_bits = WIDTHS[selector];
                            if word_bits == 0 && (word != 0 || word_left > head.count - index) {
                                return Err("integer run length".into());
                            }
                        }
                        let z = if word_bits == 0 {
                            1
                        } else {
                            let z = word & ((1u64 << word_bits) - 1);
                            word >>= word_bits;
                            z
                        };
                        word_left -= 1;
                        z
                    };
                    integer = integer
                        .checked_add(unfold(zigzag))
                        .ok_or("integer overflow")?;
                    let converted = integer as f64;
                    if scale.is_none()
                        && (converted >= (1u64 << 63) as f64 || converted as i64 != integer)
                    {
                        return Err("inexact integer".into());
                    }
                    value = (converted / scale.map_or(1.0, |s| POW10[s])).to_bits();
                }
                3 => {
                    if bits.get(1)? != 0 {
                        if bits.get(1)? != 0 {
                            leading = bits.get(5)? as u32;
                            let mut width = bits.get(6)? as u32;
                            if width == 0 {
                                width = 64;
                            }
                            if leading + width > 64 {
                                return Err("XOR window".into());
                            }
                            trailing = 64 - leading - width;
                            window = true;
                        } else if !window {
                            return Err("absent XOR window".into());
                        }
                        value ^= bits.get(64 - leading - trailing)? << trailing;
                    }
                }
                _ => unreachable!(),
            }
        }
        out.push(Sample {
            at,
            value: f64::from_bits(value),
        });
    }
    if at != head.end {
        return Err("last timestamp".into());
    }
    times.finish()?;
    if value_mode == 3 {
        bits.finish()?;
    } else {
        values.finish()?;
    }
    if (value_mode == 2 || value_mode == 4) && packing == 0 && word != 0 {
        return Err("integer word padding".into());
    }
    Ok(out)
}

pub(super) fn payload_stream(flags: u8, payload: &[u8], optimized: bool) -> Result<Cow<'_, [u8]>> {
    Ok(if flags >> 5 == 1 {
        Cow::Owned(binary::expand(payload, 8192)?)
    } else if optimized {
        Cow::Borrowed(payload)
    } else {
        Cow::Owned(payload.to_vec())
    })
}
