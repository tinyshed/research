use super::{Block, Group, Head, Result, Summary, binary, checksum, slots_mask, summaries};
use binary::{Reader, put_float, put_unsigned};

fn predictions(head: Head) -> [f64; 5] {
    [
        head.first,
        head.first,
        head.first,
        head.count as f64 * head.first,
        0.0,
    ]
}
pub fn append_block(raw: &mut Vec<u8>, group: &Group, slot: usize, block: &Block) {
    put_float(raw, block.head.first);
    let s = &block.summary;
    let values = [s.last, s.min, s.max, s.sum, s.increase];
    let mut flags = 0u8;
    if s.valid {
        flags |= 1 << 5;
    }
    if !s.exact_sum.is_empty() {
        flags |= 1 << 6;
    }
    for (i, prediction) in predictions(block.head).iter().enumerate() {
        if values[i].to_bits() == prediction.to_bits() {
            flags |= 1 << i;
        }
    }
    raw.push(flags);
    for (i, &value) in values.iter().enumerate() {
        if flags & (1 << i) == 0 {
            put_float(raw, value);
        }
    }
    put_unsigned(raw, s.resets as u64);
    if flags & (1 << 6) != 0 {
        raw.extend_from_slice(&s.exact_sum);
        raw.extend_from_slice(&s.exact_increase);
    }
    put_unsigned(raw, block.body_bytes as u64);
    if group.is_external(slot) {
        put_unsigned(raw, block.payload as u64);
    } else {
        raw.extend_from_slice(&block.body);
    }
}
pub fn expanded_size(group: &Group) -> usize {
    let mut out = Vec::new();
    for (slot, block) in group.blocks.iter().enumerate() {
        append_block(&mut out, group, slot, block);
    }
    out.len()
}
fn group_checksum(group: &Group, data: &[u8]) -> u32 {
    checksum(&[
        &group.series_id.to_le_bytes(),
        &group.start.to_le_bytes(),
        &group.end.to_le_bytes(),
        data,
    ])
}

pub fn write(group: &Group) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    for (slot, block) in group.blocks.iter().enumerate() {
        append_block(&mut raw, group, slot, block);
    }
    if raw.len() > 8192 {
        return Err("expanded directory".into());
    }
    let mut out = vec![4, group.blocks.len() as u8];
    out.extend_from_slice(&group.live.to_le_bytes());
    out.extend_from_slice(&group.allocation.to_le_bytes());
    out.extend_from_slice(&group.clock_id.to_le_bytes());
    out.extend(binary::metadata_encode(&raw)?);
    if out.len() + 4 > 8192 {
        return Err("directory size".into());
    }
    let crc = group_checksum(group, &out);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(out)
}
pub fn read(
    series_id: i64,
    start: i64,
    end: i64,
    clock_id: i64,
    data: &[u8],
    clock: &[Block],
) -> Result<Group> {
    let mut group = Group {
        series_id,
        start,
        end,
        clock_id,
        ..Default::default()
    };
    if data.len() < 23 || data.len() > 8192 || data[0] != 4 {
        return Err("directory version/size".into());
    }
    let content = &data[..data.len() - 4];
    if group_checksum(&group, content)
        != u32::from_le_bytes(data[data.len() - 4..].try_into().unwrap())
    {
        return Err("directory checksum".into());
    }
    let count = data[1] as usize;
    if count == 0 || count > 32 || count != clock.len() {
        return Err("directory clock slots".into());
    }
    group.live = u32::from_le_bytes(data[2..6].try_into().unwrap());
    group.allocation = u32::from_le_bytes(data[6..10].try_into().unwrap());
    let saved_clock = i64::from_le_bytes(data[10..18].try_into().unwrap());
    let beyond = !slots_mask(count);
    if clock_id <= 0
        || saved_clock != clock_id
        || group.live == 0
        || group.live & beyond != 0
        || group.allocation & beyond != 0
    {
        return Err("group references/masks".into());
    }
    let plain = binary::metadata_decode(&content[18..])?;
    let mut r = Reader::new(&plain);
    for (slot, template) in clock.iter().enumerate() {
        let mut block = template.clone();
        block.head.first = r.float()?;
        let flags = r.byte()?;
        if flags & 0x80 != 0 {
            return Err("summary flags".into());
        }
        let mut values = predictions(block.head);
        for (i, value) in values.iter_mut().enumerate() {
            if flags & (1 << i) == 0 {
                *value = r.float()?;
            }
        }
        let resets = r.size(block.head.count - 1)?;
        block.summary = Summary {
            last: values[0],
            min: values[1],
            max: values[2],
            sum: values[3],
            increase: values[4],
            resets: resets as u16,
            valid: flags & (1 << 5) != 0,
            ..Default::default()
        };
        if flags & 0x40 != 0 {
            block.summary.exact_sum = summaries::read_exact(&mut r)?;
            block.summary.exact_increase = summaries::read_exact(&mut r)?;
        }
        block.body_bytes = r.size(8200)?;
        if block.body_bytes > 0 && block.body_bytes < 6 {
            return Err("value body length".into());
        }
        if !group.is_external(slot) {
            if block.body_bytes > 16 {
                return Err("inline body length".into());
            }
            block.body = r.take(block.body_bytes)?.to_vec();
        } else {
            if block.body_bytes <= 16 {
                return Err("external body length".into());
            }
            let id = r.unsigned()?;
            if id == 0
                || id >= i64::MAX as u64
                || group.blocks.iter().any(|b| b.payload == id as i64)
            {
                return Err("external payload identifier".into());
            }
            block.payload = id as i64;
        }
        group.blocks.push(block);
    }
    r.finish()?;
    if group.blocks[0].head.start != start || group.blocks.last().unwrap().head.end != end {
        return Err("group temporal extent".into());
    }
    Ok(group)
}
