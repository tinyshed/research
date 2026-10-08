// Safe port of records/bits.go and the residual loops in records/ints.go at
// tinyshed/tinystore e307c48a40126aad0e2873b6bf3aaedef8115483.
// Each operation allocates a fresh output; Rice encoder scratch reuse is excluded.
// The planner, transforms, headers, radix packing and FSE are excluded.
use crate::{Case, Payload, fingerprint_bytes, fingerprint_words};
use std::hint::black_box;

#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    used: u32,
}

// Go shifts by 64 produce zero; Rust masks wrapping shifts, so the explicit
// branch preserves the source operation in debug and release builds.
fn shift_right(value: u64, width: u32) -> u64 {
    if width >= 64 { 0 } else { value >> width }
}

fn low_mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

impl BitWriter {
    fn write(&mut self, mut value: u64, mut width: u32) {
        while width > 0 {
            let take = width.min(64 - self.used);
            let mut part = value;
            if take < 64 {
                part &= low_mask(take);
            }
            self.acc |= part << self.used;
            self.used += take;
            value = shift_right(value, take);
            width -= take;
            if self.used == 64 {
                self.out.extend_from_slice(&self.acc.to_le_bytes());
                self.acc = 0;
                self.used = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        while self.used > 0 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.used -= self.used.min(8);
        }
        self.out
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    at: u64,
    short: bool,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            at: 0,
            short: false,
        }
    }

    fn read(&mut self, width: u32) -> u64 {
        let mut value = 0;
        let mut got = 0;
        while got < width {
            let index = self.at / 8;
            if index >= self.data.len() as u64 {
                self.short = true;
                return 0;
            }
            let shift = (self.at % 8) as u32;
            let take = (8 - shift).min(width - got);
            value |= (u64::from(self.data[index as usize] >> shift) & low_mask(take)) << got;
            got += take;
            self.at += u64::from(take);
        }
        value
    }

    fn consumed(&self) -> u64 {
        (self.at + 7) / 8
    }
}

const RICE_ESCAPE: u64 = 32;

fn write_rice(writer: &mut BitWriter, value: u64, k: u32) {
    let quotient = shift_right(value, k);
    if quotient < RICE_ESCAPE {
        writer.write(low_mask(quotient as u32), quotient as u32);
        writer.write(0, 1);
        writer.write(value, k);
        return;
    }
    writer.write(low_mask(RICE_ESCAPE as u32), RICE_ESCAPE as u32);
    writer.write(value, 64);
}

fn read_rice(reader: &mut BitReader<'_>, k: u32) -> u64 {
    let mut quotient = 0;
    while quotient < RICE_ESCAPE && reader.read(1) == 1 {
        quotient += 1;
    }
    if quotient == RICE_ESCAPE {
        return reader.read(64);
    }
    quotient << k | reader.read(k)
}

#[cfg(test)]
fn rice_bits(value: u64, k: u32) -> u64 {
    let quotient = shift_right(value, k);
    if quotient < RICE_ESCAPE {
        quotient + 1 + u64::from(k)
    } else {
        RICE_ESCAPE + 64
    }
}

#[cfg(test)]
fn distance(start: i64, end: i64) -> u64 {
    (end as u64).wrapping_sub(start as u64)
}

#[cfg(test)]
fn advance(start: i64, span: u64) -> i64 {
    (start as u64).wrapping_add(span) as i64
}

#[cfg(test)]
fn zigzag(value: i64) -> u64 {
    (value as u64).wrapping_shl(1) ^ ((value >> 63) as u64)
}

// Matched experimental algorithm; the Go reader loads the same little endian
// words, retains the same accumulator and advances only bits actually read.
struct WordReader<'a> {
    data: &'a [u8],
    next: usize,
    acc: u64,
    available: u32,
    at: u64,
    short: bool,
}

impl<'a> WordReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            next: 0,
            acc: 0,
            available: 0,
            at: 0,
            short: false,
        }
    }

    fn refill(&mut self) {
        let remaining = self.data.len() - self.next;
        if remaining >= 8 {
            self.acc = u64::from_le_bytes(self.data[self.next..self.next + 8].try_into().unwrap());
            self.next += 8;
            self.available = 64;
            return;
        }
        self.acc = 0;
        for i in 0..remaining {
            self.acc |= u64::from(self.data[self.next + i]) << (8 * i);
        }
        self.next += remaining;
        self.available = remaining as u32 * 8;
    }

    fn read(&mut self, width: u32) -> u64 {
        let mut value = 0;
        let mut got = 0;
        while got < width {
            if self.available == 0 {
                self.refill();
                if self.available == 0 {
                    self.short = true;
                    return 0;
                }
            }
            let take = (width - got).min(self.available);
            let mut part = self.acc;
            if take < 64 {
                part &= low_mask(take);
            }
            value |= part << got;
            self.acc = shift_right(self.acc, take);
            self.available -= take;
            self.at += u64::from(take);
            got += take;
        }
        value
    }

    fn consumed(&self) -> u64 {
        (self.at + 7) / 8
    }
}

fn read_rice_word(reader: &mut WordReader<'_>, k: u32) -> u64 {
    let mut quotient = 0;
    while quotient < RICE_ESCAPE && reader.read(1) == 1 {
        quotient += 1;
    }
    if quotient == RICE_ESCAPE {
        return reader.read(64);
    }
    quotient << k | reader.read(k)
}

fn encode_width(values: &[u64], width: u32) -> Vec<u8> {
    let mut writer = BitWriter::default();
    for &value in values {
        writer.write(value, width);
    }
    writer.finish()
}

fn decode_width(data: &[u8], count: usize, width: u32) -> (Vec<u64>, bool) {
    let mut reader = BitReader::new(data);
    let mut residuals = vec![0; count];
    for value in &mut residuals {
        *value = reader.read(width);
    }
    (residuals, reader.short)
}

fn decode_width_word(data: &[u8], count: usize, width: u32) -> (Vec<u64>, bool) {
    let mut reader = WordReader::new(data);
    let mut residuals = vec![0; count];
    for value in &mut residuals {
        *value = reader.read(width);
    }
    (residuals, reader.short)
}

fn encode_rice(values: &[u64], k: u32) -> Vec<u8> {
    let mut writer = BitWriter::default();
    for &value in values {
        write_rice(&mut writer, value, k);
    }
    writer.finish()
}

fn decode_rice(data: &[u8], count: usize, k: u32) -> (Vec<u64>, bool) {
    let mut reader = BitReader::new(data);
    let mut residuals = vec![0; count];
    for value in &mut residuals {
        *value = read_rice(&mut reader, k);
    }
    (
        residuals,
        reader.short || reader.consumed() != data.len() as u64,
    )
}

fn decode_rice_word(data: &[u8], count: usize, k: u32) -> (Vec<u64>, bool) {
    let mut reader = WordReader::new(data);
    let mut residuals = vec![0; count];
    for value in &mut residuals {
        *value = read_rice_word(&mut reader, k);
    }
    (
        residuals,
        reader.short || reader.consumed() != data.len() as u64,
    )
}

fn fixture_width(width: u32) -> Vec<u64> {
    let mut values = vec![0; 1024];
    let mut state = 0x243f6a8885a308d3u64;
    let mask = low_mask(width);
    for (i, target) in values.iter_mut().enumerate() {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        *target = value & mask;
        if i % 97 == 0 {
            *target = 0;
        } else if i % 97 == 1 {
            *target = mask;
        }
    }
    values
}

fn fixture_rice(escapes: bool) -> Vec<u64> {
    let small = [0, 1, 0, 2, 3, 5, 8, 13, 4, 0, 31, 7, 63, 15, 127, 0];
    let wide = [
        0,
        1,
        5,
        127,
        128,
        129,
        255,
        256,
        1024,
        65536,
        1 << 32,
        1 << 63,
        u64::MAX,
    ];
    let mut values = vec![0; 1024];
    for (i, target) in values.iter_mut().enumerate() {
        *target = if escapes {
            wide[i % wide.len()]
        } else {
            small[i % small.len()]
        };
        if i % 97 == 0 {
            *target = 0;
        }
    }
    values
}

fn words((values, invalid): (Vec<u64>, bool)) -> Payload {
    assert!(!invalid, "records benchmark input truncated");
    Payload::Words(values)
}

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for width in [1, 7, 17, 64] {
        let values = fixture_width(width);
        let packed = encode_width(&values, width);
        let prefix = format!("records/width_{width}/");
        let count = values.len();
        let encode_hash = fingerprint_words(&values, &[u64::from(width)]);
        let decode_hash = fingerprint_bytes(&packed, &[count as u64, u64::from(width)]);
        let packed_word = packed.clone();
        cases.push(Case {
            name: prefix.clone() + "encode",
            units: count,
            input_bytes: count * 8,
            input_hash: encode_hash,
            run: Box::new(move || {
                Payload::Bytes(encode_width(black_box(values.as_slice()), black_box(width)))
            }),
        });
        cases.push(Case {
            name: prefix.clone() + "decode",
            units: count,
            input_bytes: packed.len(),
            input_hash: decode_hash,
            run: Box::new(move || {
                words(decode_width(
                    black_box(packed.as_slice()),
                    black_box(count),
                    black_box(width),
                ))
            }),
        });
        cases.push(Case {
            name: prefix + "decode_word",
            units: count,
            input_bytes: packed_word.len(),
            input_hash: decode_hash,
            run: Box::new(move || {
                words(decode_width_word(
                    black_box(packed_word.as_slice()),
                    black_box(count),
                    black_box(width),
                ))
            }),
        });
    }
    for escapes in [false, true] {
        let values = fixture_rice(escapes);
        let packed = encode_rice(&values, 2);
        let prefix = if escapes {
            "records/rice_escapes/"
        } else {
            "records/rice_small/"
        };
        let count = values.len();
        let encode_hash = fingerprint_words(&values, &[2]);
        let decode_hash = fingerprint_bytes(&packed, &[count as u64, 2]);
        let packed_word = packed.clone();
        cases.push(Case {
            name: prefix.to_owned() + "encode",
            units: count,
            input_bytes: count * 8,
            input_hash: encode_hash,
            run: Box::new(move || {
                Payload::Bytes(encode_rice(black_box(values.as_slice()), black_box(2)))
            }),
        });
        cases.push(Case {
            name: prefix.to_owned() + "decode",
            units: count,
            input_bytes: packed.len(),
            input_hash: decode_hash,
            run: Box::new(move || {
                words(decode_rice(
                    black_box(packed.as_slice()),
                    black_box(count),
                    black_box(2),
                ))
            }),
        });
        cases.push(Case {
            name: prefix.to_owned() + "decode_word",
            units: count,
            input_bytes: packed_word.len(),
            input_hash: decode_hash,
            run: Box::new(move || {
                words(decode_rice_word(
                    black_box(packed_word.as_slice()),
                    black_box(count),
                    black_box(2),
                ))
            }),
        });
    }
    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_round_trip_and_truncation() {
        for width in [1, 7, 17, 64] {
            let values = fixture_width(width);
            let encoded = encode_width(&values, width);
            assert_eq!(encoded.len(), (values.len() * width as usize + 7) / 8);
            for decoder in [decode_width, decode_width_word] {
                let (decoded, invalid) = decoder(&encoded, values.len(), width);
                assert!(!invalid);
                assert_eq!(decoded, values);
                let (_, invalid) = decoder(&encoded[..encoded.len() - 1], values.len(), width);
                assert!(invalid);
            }
        }
        assert!(encode_width(&[u64::MAX], 0).is_empty());
        assert_eq!(decode_width(&[], 1, 0), (vec![0], false));
    }

    #[test]
    fn rice_round_trip_escape_and_truncation() {
        for escapes in [false, true] {
            let values = fixture_rice(escapes);
            let encoded = encode_rice(&values, 2);
            let total: u64 = values.iter().map(|&v| rice_bits(v, 2)).sum();
            assert_eq!(encoded.len() as u64, (total + 7) / 8);
            for decoder in [decode_rice, decode_rice_word] {
                let (decoded, invalid) = decoder(&encoded, values.len(), 2);
                assert!(!invalid);
                assert_eq!(decoded, values);
                let (_, invalid) = decoder(&encoded[..encoded.len() - 1], values.len(), 2);
                assert!(invalid);
                let mut trailing = encoded.clone();
                trailing.push(0);
                let (_, invalid) = decoder(&trailing, values.len(), 2);
                assert!(invalid);
            }
        }
        assert_eq!(rice_bits(127, 2), 34);
        assert_eq!(rice_bits(128, 2), 96);
        assert_eq!(rice_bits(u64::MAX, 2), 96);
    }

    #[test]
    fn shifts_by_64_and_mixed_alignment() {
        let widths = [1, 64, 7, 64, 17, 0, 64];
        let values = [
            1,
            u64::MAX,
            0x65,
            0x0123456789abcdef,
            0x1abcd,
            0,
            0xfedcba9876543210,
        ];
        let mut writer = BitWriter::default();
        for (&value, &width) in values.iter().zip(&widths) {
            writer.write(value, width);
        }
        let encoded = writer.finish();
        let mut reader = BitReader::new(&encoded);
        let mut word = WordReader::new(&encoded);
        for (&value, &width) in values.iter().zip(&widths) {
            assert_eq!(reader.read(width), value);
            assert_eq!(word.read(width), value);
            assert_eq!(reader.at, word.at);
        }
        assert!(!reader.short && !word.short);
        assert_eq!(reader.consumed(), word.consumed());
        assert_eq!(shift_right(u64::MAX, 64), 0);
    }

    #[test]
    fn failed_read_preserves_consumed_bits() {
        for len in 0..16 {
            let data = vec![0xa5; len];
            let mut reader = BitReader::new(&data);
            let mut word = WordReader::new(&data);
            for width in [7, 64, 1, 17, 0, 64] {
                assert_eq!(reader.read(width), word.read(width));
                assert_eq!(reader.at, word.at);
                assert_eq!(reader.short, word.short);
                assert_eq!(reader.consumed(), word.consumed());
            }
        }
    }

    #[test]
    fn signed_range_wrapping() {
        for start in [i64::MIN, -1, 0, 1, i64::MAX] {
            for end in [i64::MIN, -1, 0, 1, i64::MAX] {
                assert_eq!(advance(start, distance(start, end)), end);
            }
        }
        assert_eq!(distance(i64::MIN, i64::MAX), u64::MAX);
        assert_eq!(advance(i64::MAX, 1), i64::MIN);
        assert_eq!(zigzag(i64::MIN), u64::MAX);
        assert_eq!(zigzag(i64::MAX), u64::MAX - 1);
        assert_eq!(zigzag(-1), 1);
        assert_eq!(zigzag(1), 2);
    }
}
