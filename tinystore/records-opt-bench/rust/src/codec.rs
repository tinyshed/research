// Experimental compatible v1 head codec and restricted v1 segment codec.
// The writer always chooses direct-width integers and raw-attribute shapes.
// Arbitrary production FSE/stamped/typed attribute columns are NOT implemented.
pub mod optimized;
use crate::{Field, Record, Result};
use std::collections::HashMap;
use std::io::Read;
pub const MAX_BLOCK: usize = 256 << 10;
pub fn uv(b: &mut Vec<u8>, mut n: u64) {
    while n >= 128 {
        b.push(n as u8 | 128);
        n >>= 7
    }
    b.push(n as u8)
}
pub fn vi(b: &mut Vec<u8>, n: i64) {
    uv(b, ((n as u64) << 1) ^ ((n >> 63) as u64))
}
pub fn string(b: &mut Vec<u8>, s: &str) {
    uv(b, s.len() as u64);
    b.extend_from_slice(s.as_bytes())
}
pub fn strings(b: &mut Vec<u8>, ss: &[String]) {
    uv(b, ss.len() as u64);
    for s in ss {
        string(b, s)
    }
}
pub fn fields(b: &mut Vec<u8>, fs: &[Field]) {
    uv(b, fs.len() as u64);
    for f in fs {
        string(b, &f.key);
        string(b, &f.value)
    }
}
pub struct Cursor<'a> {
    pub data: &'a [u8],
}
impl<'a> Cursor<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.data.len() {
            return Err("truncated bytes".into());
        };
        let (v, r) = self.data.split_at(n);
        self.data = r;
        Ok(v)
    }
    pub fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn uv(&mut self) -> Result<u64> {
        let mut v = 0;
        for i in 0..10 {
            let c = self.byte()?;
            if i == 9 && c > 1 {
                return Err("varint overflow".into());
            };
            v |= u64::from(c & 127) << (7 * i);
            if c < 128 {
                return Ok(v);
            }
        }
        Err("varint overflow".into())
    }
    pub fn vi(&mut self) -> Result<i64> {
        let v = self.uv()?;
        Ok(((v >> 1) as i64) ^ (-((v & 1) as i64)))
    }
    pub fn count(&mut self, n: usize) -> Result<usize> {
        let v = self.uv()?;
        if v > n as u64 {
            return Err("count over bound".into());
        };
        Ok(v as usize)
    }
    pub fn string(&mut self) -> Result<String> {
        let n = self.count(MAX_BLOCK)?;
        Ok(String::from_utf8(self.take(n)?.to_vec())?)
    }
    pub fn strings(&mut self, n: usize) -> Result<Vec<String>> {
        let n = self.count(n)?;
        (0..n).map(|_| self.string()).collect()
    }
    pub fn fields(&mut self) -> Result<Vec<Field>> {
        let n = self.count(128)?;
        (0..n)
            .map(|_| {
                Ok(Field {
                    key: self.string()?,
                    value: self.string()?,
                })
            })
            .collect()
    }
    pub fn finish(&self) -> Result<()> {
        if self.data.is_empty() {
            Ok(())
        } else {
            Err("trailing bytes".into())
        }
    }
}
pub fn crc(b: &mut Vec<u8>) {
    let n = crc32fast::hash(b);
    b.extend_from_slice(&n.to_le_bytes())
}
fn checked(b: &[u8]) -> Result<&[u8]> {
    if b.len() < 5 || b[0] != 1 {
        return Err("row version".into());
    };
    let end = b.len() - 4;
    if crc32fast::hash(&b[..end]) != u32::from_le_bytes(b[end..].try_into()?) {
        return Err("row checksum".into());
    };
    Ok(&b[1..end])
}
pub fn presence(r: &Record) -> u8 {
    u8::from(r.level.is_some())
        | u8::from(r.body.is_some()) << 1
        | u8::from(r.trace != [0; 16]) << 2
        | u8::from(r.span != [0; 8]) << 3
        | u8::from(!r.context.is_empty()) << 4
}
pub fn serialize_record(b: &mut Vec<u8>, r: &Record) {
    vi(b, r.at);
    string(b, &r.name);
    let p = presence(r);
    b.push(p);
    if let Some(l) = r.level {
        vi(b, i64::from(l))
    };
    if let Some(s) = &r.body {
        string(b, s)
    };
    if p & 4 != 0 {
        b.extend_from_slice(&r.trace)
    };
    if p & 8 != 0 {
        b.extend_from_slice(&r.span)
    };
    fields(b, &r.context);
    fields(b, &r.attrs)
}
pub fn read_record(c: &mut Cursor, stream: &str) -> Result<Record> {
    let at = c.vi()?;
    let name = c.string()?;
    let p = c.byte()?;
    if p & !31 != 0 {
        return Err("head presence".into());
    };
    let level = if p & 1 != 0 {
        Some(i32::try_from(c.vi()?)?)
    } else {
        None
    };
    let body = if p & 2 != 0 { Some(c.string()?) } else { None };
    let trace = if p & 4 != 0 {
        c.take(16)?.try_into()?
    } else {
        [0; 16]
    };
    let span = if p & 8 != 0 {
        c.take(8)?.try_into()?
    } else {
        [0; 8]
    };
    let context = c.fields()?;
    let attrs = c.fields()?;
    if (p & 16 != 0) != !context.is_empty() {
        return Err("context presence".into());
    };
    Ok(Record {
        at,
        stream: stream.into(),
        name,
        level,
        body,
        trace,
        span,
        context,
        attrs,
    })
}
pub fn decompress(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut decoder = zstd::stream::read::Decoder::new(data)?;
    decoder.window_log_max(18)?;
    let mut out = Vec::new();
    decoder.take((limit + 1) as u64).read_to_end(&mut out)?;
    if out.len() > limit {
        return Err("decompressed data over bound".into());
    };
    Ok(out)
}
pub fn encode_head(rs: &[Record]) -> Result<Vec<u8>> {
    if rs.is_empty() || rs.len() > 1024 {
        return Err("head count".into());
    };
    let mut raw = Vec::new();
    for r in rs {
        serialize_record(&mut raw, r)
    }
    if raw.len() > 4 << 20 {
        return Err("head expansion".into());
    };
    let mut b = vec![1];
    uv(&mut b, rs.len() as u64);
    let mut compressor = zstd::bulk::Compressor::new(3)?;
    compressor.set_parameter(zstd::zstd_safe::CParameter::WindowLog(18))?;
    compressor.set_parameter(zstd::zstd_safe::CParameter::ChecksumFlag(false))?;
    b.extend_from_slice(&compressor.compress(&raw)?);
    crc(&mut b);
    Ok(b)
}
pub fn decode_head(stream: &str, b: &[u8]) -> Result<Vec<Record>> {
    let mut c = Cursor { data: checked(b)? };
    let count = c.count(1024)?;
    if count == 0 {
        return Err("empty head".into());
    };
    let raw = decompress(c.data, 4 << 20)?;
    c = Cursor { data: &raw };
    let rs = (0..count)
        .map(|_| read_record(&mut c, stream))
        .collect::<Result<Vec<_>>>()?;
    c.finish()?;
    Ok(rs)
}
pub fn input(r: &Record) -> usize {
    32 + r.stream.len()
        + r.name.len()
        + r.body.as_ref().map_or(0, |s| s.len())
        + if r.trace != [0; 16] { 16 } else { 0 }
        + if r.span != [0; 8] { 8 } else { 0 }
        + r.context
            .iter()
            .chain(&r.attrs)
            .map(|f| 4 + f.key.len() + f.value.len())
            .sum::<usize>()
}
fn width_write(values: &[u64], width: u32) -> Vec<u8> {
    let mut b = vec![0; ((values.len() * width as usize) + 7) / 8];
    let mut at = 0;
    for &v in values {
        for i in 0..width {
            if v & (1 << i) != 0 {
                b[at / 8] |= 1 << (at % 8)
            };
            at += 1
        }
    }
    b
}
fn width_read(c: &mut Cursor, n: usize, width: u32) -> Result<Vec<u64>> {
    if width > 64 {
        return Err("width".into());
    };
    let b = c.take((n * width as usize + 7) / 8)?;
    let mut at = 0;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = 0;
        for i in 0..width {
            v |= u64::from((b[at / 8] >> (at % 8)) & 1) << i;
            at += 1
        }
        out.push(v)
    }
    Ok(out)
}
fn ints(b: &mut Vec<u8>, vs: &[i64]) {
    let base = *vs.iter().min().unwrap_or(&0);
    let residuals: Vec<u64> = vs
        .iter()
        .map(|v| (*v as u64).wrapping_sub(base as u64))
        .collect();
    let top = residuals.iter().max().copied().unwrap_or(0);
    let width = 64 - top.leading_zeros();
    b.extend_from_slice(&[0, 0]);
    vi(b, base);
    uv(b, 1);
    b.extend_from_slice(&[0, width as u8]);
    b.extend_from_slice(&width_write(&residuals, width))
}
fn read_ints(c: &mut Cursor, n: usize) -> Result<Vec<i64>> {
    if c.byte()? != 0 {
        return Err("unsupported integer layout (slice accepts direct only)".into());
    };
    let transform = c.byte()?;
    let first = if transform != 0 { c.vi()? } else { 0 };
    let step = if transform == 2 { c.vi()? } else { 0 };
    if transform > 2 {
        return Err("integer transform".into());
    };
    let base = c.vi()?;
    let divisor = c.uv()?;
    if divisor == 0 {
        return Err("integer divisor".into());
    };
    if c.byte()? != 0 {
        return Err("unsupported integer packer (slice accepts width only)".into());
    };
    let width = c.count(64)? as u32;
    let m = n - if transform == 1 { 1 } else { 0 };
    let residuals = width_read(c, m, width)?;
    let mut vs = Vec::with_capacity(n);
    if transform == 1 {
        vs.push(first)
    };
    for (i, v) in residuals.into_iter().enumerate() {
        let x = (base as u64).wrapping_add(v.wrapping_mul(divisor)) as i64;
        vs.push(match transform {
            1 => vs[i].wrapping_add(x),
            2 => first
                .wrapping_add(step.wrapping_mul(i as i64))
                .wrapping_add(x),
            _ => x,
        })
    }
    Ok(vs)
}
fn texts(b: &mut Vec<u8>, vs: &[Vec<u8>]) {
    b.extend_from_slice(&[0, 0]);
    let lengths: Vec<i64> = vs.iter().map(|s| s.len() as i64).collect();
    ints(b, &lengths);
    let raw: Vec<u8> = vs.iter().flatten().copied().collect();
    let packed = zstd::bulk::compress(&raw, 7).unwrap();
    if packed.len() + 4 < raw.len() {
        b.push(1);
        uv(b, packed.len() as u64);
        b.extend_from_slice(&packed)
    } else {
        b.push(0);
        b.extend_from_slice(&raw)
    }
}
fn read_texts(c: &mut Cursor, n: usize) -> Result<Vec<Vec<u8>>> {
    if c.byte()? != 0 || c.byte()? != 0 {
        return Err("unsupported text layout (slice accepts raw lengths)".into());
    };
    let lengths = read_ints(c, n)?;
    let mut total = 0usize;
    for &l in &lengths {
        if l < 0 || l as usize > (4 << 20) - total {
            return Err("text expansion".into());
        };
        total += l as usize
    }
    let raw = match c.byte()? {
        0 => c.take(total)?.to_vec(),
        1 => {
            let size = c.count(4 << 20)?;
            let raw = decompress(c.take(size)?, total)?;
            if raw.len() != total {
                return Err("blob length".into());
            };
            raw
        }
        _ => return Err("blob kind".into()),
    };
    let mut at = 0;
    Ok(lengths
        .into_iter()
        .map(|n| {
            let v = raw[at..at + n as usize].to_vec();
            at += n as usize;
            v
        })
        .collect())
}
fn values(b: &mut Vec<u8>, vs: &[String]) {
    b.push(0);
    texts(
        b,
        &vs.iter().map(|s| s.as_bytes().to_vec()).collect::<Vec<_>>(),
    )
}
fn read_values(c: &mut Cursor, n: usize) -> Result<Vec<String>> {
    if c.byte()? != 0 {
        return Err("unsupported typed/stamped value (slice accepts text)".into());
    };
    read_texts(c, n)?
        .into_iter()
        .map(|s| Ok(String::from_utf8(s)?))
        .collect()
}
#[derive(Clone)]
pub struct Schema {
    pub stream: String,
    pub count: usize,
    names: Vec<String>,
    shapes: Vec<u8>,
    contexts: Vec<Vec<Field>>,
}
pub struct Segment {
    pub row: Vec<u8>,
    pub blocks: Vec<Block>,
    pub records: Vec<Record>,
}
pub struct Block {
    pub first: i64,
    pub last: i64,
    pub count: usize,
    pub levels: i64,
    pub body: Vec<u8>,
    pub traces: Vec<Vec<u8>>,
}
fn slots(s: &Schema) -> Vec<u8> {
    let p = s.shapes.iter().fold(0, |a, b| a | b);
    let mut kinds = vec![0];
    if s.names.len() > 1 {
        kinds.push(1)
    };
    if s.shapes.len() > 1 {
        kinds.push(2)
    };
    for (bit, kind) in [(16, 3), (1, 4), (4, 6), (8, 7), (32, 8)] {
        if p & bit != 0 {
            kinds.push(kind)
        }
    }
    if p & 2 != 0 {
        kinds.push(5)
    };
    kinds
}
fn carries(p: u8, k: u8) -> bool {
    match k {
        3 => p & 16 != 0,
        4 => p & 1 != 0,
        5 => p & 2 != 0,
        6 => p & 4 != 0,
        7 => p & 8 != 0,
        8 => p & 32 != 0,
        _ => true,
    }
}
fn intern<T: PartialEq + Clone>(xs: &mut Vec<T>, v: T) -> usize {
    if let Some(i) = xs.iter().position(|x| *x == v) {
        i
    } else {
        xs.push(v);
        xs.len() - 1
    }
}
pub fn encode_segment(mut rs: Vec<Record>) -> Result<Segment> {
    if rs.is_empty() || rs.len() > 16384 || rs.iter().map(input).sum::<usize>() > 4 << 20 {
        return Err("segment bounds".into());
    };
    rs.sort_by_key(|r| r.at);
    let mut s = Schema {
        stream: rs[0].stream.clone(),
        count: rs.len(),
        names: vec![],
        shapes: vec![],
        contexts: vec![],
    };
    let mut ids = Vec::new();
    for r in &rs {
        if r.stream != s.stream {
            return Err("segment stream".into());
        };
        ids.push((
            intern(&mut s.names, r.name.clone()),
            intern(&mut s.shapes, presence(r) | 32),
            if r.context.is_empty() {
                0
            } else {
                intern(&mut s.contexts, r.context.clone())
            },
        ))
    }
    let mut row = vec![1];
    uv(&mut row, s.count as u64);
    string(&mut row, &s.stream);
    strings(&mut row, &s.names);
    uv(&mut row, s.shapes.len() as u64);
    for p in &s.shapes {
        row.push(*p);
        uv(&mut row, 0)
    }
    uv(&mut row, s.contexts.len() as u64);
    if !s.contexts.is_empty() {
        let mut keys: Vec<Vec<String>> = vec![];
        let owners: Vec<i64> = s
            .contexts
            .iter()
            .map(|fs| intern(&mut keys, fs.iter().map(|f| f.key.clone()).collect()) as i64)
            .collect();
        uv(&mut row, keys.len() as u64);
        for k in &keys {
            strings(&mut row, k)
        }
        ints(&mut row, &owners);
        for (i, k) in keys.iter().enumerate() {
            for j in 0..k.len() {
                let vs = s
                    .contexts
                    .iter()
                    .enumerate()
                    .filter(|(n, _)| owners[*n] == i as i64)
                    .map(|(_, fs)| fs[j].value.clone())
                    .collect::<Vec<_>>();
                values(&mut row, &vs)
            }
        }
    };
    crc(&mut row);
    let mut blocks = Vec::new();
    let mut start = 0;
    while start < rs.len() {
        let mut end = start;
        let mut size = 0;
        while end < rs.len() && end - start < 1024 {
            let next = input(&rs[end]);
            if end > start && size + next > MAX_BLOCK {
                break;
            };
            size += next;
            end += 1
        }
        let chunk = &rs[start..end];
        let kinds = slots(&s);
        let mut columns = Vec::new();
        for k in &kinds {
            let mut b = Vec::new();
            let selected = chunk
                .iter()
                .enumerate()
                .filter(|(_, r)| carries(presence(r) | 32, *k))
                .collect::<Vec<_>>();
            match k {
                0 => ints(&mut b, &chunk.iter().map(|r| r.at).collect::<Vec<_>>()),
                1 => ints(
                    &mut b,
                    &ids[start..end]
                        .iter()
                        .map(|id| id.0 as i64)
                        .collect::<Vec<_>>(),
                ),
                2 => ints(
                    &mut b,
                    &ids[start..end]
                        .iter()
                        .map(|id| id.1 as i64)
                        .collect::<Vec<_>>(),
                ),
                3 => ints(
                    &mut b,
                    &selected
                        .iter()
                        .map(|(i, _)| ids[start + *i].2 as i64)
                        .collect::<Vec<_>>(),
                ),
                4 => ints(
                    &mut b,
                    &selected
                        .iter()
                        .map(|(_, r)| i64::from(r.level.unwrap()))
                        .collect::<Vec<_>>(),
                ),
                5 => values(
                    &mut b,
                    &selected
                        .iter()
                        .map(|(_, r)| r.body.clone().unwrap())
                        .collect::<Vec<_>>(),
                ),
                6 | 7 | 8 => texts(
                    &mut b,
                    &selected
                        .iter()
                        .map(|(_, r)| match k {
                            6 => r.trace.to_vec(),
                            7 => r.span.to_vec(),
                            _ => {
                                let mut v = Vec::new();
                                fields(&mut v, &r.attrs);
                                v
                            }
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => unreachable!(),
            };
            columns.push(b)
        }
        let mut body = vec![1];
        uv(&mut body, chunk.len() as u64);
        for c in &columns {
            uv(&mut body, c.len() as u64)
        }
        for c in columns {
            body.extend_from_slice(&c)
        }
        crc(&mut body);
        blocks.push(Block {
            first: chunk[0].at,
            last: chunk.last().unwrap().at,
            count: chunk.len(),
            levels: chunk
                .iter()
                .filter_map(|r| r.level)
                .fold(0, |a, l| a | level_bit(l)),
            body,
            traces: chunk
                .iter()
                .filter(|r| r.trace != [0; 16])
                .map(|r| r.trace.to_vec())
                .collect(),
        });
        start = end
    }
    Ok(Segment {
        row,
        blocks,
        records: rs,
    })
}
pub fn decode_schema(b: &[u8]) -> Result<Schema> {
    let mut c = Cursor { data: checked(b)? };
    let count = c.count(16384)?;
    let stream = c.string()?;
    let names = c.strings(16384)?;
    let mut shapes = Vec::new();
    for _ in 0..c.count(128)? {
        let p = c.byte()?;
        let keys = c.strings(128)?;
        if p & !63 != 0 || p & 32 == 0 || !keys.is_empty() {
            return Err("unsupported typed shape".into());
        };
        shapes.push(p)
    }
    let n = c.count(16384)?;
    let mut contexts = vec![vec![]; n];
    if n > 0 {
        let lists = (0..c.count(n)?)
            .map(|_| c.strings(128))
            .collect::<Result<Vec<_>>>()?;
        let owners = read_ints(&mut c, n)?;
        let mut cells = 0;
        for &id in &owners {
            let k = lists.get(usize::try_from(id)?).ok_or("context reference")?;
            if k.is_empty() {
                return Err("empty context keys".into());
            };
            cells += k.len();
            if cells > 1 << 20 {
                return Err("context cells".into());
            }
        }
        for (id, keys) in lists.iter().enumerate() {
            let members = owners
                .iter()
                .enumerate()
                .filter(|(_, v)| **v == id as i64)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            for key in keys {
                let vs = read_values(&mut c, members.len())?;
                for (&i, v) in members.iter().zip(vs) {
                    contexts[i].push(Field {
                        key: key.clone(),
                        value: v,
                    })
                }
            }
        }
    };
    c.finish()?;
    if count == 0 || names.is_empty() || shapes.is_empty() {
        return Err("empty schema".into());
    };
    Ok(Schema {
        stream,
        count,
        names,
        shapes,
        contexts,
    })
}
pub fn decode_block(s: &Schema, b: &[u8]) -> Result<Vec<Record>> {
    let mut c = Cursor { data: checked(b)? };
    let n = c.count(1024)?;
    if n == 0 {
        return Err("empty block".into());
    };
    let kinds = slots(s);
    let lens = (0..kinds.len())
        .map(|_| c.count(b.len()))
        .collect::<Result<Vec<_>>>()?;
    let mut payloads = HashMap::new();
    for (&k, l) in kinds.iter().zip(lens) {
        payloads.insert(k, c.take(l)?);
    }
    c.finish()?;
    let read_ids = |k: u8, limit: usize| -> Result<Vec<i64>> {
        if let Some(b) = payloads.get(&k) {
            let mut c = Cursor { data: b };
            let vs = read_ints(&mut c, n)?;
            c.finish()?;
            if vs.iter().any(|v| *v < 0 || *v >= limit as i64) {
                return Err("id reference".into());
            };
            Ok(vs)
        } else {
            Ok(vec![0; n])
        }
    };
    let shapes = read_ids(2, s.shapes.len())?;
    let names = read_ids(1, s.names.len())?;
    let mut rs = (0..n)
        .map(|i| Record {
            at: 0,
            stream: s.stream.clone(),
            name: s.names[names[i] as usize].clone(),
            level: None,
            body: None,
            trace: [0; 16],
            span: [0; 8],
            context: vec![],
            attrs: vec![],
        })
        .collect::<Vec<_>>();
    for k in kinds {
        if k == 1 || k == 2 {
            continue;
        };
        let rows = (0..n)
            .filter(|i| carries(s.shapes[shapes[*i] as usize], k))
            .collect::<Vec<_>>();
        let mut c = Cursor { data: payloads[&k] };
        if rows.is_empty() {
            c.finish()?;
            continue;
        };
        match k {
            0 | 3 | 4 => {
                let vs = read_ints(&mut c, rows.len())?;
                for (i, v) in rows.into_iter().zip(vs) {
                    match k {
                        0 => rs[i].at = v,
                        3 => {
                            rs[i].context = s
                                .contexts
                                .get(usize::try_from(v)?)
                                .ok_or("context reference")?
                                .clone()
                        }
                        _ => rs[i].level = Some(i32::try_from(v)?),
                    }
                }
            }
            5 => {
                let vs = read_values(&mut c, rows.len())?;
                for (i, v) in rows.into_iter().zip(vs) {
                    rs[i].body = Some(v)
                }
            }
            6 | 7 | 8 => {
                let vs = read_texts(&mut c, rows.len())?;
                for (i, v) in rows.into_iter().zip(vs) {
                    match k {
                        6 => rs[i].trace = v.as_slice().try_into()?,
                        7 => rs[i].span = v.as_slice().try_into()?,
                        _ => {
                            let mut f = Cursor { data: &v };
                            rs[i].attrs = f.fields()?;
                            f.finish()?;
                        }
                    }
                }
            }
            _ => unreachable!(),
        };
        c.finish()?;
    }
    if rs.windows(2).any(|x| x[0].at > x[1].at) {
        return Err("unordered block".into());
    };
    Ok(rs)
}
pub fn level_bit(level: i32) -> i64 {
    if level < 0 {
        1
    } else if level >= 24 {
        128
    } else {
        1 << (level / 4 + 1)
    }
}
pub fn bloom(values: &[Vec<u8>]) -> Vec<u8> {
    let mut distinct = values.to_vec();
    distinct.sort();
    distinct.dedup();
    let mut filter = vec![0; 8.max((distinct.len() * 10 + 7) / 8)];
    let size = filter.len() as u64 * 8;
    for value in distinct {
        let mut a = 14695981039346656037u64;
        for b in value {
            a = (a ^ u64::from(b)).wrapping_mul(1099511628211)
        }
        let b = (a ^ (a >> 31)).wrapping_mul(0x9e3779b97f4a7c15) | 1;
        for i in 0..7 {
            let bit = a.wrapping_add(i * b) % size;
            filter[bit as usize / 8] |= 1 << (bit % 8)
        }
    }
    filter
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_bits() {
        for n in [i64::MIN, -1, 0, 1, i64::MAX] {
            let mut b = vec![];
            vi(&mut b, n);
            assert_eq!(Cursor { data: &b }.vi().unwrap(), n)
        }
        let values = [i64::MIN, 0, i64::MAX, -1];
        let mut b = vec![];
        ints(&mut b, &values);
        assert_eq!(
            read_ints(&mut Cursor { data: &b }, values.len()).unwrap(),
            values
        );
        assert!(Cursor { data: &[255; 10] }.uv().is_err());
        assert!(Cursor { data: &[129, 1] }.count(128).is_err())
    }
    #[test]
    fn head_and_segment_guards() {
        let rs = crate::fixture(64);
        let b = encode_head(&rs).unwrap();
        assert_eq!(decode_head(&rs[0].stream, &b).unwrap().len(), 64);
        let mut bad = b.clone();
        let end = bad.len() - 1;
        bad[end] ^= 1;
        assert!(decode_head("x", &bad).is_err());
        assert!(decode_head("x", &b[..b.len() - 1]).is_err());
        let mut first = rs
            .iter()
            .filter(|r| r.stream == "stream-0")
            .cloned()
            .collect::<Vec<_>>();
        first[0].at = first[1].at;
        let segment = encode_segment(first).unwrap();
        let schema = decode_schema(&segment.row).unwrap();
        let decoded = decode_block(&schema, &segment.blocks[0].body).unwrap();
        assert_eq!(decoded, segment.records);
        let mut bad = segment.blocks[0].body.clone();
        bad[1] = 0;
        crc(&mut bad);
        assert!(decode_block(&schema, &bad).is_err())
    }
}
