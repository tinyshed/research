use super::{Result, binary};
use binary::{Reader, fold, put_unsigned, unfold};

fn packed(values: &[u64], width: usize) -> Vec<u8> {
    let mut out = vec![0; (values.len() * width + 7) / 8];
    for (i, &value) in values.iter().enumerate() {
        for bit in 0..width {
            let position = i * width + bit;
            out[position / 8] |= ((value >> bit & 1) as u8) << (position % 8);
        }
    }
    out
}
fn read_packed(data: &[u8], count: usize, width: usize) -> Result<Vec<u64>> {
    if width > 64 || count > 240 || data.len() != (count * width + 7) / 8 {
        return Err("packed residual size".into());
    }
    let used = count * width % 8;
    if used != 0 && data[data.len() - 1] >> used != 0 {
        return Err("residual padding".into());
    }
    let mut out = vec![0; count];
    for (i, value) in out.iter_mut().enumerate() {
        for bit in 0..width {
            let position = i * width + bit;
            *value |= u64::from(data[position / 8] >> (position % 8) & 1) << bit;
        }
    }
    Ok(out)
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
