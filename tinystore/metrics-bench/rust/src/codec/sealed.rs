use super::{
    Block, Group, Result, Sample, clocks, directory, distance, slots_mask, summaries, values,
};

pub fn encode_clock_group(group: &Group) -> Vec<u8> {
    clocks::encode_group(group)
}
pub fn decode_clock_group(body: &[u8]) -> Result<Vec<Block>> {
    clocks::decode_group(body)
}
pub fn write_directory(group: &Group) -> Result<Vec<u8>> {
    directory::write(group)
}
pub fn read_directory(
    series_id: i64,
    start: i64,
    end: i64,
    clock_id: i64,
    data: &[u8],
    clock: &[Block],
) -> Result<Group> {
    directory::read(series_id, start, end, clock_id, data, clock)
}
pub fn expanded_directory_size(group: &Group) -> usize {
    directory::expanded_size(group)
}
pub fn decode_block(block: &Block) -> Result<Vec<Sample>> {
    let times = clocks::decode_values(block)?;
    let mut points = values::decode(block)?;
    if times.len() != points.len() {
        return Err("value count".into());
    }
    for (p, at) in points.iter_mut().zip(times) {
        p.at = at;
    }
    Ok(points)
}

pub fn prepare_group(
    series_id: i64,
    points: &[Sample],
    kind: i32,
    model_scale: i32,
    max_span_ms: i64,
) -> Result<Group> {
    if points.is_empty() || max_span_ms <= 0 {
        return Err("empty packing candidate/block span".into());
    }
    let mut group = Group {
        series_id,
        model_scale,
        start: points[0].at,
        end: points.last().unwrap().at,
        ..Default::default()
    };
    let (mut clock_bytes, mut directory_bytes, mut start) = (6usize, 0usize, 0usize);
    while start < points.len() && group.blocks.len() < 32 {
        let mut end = (start + 240).min(points.len());
        while end > start + 1 && distance(points[start].at, points[end - 1].at) > max_span_ms as u64
        {
            end -= 1;
        }
        let part = &points[start..end];
        let clock = clocks::encode_values(part);
        if !group.blocks.is_empty() && clock_bytes + clock.len() + 32 > 65536 {
            break;
        }
        clock_bytes += clock.len() + 32;
        let (body, hint) = values::encode(part, group.model_scale)?;
        group.model_scale = hint;
        let mut block = Block {
            clock,
            summary: summaries::summarize(part, kind),
            head: super::Head {
                start: part[0].at,
                end: part.last().unwrap().at,
                count: part.len(),
                first: part[0].value,
            },
            ..Default::default()
        };
        block.body = values::seal(&block, body);
        block.body_bytes = block.body.len();
        let slot = group.blocks.len();
        let mut descriptor = Group {
            allocation: group.allocation,
            ..Default::default()
        };
        if block.body_bytes > 16 {
            descriptor.allocation |= 1u32 << slot;
        }
        let mut entry = Vec::new();
        directory::append_block(&mut entry, &descriptor, slot, &block);
        let entry_bytes = entry.len() + 9;
        if !group.blocks.is_empty() && entry_bytes > 8064 - directory_bytes {
            break;
        }
        directory_bytes += entry_bytes;
        if block.body_bytes > 16 {
            group.allocation |= 1u32 << slot;
        }
        group.blocks.push(block);
        start = end;
    }
    group.live = slots_mask(group.blocks.len());
    group.end = group.blocks.last().unwrap().head.end;
    group.clock_body = clocks::encode_group(&group);
    if group.clock_body.len() > 65536 {
        return Err("clock group size".into());
    }
    Ok(group)
}
