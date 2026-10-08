use super::binary::{Reader, fold, put_unsigned, unfold};
use super::{Block, Group, Head, Result, Sample, advance, checksum, distance};
use std::collections::BTreeMap;

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
pub fn encode_values(points: &[Sample]) -> Vec<u8> {
    if points.len() < 2 {
        return Vec::new();
    }
    let deltas: Vec<u64> = points
        .windows(2)
        .map(|p| distance(p[0].at, p[1].at))
        .collect();
    let mut frequencies = BTreeMap::new();
    let mut unit = 0;
    for &delta in &deltas {
        unit = gcd(unit, delta);
        *frequencies.entry(delta).or_insert(0usize) += 1;
    }
    if frequencies.len() == 1 {
        return Vec::new();
    }
    let mut step = deltas[0];
    for (&delta, &count) in &frequencies {
        if count > frequencies[&step] || (count == frequencies[&step] && delta < step) {
            step = delta;
        }
    }
    let mut plain = vec![0];
    put_unsigned(&mut plain, unit);
    for &delta in &deltas {
        put_unsigned(&mut plain, delta / unit);
    }
    let mut runs = vec![1];
    put_unsigned(&mut runs, unit);
    let mut at = 0;
    while at < deltas.len() {
        let mut end = at + 1;
        while end < deltas.len() && deltas[end] == deltas[at] {
            end += 1;
        }
        put_unsigned(&mut runs, (end - at) as u64);
        put_unsigned(&mut runs, deltas[at] / unit);
        at = end;
    }
    let mut exceptions = vec![2];
    put_unsigned(&mut exceptions, unit);
    put_unsigned(&mut exceptions, step / unit);
    let mut previous = -1i64;
    for (i, &delta) in deltas.iter().enumerate() {
        if delta != step {
            put_unsigned(&mut exceptions, (i as i64 - previous) as u64);
            put_unsigned(&mut exceptions, delta / unit);
            previous = i as i64;
        }
    }
    let mut best = plain;
    if runs.len() < best.len() {
        best = runs;
    }
    if exceptions.len() < best.len() {
        best = exceptions;
    }
    best
}

pub fn decode_values(block: &Block) -> Result<Vec<i64>> {
    let head = block.head;
    if head.count < 1 || head.count > 240 || head.end < head.start {
        return Err("clock head".into());
    }
    if head.count == 1 {
        if !block.clock.is_empty() || head.start != head.end {
            return Err("singleton clock".into());
        }
        return Ok(vec![head.start]);
    }
    let mut deltas = vec![0u64; head.count - 1];
    if block.clock.is_empty() {
        let span = distance(head.start, head.end);
        let divisor = (head.count - 1) as u64;
        if span == 0 || span % divisor != 0 {
            return Err("regular clock step".into());
        }
        deltas.fill(span / divisor);
    } else {
        let mut r = Reader::new(&block.clock);
        let mode = r.byte()?;
        let unit = r.unsigned()?;
        match mode {
            0 => {
                for delta in &mut deltas {
                    *delta = r.unsigned()?;
                }
            }
            1 => {
                let mut at = 0;
                while at < deltas.len() {
                    let count = r.size(deltas.len() - at)?;
                    let delta = r.unsigned()?;
                    if count == 0 {
                        return Err("empty clock run".into());
                    }
                    deltas[at..at + count].fill(delta);
                    at += count;
                }
            }
            2 => {
                let step = r.unsigned()?;
                deltas.fill(step);
                let mut position = -1i64;
                while !r.remaining().is_empty() {
                    let gap = r.size((deltas.len() as i64 - 1 - position) as usize)?;
                    if gap == 0 {
                        return Err("clock exception position".into());
                    }
                    position += gap as i64;
                    deltas[position as usize] = r.unsigned()?;
                }
            }
            _ => return Err("clock mode".into()),
        }
        r.finish()?;
        if unit == 0 {
            return Err("clock quantum".into());
        }
        for delta in &mut deltas {
            *delta = delta.checked_mul(unit).ok_or("clock multiplication")?;
        }
    }
    let mut times = Vec::with_capacity(head.count);
    times.push(head.start);
    for delta in deltas {
        let at = *times.last().unwrap();
        if delta == 0 || delta > distance(at, i64::MAX) {
            return Err("clock ordering/overflow".into());
        }
        times.push(advance(at, delta));
    }
    if *times.last().unwrap() != head.end {
        return Err("clock endpoint".into());
    }
    Ok(times)
}

pub fn encode_group(group: &Group) -> Vec<u8> {
    let mut out = vec![1, group.blocks.len() as u8];
    for block in &group.blocks {
        put_unsigned(&mut out, fold(block.head.start));
        put_unsigned(&mut out, distance(block.head.start, block.head.end));
        put_unsigned(&mut out, block.head.count as u64);
        put_unsigned(&mut out, block.clock.len() as u64);
        out.extend_from_slice(&block.clock);
    }
    let crc = checksum(&[&out]);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}
pub fn decode_group(body: &[u8]) -> Result<Vec<Block>> {
    if body.len() < 6 || body.len() > 65536 {
        return Err("clock object size".into());
    }
    let end = body.len() - 4;
    let content = &body[..end];
    if checksum(&[content]) != u32::from_le_bytes(body[end..].try_into().unwrap()) {
        return Err("clock object checksum".into());
    }
    let mut r = Reader::new(content);
    if r.byte()? != 1 {
        return Err("clock version".into());
    }
    let count = r.byte()? as usize;
    if count == 0 || count > 32 {
        return Err("clock slots".into());
    }
    let mut blocks: Vec<Block> = Vec::with_capacity(count);
    for _ in 0..count {
        let start = unfold(r.unsigned()?);
        let span = r.unsigned()?;
        let samples = r.size(240)?;
        let length = r.size(2432)?;
        let clock = r.take(length)?.to_vec();
        if span > distance(start, i64::MAX) || samples == 0 {
            return Err("clock extent".into());
        }
        let end = advance(start, span);
        if end == i64::MAX || blocks.last().is_some_and(|b| start <= b.head.end) {
            return Err("clock block ordering".into());
        }
        let block = Block {
            head: Head {
                start,
                end,
                count: samples,
                first: 0.0,
            },
            clock,
            ..Default::default()
        };
        if block.clock.is_empty() {
            let span = distance(block.head.start, block.head.end);
            if (samples == 1 && span != 0)
                || (samples > 1 && (span == 0 || span % (samples as u64 - 1) != 0))
            {
                return Err("regular clock extent".into());
            }
        } else {
            decode_values(&block)?;
        }
        blocks.push(block);
    }
    r.finish()?;
    Ok(blocks)
}
