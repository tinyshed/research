//! Huff0 uses the zstd Huffman table and single-stream byte format. Native
//! compression/table parsing come from the pinned zstd dependency, while the
//! dynamic-length decode below follows huff0's canonical table construction.
use super::Result;
use std::cell::RefCell;
use std::ffi::{c_int, c_uint, c_void};

struct Scratch {
    workspace: Vec<u64>,
    table: [usize; 257],
}
thread_local! {static SCRATCH:RefCell<Scratch>=RefCell::new(Scratch{workspace:vec![0;1088],table:[0;257]});}

unsafe extern "C" {
    fn HUF_compress1X_repeat(
        dst: *mut c_void,
        dst_size: usize,
        src: *const c_void,
        src_size: usize,
        maximum: c_uint,
        table_log: c_uint,
        workspace: *mut c_void,
        workspace_size: usize,
        table: *mut usize,
        repeat: *mut c_int,
        flags: c_int,
    ) -> usize;
    fn HUF_readStats(
        weights: *mut u8,
        weight_size: usize,
        ranks: *mut u32,
        symbols: *mut u32,
        table_log: *mut u32,
        src: *const c_void,
        src_size: usize,
    ) -> usize;
    fn HUF_isError(code: usize) -> c_uint;
}

pub fn compress(symbols: &[u8]) -> Option<Vec<u8>> {
    if symbols.is_empty() {
        return None;
    }
    let mut out = vec![0u8; symbols.len() + 1024];
    let size = SCRATCH.with(|slot| {
        let mut scratch = slot.borrow_mut();
        let mut repeat = 0;
        let Scratch { workspace, table } = &mut *scratch;
        // Buffers are owned, correctly aligned, and sized per zstd's huf.h.
        unsafe {
            HUF_compress1X_repeat(
                out.as_mut_ptr().cast(),
                out.len(),
                symbols.as_ptr().cast(),
                symbols.len(),
                255,
                11,
                workspace.as_mut_ptr().cast(),
                workspace.len() * 8,
                table.as_mut_ptr(),
                &mut repeat,
                0,
            )
        }
    });
    if size <= 1 || size >= symbols.len() || unsafe { HUF_isError(size) } != 0 {
        return None;
    }
    out.truncate(size);
    Some(out)
}

pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    decode_with(data, crate::tuning::bits())
}

fn decode_with(data: &[u8], words: bool) -> Result<Vec<u8>> {
    if data.len() < 2 {
        return Err("huffman table too small".into());
    }
    let mut weights = [0u8; 256];
    let mut ranks = [0u32; 16];
    let mut symbols = 0;
    let mut log = 0;
    // HUF_readStats bounds the output to the 256-byte weight buffer and checks
    // both raw and FSE-compressed weight tables, including the implied symbol.
    let header = unsafe {
        HUF_readStats(
            weights.as_mut_ptr(),
            weights.len(),
            ranks.as_mut_ptr(),
            &mut symbols,
            &mut log,
            data.as_ptr().cast(),
            data.len(),
        )
    };
    if unsafe { HUF_isError(header) } != 0 || header >= data.len() || log > 11 || symbols > 256 {
        return Err("huffman table".into());
    }
    let mut next = 0u32;
    for weight in 1..=log as usize {
        let current = next;
        next += ranks[weight] << (weight - 1);
        ranks[weight] = current;
    }
    let mut table = vec![(0u8, 0u8); 1usize << log];
    for (symbol, &weight) in weights[..symbols as usize].iter().enumerate() {
        if weight == 0 {
            continue;
        }
        let length = 1usize << (weight - 1);
        let at = ranks[weight as usize] as usize;
        if at + length > table.len() {
            return Err("huffman table bounds".into());
        }
        table[at..at + length].fill((symbol as u8, (log + 1 - u32::from(weight)) as u8));
        ranks[weight as usize] += length as u32;
    }
    let stream = &data[header..];
    let last = *stream.last().unwrap();
    if last == 0 {
        return Err("huffman missing end marker".into());
    }
    let left = (stream.len() - 1) * 8 + (7 - last.leading_zeros()) as usize;
    if words {
        decode_word_stream(stream, &table, log as usize, left)
    } else {
        decode_scalar_stream(stream, &table, log as usize, left)
    }
}

fn decode_scalar_stream(
    stream: &[u8],
    table: &[(u8, u8)],
    log: usize,
    mut left: usize,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    while left > 0 {
        let mut code = 0usize;
        for offset in 0..log {
            code <<= 1;
            if offset < left {
                let position = left - 1 - offset;
                code |= ((stream[position / 8] >> (position % 8)) & 1) as usize;
            }
        }
        let (symbol, width) = table[code];
        if width == 0 || width as usize > left || out.len() == 8192 {
            return Err("huffman stream bounds".into());
        }
        left -= width as usize;
        out.push(symbol);
    }
    Ok(out)
}

/// Huff0 reads backwards, beginning immediately below the high end-marker bit.
/// The buffer's low `available` bits are the next bits to consume, with the
/// first bit at `available - 1`. Refill with complete little-endian words from
/// the preceding bytes, without ever loading past either end of the slice.
struct BackwardWords<'a> {
    stream: &'a [u8],
    position: usize,
    buffer: u64,
    available: usize,
}

impl<'a> BackwardWords<'a> {
    fn new(stream: &'a [u8], left: usize) -> Self {
        let position = stream.len().saturating_sub(8);
        let mut word = [0u8; 8];
        word[..stream.len() - position].copy_from_slice(&stream[position..]);
        Self {
            stream,
            position,
            buffer: u64::from_le_bytes(word),
            available: left - position * 8,
        }
    }

    #[inline]
    fn peek(&mut self, log: usize) -> usize {
        if self.available < log && self.position != 0 {
            // Huff0's table log is at most 11, so one refill always provides
            // enough lookahead unless the stream itself has fewer bits left.
            let bytes = ((64 - self.available) / 8).min(self.position);
            let mut word = [0u8; 8];
            word[..bytes].copy_from_slice(&self.stream[self.position - bytes..self.position]);
            let word = u64::from_le_bytes(word);
            self.buffer = if bytes == 8 {
                word
            } else {
                (self.buffer << (bytes * 8)) | word
            };
            self.available += bytes * 8;
            self.position -= bytes;
        }
        let mask = (1u64 << log) - 1;
        if self.available >= log {
            ((self.buffer >> (self.available - log)) & mask) as usize
        } else {
            // Zero lookahead beyond the beginning of the stream exactly as
            // the scalar decoder does. The selected width must still fit.
            ((self.buffer << (log - self.available)) & mask) as usize
        }
    }

    #[inline]
    fn consume(&mut self, width: usize) {
        self.available -= width;
    }
}

fn decode_word_stream(
    stream: &[u8],
    table: &[(u8, u8)],
    log: usize,
    mut left: usize,
) -> Result<Vec<u8>> {
    let mut bits = BackwardWords::new(stream, left);
    let mut out = Vec::new();
    while left > 0 {
        let (symbol, width) = table[bits.peek(log)];
        if width == 0 || width as usize > left || out.len() == 8192 {
            return Err("huffman stream bounds".into());
        }
        bits.consume(width as usize);
        left -= width as usize;
        out.push(symbol);
    }
    Ok(out)
}

pub(crate) fn kernel_bench(iterations: u64, warm: u64) -> Result<serde_json::Value> {
    use std::hint::black_box;
    use std::time::Instant;

    fn hash(bytes: &[u8]) -> u64 {
        bytes.iter().fold(14695981039346656037, |h, &byte| {
            (h ^ u64::from(byte)).wrapping_mul(1099511628211)
        })
    }

    if iterations == 0 {
        return Err("kernel iterations".into());
    }
    let samples = iterations.checked_mul(4096).ok_or("kernel sample count")?;
    let symbols: Vec<_> = (0..4096)
        .map(|i| {
            if i % 11 == 0 {
                (i % 255) as u8
            } else {
                (i % 3) as u8
            }
        })
        .collect();
    let data = compress(&symbols).ok_or("huffman kernel fixture compression")?;
    let checked = decode(&data)?;
    if checked != symbols {
        return Err("huffman kernel mismatch".into());
    }
    let input_hash = hash(&data);
    let output_hash = hash(&checked);
    for _ in 0..warm {
        black_box(decode(black_box(&data))?);
    }
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(decode(black_box(&data))?);
    }
    let elapsed = start.elapsed();
    Ok(serde_json::json!({
        "kernel": "huffman",
        "word_bits": crate::tuning::bits(),
        "iterations": iterations,
        "warm": warm,
        "values": symbols.len(),
        "input_bytes": data.len(),
        "input_hash": format!("{input_hash:016x}"),
        "output_hash": format!("{output_hash:016x}"),
        "samples": samples,
        "ns_per_op": elapsed.as_nanos() as f64 / iterations as f64,
    }))
}

#[cfg(test)]
mod word_tests {
    use super::*;

    fn random(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn marked_stream(state: &mut u64, left: usize) -> Vec<u8> {
        let mut stream: Vec<_> = (0..left / 8 + 1).map(|_| random(state) as u8).collect();
        let used = left % 8;
        *stream.last_mut().unwrap() &= ((1u16 << used) - 1) as u8;
        *stream.last_mut().unwrap() |= 1 << used;
        stream
    }

    #[test]
    fn backward_word_lookahead_matches_each_scalar_bit() {
        let mut state = 0x2fc9_b784_631d_e5a0;
        for log in 1..=11 {
            for size in (0..=256).chain([511, 512, 513, 1023, 1024, 1025, 8192, 8193]) {
                let stream = marked_stream(&mut state, size);
                let mut bits = BackwardWords::new(&stream, size);
                let mut left = size;
                while left != 0 {
                    let mut scalar = 0usize;
                    for offset in 0..log {
                        scalar <<= 1;
                        if offset < left {
                            let position = left - offset - 1;
                            scalar |= usize::from(stream[position / 8] >> (position % 8) & 1);
                        }
                    }
                    assert_eq!(
                        bits.peek(log),
                        scalar,
                        "log={log}, size={size}, left={left}"
                    );
                    let width = (1 + random(&mut state) as usize % log).min(left);
                    bits.consume(width);
                    left -= width;
                    assert_eq!(bits.position * 8 + bits.available, left);
                }
            }
        }
    }

    #[test]
    fn huffman_end_marker_tail_and_output_bounds_match() {
        // A raw Huff0 table with two weight-one symbols, followed by the
        // backwards stream. These bytes are independent of libzstd encoding.
        for (stream, expected) in [
            (vec![1], vec![]),
            (vec![2], vec![0]),
            (vec![3], vec![1]),
            (vec![0b0000_1101], vec![1, 0, 1]),
        ] {
            let mut data = vec![128, 0x10];
            data.extend(stream);
            assert_eq!(decode_with(&data, false).unwrap(), expected);
            assert_eq!(decode_with(&data, true).unwrap(), expected);
        }
        for data in [vec![], vec![128], vec![128, 0x10], vec![128, 0x10, 0]] {
            assert!(decode_with(&data, true).is_err());
            assert_eq!(decode_with(&data, true), decode_with(&data, false));
        }

        let mut state = 0x5298_6fcb_013a_ed47;
        let table = vec![(7, 1); 1 << 11];
        for left in [0, 1, 7, 8, 9, 63, 64, 65, 8191, 8192, 8193] {
            let stream = marked_stream(&mut state, left);
            let scalar = decode_scalar_stream(&stream, &table, 11, left);
            let words = decode_word_stream(&stream, &table, 11, left);
            assert_eq!(words, scalar, "left={left}");
            if left <= 8192 {
                assert_eq!(words.unwrap(), vec![7; left]);
            } else {
                assert_eq!(words, Err("huffman stream bounds".into()));
            }
        }
        let table = vec![(0, 3); 8];
        for left in [1, 2] {
            let stream = marked_stream(&mut state, left);
            assert_eq!(
                decode_word_stream(&stream, &table, 3, left),
                Err("huffman stream bounds".into())
            );
        }
        let stream = marked_stream(&mut state, 1);
        assert_eq!(
            decode_word_stream(&stream, &[(0, 0)], 0, 1),
            Err("huffman stream bounds".into())
        );
    }

    #[test]
    fn native_random_roundtrips_and_corruptions_match_scalar() {
        let mut state = 0xaac7_240d_e918_536b;
        let mut compressed = 0;
        for count in [2, 3, 7, 8, 9, 63, 64, 65, 239, 240, 600, 1024, 8192] {
            for alphabet in [2, 3, 7, 16, 64, 256] {
                let symbols: Vec<_> = (0..count)
                    .map(|_| {
                        let value = random(&mut state);
                        if value % 5 == 0 {
                            ((value >> 8) as usize % alphabet) as u8
                        } else {
                            (value & 1) as u8
                        }
                    })
                    .collect();
                let Some(data) = compress(&symbols) else {
                    continue;
                };
                compressed += 1;
                assert_eq!(decode_with(&data, false).unwrap(), symbols);
                assert_eq!(decode_with(&data, true).unwrap(), symbols);
                // A shorter or damaged stream can still be a valid, different
                // Huffman message. Check identical acceptance and output.
                for length in [0, 1, 2, data.len() / 2, data.len() - 1] {
                    assert_eq!(
                        decode_with(&data[..length], true),
                        decode_with(&data[..length], false),
                        "truncated count={count}, alphabet={alphabet}, length={length}"
                    );
                }
                for _ in 0..32 {
                    let mut damaged = data.clone();
                    let position = random(&mut state) as usize % data.len();
                    damaged[position] ^= 1 << (random(&mut state) % 8);
                    assert_eq!(
                        decode_with(&damaged, true),
                        decode_with(&damaged, false),
                        "damaged count={count}, alphabet={alphabet}, position={position}"
                    );
                }
                let mut no_marker = data.clone();
                *no_marker.last_mut().unwrap() = 0;
                assert_eq!(
                    decode_with(&no_marker, true),
                    Err("huffman missing end marker".into())
                );
            }
        }
        assert!(compressed >= 30, "compressed cases={compressed}");
    }
}
