use super::{Result, binary};
use binary::{Reader, fold, put_unsigned, unfold};

fn packed(values: &[u64], width: usize) -> Vec<u8> {
    if crate::tuning::bits() {
        packed_words(values, width)
    } else {
        packed_scalar(values, width)
    }
}

fn packed_scalar(values: &[u64], width: usize) -> Vec<u8> {
    let mut out = vec![0; (values.len() * width + 7) / 8];
    for (i, &value) in values.iter().enumerate() {
        for bit in 0..width {
            let position = i * width + bit;
            out[position / 8] |= ((value >> bit & 1) as u8) << (position % 8);
        }
    }
    out
}

/// The format concatenates each value's low bits in little-endian order.
/// Keep a partial word between values and write full words only when the
/// destination contains all eight bytes. No unaligned or unchecked loads are
/// needed, including at width 64 and the final partial byte.
fn packed_words(values: &[u64], width: usize) -> Vec<u8> {
    debug_assert!(width <= 64);
    let mut out = vec![0; (values.len() * width + 7) / 8];
    if width == 0 {
        return out;
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let mut buffer = 0u64;
    let mut used = 0;
    let mut position = 0;
    for &value in values {
        let value = value & mask;
        buffer |= value << used;
        let next = used + width;
        if next >= 64 {
            out[position..position + 8].copy_from_slice(&buffer.to_le_bytes());
            position += 8;
            used = next - 64;
            buffer = if used == 0 {
                0
            } else {
                value >> (width - used)
            };
        } else {
            used = next;
        }
    }
    if used != 0 {
        out[position..].copy_from_slice(&buffer.to_le_bytes()[..used.div_ceil(8)]);
    }
    out
}

fn read_packed(data: &[u8], count: usize, width: usize) -> Result<Vec<u64>> {
    read_packed_checked(data, count, width, crate::tuning::bits())
}

fn read_packed_checked(data: &[u8], count: usize, width: usize, words: bool) -> Result<Vec<u64>> {
    if width > 64 || count > 240 || data.len() != (count * width + 7) / 8 {
        return Err("packed residual size".into());
    }
    let used = count * width % 8;
    if used != 0 && data[data.len() - 1] >> used != 0 {
        return Err("residual padding".into());
    }
    Ok(if words {
        read_packed_words(data, count, width)
    } else {
        read_packed_scalar(data, count, width)
    })
}

fn read_packed_scalar(data: &[u8], count: usize, width: usize) -> Vec<u64> {
    let mut out = vec![0; count];
    for (i, value) in out.iter_mut().enumerate() {
        for bit in 0..width {
            let position = i * width + bit;
            *value |= u64::from(data[position / 8] >> (position % 8) & 1) << bit;
        }
    }
    out
}

fn read_packed_words(data: &[u8], count: usize, width: usize) -> Vec<u64> {
    let mut out = vec![0; count];
    if width == 0 {
        return out;
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let mut buffer = 0u64;
    let mut available = 0;
    let mut position = 0;
    for value in &mut out {
        if width <= available {
            *value = buffer & mask;
            buffer = if width == 64 { 0 } else { buffer >> width };
            available -= width;
        } else {
            // The size check guarantees the next word contains every bit
            // needed by this value. Zero-fill only the final word's tail.
            let low = buffer;
            let low_width = available;
            let bytes = (data.len() - position).min(8);
            let mut word = [0u8; 8];
            word[..bytes].copy_from_slice(&data[position..position + bytes]);
            buffer = u64::from_le_bytes(word);
            position += bytes;
            let high_width = width - low_width;
            *value = (low | (buffer << low_width)) & mask;
            buffer = if high_width == 64 {
                0
            } else {
                buffer >> high_width
            };
            available = bytes * 8 - high_width;
        }
    }
    out
}
fn consider(best: &mut Vec<u8>, candidate: Vec<u8>) {
    if candidate.len() < best.len() {
        *best = candidate;
    }
}

pub fn encode(residuals: &[i64]) -> Result<Vec<u8>> {
    let values: Vec<u64> = residuals.iter().map(|&v| fold(v)).collect();
    let positions: Vec<usize> = residuals
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| if v != 0 { Some(i) } else { None })
        .collect();
    let nonzero: Vec<u64> = positions.iter().map(|&i| values[i]).collect();
    let union = values.iter().fold(0u64, |a, b| a | b);
    if nonzero.is_empty() {
        return Ok(vec![0]);
    }
    let mut best = vec![5];
    for &value in &values {
        put_unsigned(&mut best, value);
    }
    let compressed = binary::metadata_encode(&best[1..])?;
    if compressed[0] == 1 {
        let mut candidate = vec![6];
        candidate.extend_from_slice(&compressed[1..]);
        consider(&mut best, candidate);
    }
    let width = 64 - union.leading_zeros() as usize;
    let mut candidate = vec![3, width as u8];
    candidate.extend(packed(&values, width));
    consider(&mut best, candidate);
    let mut bitmap = vec![0; (values.len() + 7) / 8];
    for &i in &positions {
        bitmap[i / 8] |= 1 << (i % 8);
    }
    let mut candidate = vec![2, width as u8];
    candidate.extend_from_slice(&bitmap);
    candidate.extend(packed(&nonzero, width));
    consider(&mut best, candidate);
    if union <= 3
        && positions
            .iter()
            .all(|&i| residuals[i] == 1 || residuals[i] == -1)
    {
        let signs: Vec<u64> = positions
            .iter()
            .map(|&i| u64::from(residuals[i] < 0))
            .collect();
        let mut candidate = vec![1];
        candidate.extend_from_slice(&bitmap);
        candidate.extend(packed(&signs, 1));
        consider(&mut best, candidate);
    }
    let mut sparse = vec![4];
    let mut previous = -1i64;
    for &i in &positions {
        put_unsigned(&mut sparse, (i as i64 - previous) as u64);
        put_unsigned(&mut sparse, values[i]);
        previous = i as i64;
    }
    consider(&mut best, sparse);
    Ok(best)
}

pub fn decode(data: &[u8], count: usize) -> Result<Vec<i64>> {
    if count >= 240 || data.is_empty() || data.len() > 8192 {
        return Err("residual bounds".into());
    }
    let mut reader = Reader::new(data);
    let mut mode = reader.byte()?;
    let expanded;
    if mode == 6 {
        let mut body = vec![1];
        body.extend_from_slice(reader.remaining());
        expanded = binary::metadata_decode(&body)?;
        reader = Reader::new(&expanded);
        mode = 5;
    }
    let mut out = vec![0; count];
    match mode {
        0 => {}
        5 => {
            for value in &mut out {
                *value = unfold(reader.unsigned()?);
            }
        }
        4 => {
            let mut position = -1i64;
            while !reader.remaining().is_empty() {
                let gap = reader.size((count as i64 - 1 - position) as usize)?;
                if gap == 0 {
                    return Err("residual position".into());
                }
                position += gap as i64;
                out[position as usize] = unfold(reader.unsigned()?);
            }
        }
        3 => {
            let width = reader.byte()? as usize;
            let data = reader.take(reader.remaining().len())?;
            for (target, value) in out.iter_mut().zip(read_packed(data, count, width)?) {
                *target = unfold(value);
            }
        }
        1 | 2 => {
            let width = if mode == 2 {
                reader.byte()? as usize
            } else {
                1
            };
            let bitmap = reader.take((count + 7) / 8)?;
            let used = count % 8;
            if used != 0 && bitmap[bitmap.len() - 1] >> used != 0 {
                return Err("residual bitmap padding".into());
            }
            let nonzero = bitmap.iter().map(|b| b.count_ones() as usize).sum();
            let data = reader.take(reader.remaining().len())?;
            let values = read_packed(data, nonzero, width)?;
            let mut position = 0;
            for (i, target) in out.iter_mut().enumerate() {
                if bitmap[i / 8] & (1 << (i % 8)) != 0 {
                    let value = values[position];
                    position += 1;
                    *target = if mode == 2 {
                        unfold(value)
                    } else if value == 1 {
                        -1
                    } else {
                        1
                    };
                }
            }
        }
        _ => return Err("residual representation".into()),
    }
    reader.finish()?;
    Ok(out)
}

/// Isolate packed-bit decoding from SQLite, query planning, and model choice.
/// Fixture generation, correctness checks, and checksums are outside the timed
/// loop; each decoded allocation is consumed through `black_box`.
pub(crate) fn kernel_bench(iterations: u64, warm: u64) -> Result<serde_json::Value> {
    use std::hint::black_box;
    use std::time::Instant;

    fn hash(bytes: impl IntoIterator<Item = u8>) -> u64 {
        bytes.into_iter().fold(14695981039346656037, |h, byte| {
            (h ^ u64::from(byte)).wrapping_mul(1099511628211)
        })
    }

    if iterations == 0 {
        return Err("kernel iterations".into());
    }
    let samples = iterations.checked_mul(239).ok_or("kernel sample count")?;
    let mut cases = Vec::new();
    for width in [1, 2, 5, 8, 13, 32, 63, 64] {
        let mask = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let values: Vec<_> = (0..239u64)
            .map(|i| {
                if i % 7 == 0 {
                    mask
                } else {
                    (i.wrapping_mul(0x9e37_79b9_7f4a_7c15)
                        .rotate_left((i % 64) as u32)
                        ^ 0x62fd_b851_c7ea_9034)
                        & mask
                }
            })
            .collect();
        let data = packed_scalar(&values, width);
        let checked = read_packed(&data, 239, width)?;
        if checked != values {
            return Err("residual kernel mismatch".into());
        }
        let input_hash = hash(data.iter().copied());
        let output_hash = hash(checked.iter().flat_map(|value| value.to_le_bytes()));
        for _ in 0..warm {
            black_box(read_packed(black_box(&data), 239, width)?);
        }
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(read_packed(black_box(&data), 239, width)?);
        }
        let elapsed = start.elapsed();
        cases.push(serde_json::json!({
            "width": width,
            "values": 239,
            "input_bytes": data.len(),
            "input_hash": format!("{input_hash:016x}"),
            "output_hash": format!("{output_hash:016x}"),
            "samples": samples,
            "ns_per_op": elapsed.as_nanos() as f64 / iterations as f64,
        }));
    }
    Ok(serde_json::json!({
        "kernel": "packed_residuals",
        "word_bits": crate::tuning::bits(),
        "iterations": iterations,
        "warm": warm,
        "cases": cases,
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

    #[test]
    fn every_width_and_count_matches_scalar_bits() {
        let mut state = 0xe4b8_a901_6c75_2d3f;
        for width in 0..=64 {
            let mask = if width == 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            for count in 0..=240 {
                let values: Vec<_> = (0..count)
                    .map(|i| match i % 5 {
                        0 => 0,
                        1 => u64::MAX,
                        2 => 1u64 << (i % 64),
                        _ => random(&mut state),
                    })
                    .collect();
                let scalar = packed_scalar(&values, width);
                let words = packed_words(&values, width);
                assert_eq!(words, scalar, "width={width}, count={count}");
                let expected: Vec<_> = values.iter().map(|value| value & mask).collect();
                assert_eq!(
                    read_packed_checked(&words, count, width, true).unwrap(),
                    expected,
                    "word width={width}, count={count}"
                );
                assert_eq!(
                    read_packed_checked(&words, count, width, false).unwrap(),
                    expected,
                    "scalar width={width}, count={count}"
                );
            }
        }
    }

    #[test]
    fn word_reader_preserves_size_padding_and_count_guards() {
        for width in 0..=64 {
            for count in [0, 1, 2, 7, 8, 9, 63, 64, 65, 238, 239, 240] {
                let values = vec![u64::MAX; count];
                let data = packed_words(&values, width);
                for words in [false, true] {
                    if !data.is_empty() {
                        assert!(
                            read_packed_checked(&data[..data.len() - 1], count, width, words)
                                .is_err()
                        );
                    }
                    let mut longer = data.clone();
                    longer.push(0);
                    assert!(read_packed_checked(&longer, count, width, words).is_err());
                    let used = count * width % 8;
                    if used != 0 {
                        let mut padding = data.clone();
                        *padding.last_mut().unwrap() |= 1 << used;
                        assert_eq!(
                            read_packed_checked(&padding, count, width, words),
                            Err("residual padding".into())
                        );
                    }
                }
            }
        }
        for words in [false, true] {
            for count in [241, usize::MAX] {
                assert!(read_packed_checked(&[], count, 0, words).is_err());
            }
            for width in [65, usize::MAX] {
                assert!(read_packed_checked(&[], 0, width, words).is_err());
            }
            assert!(decode(&[0], 240).is_err());
            assert_eq!(decode(&[0], 239).unwrap(), vec![0; 239]);
        }
    }

    #[test]
    fn hand_written_little_endian_vectors_match_go_layout() {
        for (values, width, bytes) in [
            (vec![0, 2, 1, 4, 3], 3, vec![0x50, 0x38]),
            (vec![0, 1, 0], 1, vec![0x02]),
            (
                vec![0x0123_4567_89ab_cdef, u64::MAX],
                64,
                vec![
                    0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff,
                ],
            ),
        ] {
            assert_eq!(packed_words(&values, width), bytes);
            assert_eq!(
                read_packed_checked(&bytes, values.len(), width, true).unwrap(),
                values
            );
        }
    }
}
