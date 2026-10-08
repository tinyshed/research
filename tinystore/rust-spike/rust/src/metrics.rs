use crate::{Case, Payload, Sample};

// Algorithm and validation port of tinyshed/tinystore at
// e307c48a40126aad0e2873b6bf3aaedef8115483, metrics/values_changes.go,
// metrics/residuals.go, metrics/binary.go and metrics/head.go.
// The exact source hashes are recorded beside the Go baseline in go/metrics.go.

const BLOCK_SAMPLES: usize = 240;
const VALUES_CHANGES: u8 = 1;
type DecodeResult<T> = Result<T, &'static str>;

fn change_values(points: &[Sample]) -> Vec<u8> {
    if constant_values(points) {
        return Vec::new();
    }
    let mut best: Option<Vec<u8>> = None;
    for kind in 0..3 {
        if let Some(candidate) = change_candidate(points, kind) {
            if best
                .as_ref()
                .is_none_or(|current| candidate.len() < current.len())
            {
                best = Some(candidate);
            }
        }
    }
    best.unwrap()
}

fn constant_values(points: &[Sample]) -> bool {
    points[1..]
        .iter()
        .all(|point| point.value.to_bits() == points[0].value.to_bits())
}

fn change_candidate(points: &[Sample], kind: u8) -> Option<Vec<u8>> {
    let integers = change_integers(points, kind)?;
    let mut out = vec![VALUES_CHANGES, kind];
    let mut previous = 0;
    for i in 1..points.len() {
        if points[i].value.to_bits() == points[i - 1].value.to_bits() {
            continue;
        }
        append_uvarint(&mut out, (i - previous) as u64);
        previous = i;
        if kind == 2 {
            out.extend_from_slice(&points[i].value.to_bits().to_le_bytes());
        } else {
            append_uvarint(&mut out, fold_signed(integers[i] - integers[i - 1]));
        }
    }
    Some(out)
}

fn change_integers(points: &[Sample], kind: u8) -> Option<Vec<i64>> {
    let mut integers = vec![0; points.len()];
    if kind == 2 {
        return Some(integers);
    }
    let factor = change_factor(kind);
    for (i, point) in points.iter().enumerate() {
        let q = (point.value * factor).round();
        if !q.is_finite() || q.abs() >= (1_u64 << 60) as f64 {
            return None;
        }
        integers[i] = q as i64;
        if (integers[i] as f64 / factor).to_bits() != point.value.to_bits() {
            return None;
        }
    }
    Some(integers)
}

fn change_factor(kind: u8) -> f64 {
    if kind == 1 { 100.0 } else { 1.0 }
}

fn read_changes(first: f64, count: usize, body: &[u8]) -> DecodeResult<Vec<Sample>> {
    let mut reader = BinaryReader::new(body);
    let kind = reader.byte()?;
    if kind > 2 {
        return Err("change representation");
    }
    let (mut value, mut integer) = first_change(first, kind)?;
    let mut out = vec![Sample { at: 0, value: 0.0 }; count];
    let mut next = reader.size(count as isize - 1)?;
    if next == 0 {
        return Err("first change position");
    }
    for (i, point) in out.iter_mut().enumerate() {
        if i == next {
            (value, integer) = read_change(&mut reader, kind, integer)?;
            next = next_change(&mut reader, count, i)?;
        }
        *point = Sample {
            at: i as i64,
            value,
        };
    }
    reader.finish()?;
    Ok(out)
}

fn first_change(first: f64, kind: u8) -> DecodeResult<(f64, i64)> {
    if kind == 2 {
        return Ok((first, 0));
    }
    let factor = change_factor(kind);
    let q = (first * factor).round();
    if !q.is_finite()
        || q.abs() >= (1_u64 << 60) as f64
        || (q as i64 as f64 / factor).to_bits() != first.to_bits()
    {
        return Err("first change value");
    }
    Ok((first, q as i64))
}

fn read_change(reader: &mut BinaryReader<'_>, kind: u8, integer: i64) -> DecodeResult<(f64, i64)> {
    if kind == 2 {
        return Ok((f64::from_bits(reader.word()?), integer));
    }
    let delta = unfold_signed(reader.unsigned()?);
    let integer = integer
        .checked_add(delta)
        .ok_or("change integer overflow")?;
    if integer <= -(1_i64 << 60) || integer >= 1_i64 << 60 {
        return Err("change integer range");
    }
    Ok((integer as f64 / change_factor(kind), integer))
}

fn next_change(reader: &mut BinaryReader<'_>, count: usize, i: usize) -> DecodeResult<usize> {
    if reader.data.is_empty() {
        return Ok(count);
    }
    let gap = reader.size((count - 1 - i) as isize)?;
    if gap == 0 {
        return Err("change position");
    }
    Ok(i + gap)
}

struct BinaryReader<'a> {
    data: &'a [u8],
}

impl<'a> BinaryReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    fn take(&mut self, size: usize) -> DecodeResult<&'a [u8]> {
        if size > self.data.len() {
            return Err("truncated binary field");
        }
        let (value, remaining) = self.data.split_at(size);
        self.data = remaining;
        Ok(value)
    }

    fn byte(&mut self) -> DecodeResult<u8> {
        Ok(self.take(1)?[0])
    }

    fn unsigned(&mut self) -> DecodeResult<u64> {
        let mut value = 0;
        for (i, &byte) in self.data.iter().take(10).enumerate() {
            if i == 9 && byte > 1 {
                return Err("invalid varint");
            }
            value |= ((byte & 0x7f) as u64) << (7 * i);
            if byte < 0x80 {
                self.data = &self.data[i + 1..];
                return Ok(value);
            }
        }
        Err("invalid varint")
    }

    fn size(&mut self, maximum: isize) -> DecodeResult<usize> {
        let value = self.unsigned()?;
        if maximum < 0 || value > maximum as u64 {
            return Err("binary size exceeds limit");
        }
        Ok(value as usize)
    }

    fn word(&mut self) -> DecodeResult<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn finish(self) -> DecodeResult<()> {
        if self.data.is_empty() {
            Ok(())
        } else {
            Err("trailing binary fields")
        }
    }
}

fn fold_signed(value: i64) -> u64 {
    (value as u64) << 1 ^ (value >> 63) as u64
}

fn unfold_signed(value: u64) -> i64 {
    (value >> 1) as i64 ^ -((value & 1) as i64)
}

fn append_uvarint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(value as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn packed_bits(values: &[u64], width: usize) -> Vec<u8> {
    let mut out = vec![0; (values.len() * width + 7) / 8];
    for (i, &value) in values.iter().enumerate() {
        for bit in 0..width {
            let position = i * width + bit;
            out[position / 8] |= ((value >> bit & 1) as u8) << (position % 8);
        }
    }
    out
}

fn validate_packed(data: &[u8], count: usize, width: usize) -> DecodeResult<()> {
    if width > 64 || count > BLOCK_SAMPLES || data.len() != (count * width + 7) / 8 {
        return Err("packed residual size");
    }
    let used = count * width % 8;
    if used != 0 && data[data.len() - 1] >> used != 0 {
        return Err("residual padding");
    }
    Ok(())
}

fn read_packed_bits(data: &[u8], count: usize, width: usize) -> DecodeResult<Vec<u64>> {
    validate_packed(data, count, width)?;
    let mut out = vec![0; count];
    for (i, value) in out.iter_mut().enumerate() {
        for bit in 0..width {
            let position = i * width + bit;
            *value |= ((data[position / 8] >> (position % 8) & 1) as u64) << bit;
        }
    }
    Ok(out)
}

fn packed_bits_chunked(values: &[u64], width: usize) -> Vec<u8> {
    let mut out = vec![0; (values.len() * width + 7) / 8];
    if width == 0 {
        return out;
    }
    let mask = if width < 64 {
        (1_u64 << width) - 1
    } else {
        u64::MAX
    };
    let (mut pending, mut used, mut position) = (0_u64, 0, 0);
    for &value in values {
        let value = value & mask;
        pending |= value << used;
        if used + width < 64 {
            used += width;
            continue;
        }
        out[position..position + 8].copy_from_slice(&pending.to_le_bytes());
        position += 8;
        let consumed = 64 - used;
        pending = if consumed == 64 { 0 } else { value >> consumed };
        used = width - consumed;
    }
    while used > 0 {
        out[position] = pending as u8;
        position += 1;
        pending >>= 8;
        used = used.saturating_sub(8);
    }
    out
}

fn read_packed_bits_chunked(data: &[u8], count: usize, width: usize) -> DecodeResult<Vec<u64>> {
    validate_packed(data, count, width)?;
    let mut out = vec![0; count];
    if width == 0 {
        return Ok(out);
    }
    let mask = if width < 64 {
        (1_u64 << width) - 1
    } else {
        u64::MAX
    };
    let (mut pending, mut used, mut position) = (0_u64, 0, 0);
    for value in &mut out {
        if used >= width {
            *value = pending & mask;
            pending = if width == 64 { 0 } else { pending >> width };
            used -= width;
            continue;
        }
        let take = 8.min(data.len() - position);
        let word = if take == 8 {
            u64::from_le_bytes(data[position..position + 8].try_into().unwrap())
        } else {
            let mut word = 0;
            for j in 0..take {
                word |= (data[position + j] as u64) << (j * 8);
            }
            word
        };
        *value = (pending | word << used) & mask;
        let needed = width - used;
        pending = if needed == 64 { 0 } else { word >> needed };
        used = take * 8 - needed;
        position += take;
    }
    Ok(out)
}

fn change_fixture(shape: &str) -> Vec<Sample> {
    let raw = [
        0,
        0x8000000000000000,
        0x7ff8000000000001,
        0x7ff8000000000042,
        0x7ff0000000000000,
        0xfff0000000000000,
        0x3ff0000000000001,
        0xbff0000000000001,
    ];
    (0..BLOCK_SAMPLES)
        .map(|i| Sample {
            at: i as i64,
            value: match shape {
                "constant" => f64::from_bits(0x8000000000000000),
                "sparse" => (5 + (i / 48) * 2) as f64,
                "many" => ((i * 37) % 1000) as f64 - 500.0,
                "decimal" => (1200 + (i / 3) % 31) as f64 / 100.0,
                "raw" => f64::from_bits(raw[(i / 2) % raw.len()]),
                _ => panic!("unknown metric shape"),
            },
        })
        .collect()
}

fn packed_fixture(width: usize) -> Vec<u64> {
    let mask = if width < 64 {
        (1_u64 << width) - 1
    } else {
        u64::MAX
    };
    let mut x = 0x9e3779b97f4a7c15_u64;
    (0..BLOCK_SAMPLES)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x & mask
        })
        .collect()
}

fn read_value_body(first: f64, count: usize, body: &[u8]) -> Vec<Sample> {
    if body.is_empty() {
        return (0..count)
            .map(|i| Sample {
                at: i as i64,
                value: first,
            })
            .collect();
    }
    read_changes(first, count, &body[1..]).unwrap()
}

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for shape in ["constant", "sparse", "many", "decimal", "raw"] {
        let points = change_fixture(shape);
        let body = change_values(&points);
        let (first, count) = (points[0].value, points.len());
        cases.push(Case {
            name: format!("metrics/change/{shape}/encode"),
            units: count,
            input_bytes: count * 16,
            input_hash: crate::fingerprint_samples(&points),
            run: Box::new(move || Payload::Bytes(change_values(std::hint::black_box(&points)))),
        });
        cases.push(Case {
            name: format!("metrics/change/{shape}/decode"),
            units: count,
            input_bytes: body.len() + 8,
            input_hash: crate::fingerprint_bytes(&body, &[first.to_bits(), count as u64]),
            run: Box::new(move || {
                Payload::Samples(read_value_body(
                    std::hint::black_box(first),
                    std::hint::black_box(count),
                    std::hint::black_box(&body),
                ))
            }),
        });
    }
    for width in [1, 7, 17, 64] {
        for variant in ["baseline", "chunked"] {
            let values = packed_fixture(width);
            let body = packed_bits(&values, width);
            let encode = if variant == "baseline" {
                packed_bits
            } else {
                packed_bits_chunked
            };
            let decode = if variant == "baseline" {
                read_packed_bits
            } else {
                read_packed_bits_chunked
            };
            cases.push(Case {
                name: format!("metrics/packed/{width}/{variant}/encode"),
                units: values.len(),
                input_bytes: values.len() * 8,
                input_hash: crate::fingerprint_words(&values, &[width as u64]),
                run: Box::new(move || {
                    Payload::Bytes(encode(
                        std::hint::black_box(&values),
                        std::hint::black_box(width),
                    ))
                }),
            });
            cases.push(Case {
                name: format!("metrics/packed/{width}/{variant}/decode"),
                units: BLOCK_SAMPLES,
                input_bytes: body.len(),
                input_hash: crate::fingerprint_bytes(&body, &[BLOCK_SAMPLES as u64, width as u64]),
                run: Box::new(move || {
                    Payload::Words(
                        decode(
                            std::hint::black_box(&body),
                            std::hint::black_box(BLOCK_SAMPLES),
                            std::hint::black_box(width),
                        )
                        .unwrap(),
                    )
                }),
            });
        }
    }
    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_preserve_bits_and_choose_modes() {
        for (shape, kind) in [("sparse", 0), ("many", 0), ("decimal", 1), ("raw", 2)] {
            let points = change_fixture(shape);
            let body = change_values(&points);
            assert_eq!(body[1], kind, "{shape}");
            let out = read_value_body(points[0].value, points.len(), &body);
            for (actual, expected) in out.iter().zip(&points) {
                assert_eq!(actual.at, expected.at);
                assert_eq!(actual.value.to_bits(), expected.value.to_bits(), "{shape}");
            }
        }
        let points = change_fixture("constant");
        assert!(change_values(&points).is_empty());
        assert!(
            read_value_body(points[0].value, points.len(), &[])
                .iter()
                .all(|point| point.value.to_bits() == 0x8000000000000000)
        );
    }

    #[test]
    fn change_example_and_integer_limits() {
        let points: Vec<_> = [5.0, 5.0, 5.0, 7.0, 7.0, 9.0]
            .into_iter()
            .enumerate()
            .map(|(i, value)| Sample {
                at: i as i64,
                value,
            })
            .collect();
        assert_eq!(change_values(&points), [1, 0, 3, 4, 2, 4]);
        assert!(first_change(-0.0, 0).is_err());
        assert!(first_change((1_u64 << 60) as f64, 0).is_err());
        let mut reader = BinaryReader::new(&[2]);
        assert!(read_change(&mut reader, 0, (1_i64 << 60) - 1).is_err());
        let mut bytes = Vec::new();
        append_uvarint(&mut bytes, fold_signed(i64::MAX));
        assert!(read_change(&mut BinaryReader::new(&bytes), 0, 1).is_err());
    }

    #[test]
    fn change_reader_rejects_malformed_data() {
        for body in [
            &[][..],
            &[3],
            &[0, 0],
            &[0, 3],
            &[0, 1],
            &[2, 1, 0],
            &[0, 1, 2, 0],
        ] {
            assert!(read_changes(0.0, 3, body).is_err(), "{body:?}");
        }
        assert!(read_changes(f64::NAN, 3, &[0, 1, 2]).is_err());
        assert!(read_changes(0.0, 3, &[0, 1, 2, 0x80]).is_err());
    }

    #[test]
    fn all_packed_widths_and_tail_lengths_match() {
        for width in 0..=64 {
            for count in [0, 1, 2, 7, 9, 63, 64, 65, 239, 240] {
                let values = packed_fixture(width);
                let values = &values[..count];
                let baseline = packed_bits(values, width);
                let chunked = packed_bits_chunked(values, width);
                assert_eq!(chunked, baseline, "width={width} count={count}");
                assert_eq!(read_packed_bits(&baseline, count, width).unwrap(), values);
                assert_eq!(
                    read_packed_bits_chunked(&baseline, count, width).unwrap(),
                    values
                );
            }
        }
    }

    #[test]
    fn packed_readers_reject_truncation_padding_and_bounds() {
        for decode in [read_packed_bits, read_packed_bits_chunked] {
            assert!(decode(&[], 1, 1).is_err());
            assert!(decode(&[0, 0], 1, 1).is_err());
            assert!(decode(&[2], 1, 1).is_err());
            assert!(decode(&[], 0, 65).is_err());
            assert!(decode(&[], 241, 0).is_err());
            assert_eq!(decode(&[], 240, 0).unwrap(), vec![0; 240]);
        }
    }
}
