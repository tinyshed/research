// the layout of our own: a header naming the columns, then each column's buffers, every one starting 8-byte aligned
//
// header: "DBF1", u32 rows, u32 columns, u32 header length, then per column:
//   u8 type, u8 flags (1 validity, 2 time as f64), i8 scale, u8 0, u32 text bytes, u16+name, u16+unit
// body, per column: validity bitmap if flagged (bit set = present), then
//   time, wall, int: i64 (or f64 under flag 2) · date: i32 · float: f64 · decimal: u64 low, i64 high
//   text: u32 offsets (rows + 1), then the UTF-8 bytes · bool: bitmap
use crate::{Column, Data, Frame};

const MAGIC: &[u8; 4] = b"DBF1";
const VALIDITY: u8 = 1;
const TIME_AS_F64: u8 = 2;

fn type_of(data: &Data) -> u8 {
    match data {
        Data::Time(_) => 1,
        Data::Wall(_) => 2,
        Data::Date(_) => 3,
        Data::Int(_) => 4,
        Data::Float(_) => 5,
        Data::Decimal { .. } => 6,
        Data::Text { .. } => 7,
        Data::Bool(_) => 8,
    }
}

fn pad(out: &mut Vec<u8>) {
    out.resize(out.len().next_multiple_of(8), 0);
}

fn bitmap(out: &mut Vec<u8>, bits: impl Iterator<Item = bool>) {
    let start = out.len();
    for (i, bit) in bits.enumerate() {
        if i % 8 == 0 {
            out.push(0);
        }
        if bit {
            out[start + i / 8] |= 1 << (i % 8);
        }
    }
    pad(out);
}

fn times(out: &mut Vec<u8>, values: &[i64], as_f64: bool) {
    for &v in values {
        if as_f64 {
            out.extend_from_slice(&(v as f64).to_le_bytes());
        } else {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

pub fn encode(frame: &Frame, time_as_f64: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(frame.rows * frame.columns.len() * 9);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(frame.rows as u32).to_le_bytes());
    out.extend_from_slice(&(frame.columns.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for c in &frame.columns {
        let mut flags = if c.has_nulls() { VALIDITY } else { 0 };
        if time_as_f64 && matches!(c.data, Data::Time(_) | Data::Wall(_)) {
            flags |= TIME_AS_F64;
        }
        let (scale, text_bytes) = match &c.data {
            Data::Decimal { scale, .. } => (*scale, 0),
            Data::Text { bytes, .. } => (0, bytes.len() as u32),
            _ => (0, 0),
        };
        out.extend_from_slice(&[type_of(&c.data), flags, scale as u8, 0]);
        out.extend_from_slice(&text_bytes.to_le_bytes());
        for s in [c.name.as_str(), c.unit.as_deref().unwrap_or("")] {
            out.extend_from_slice(&(s.len() as u16).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
    }
    pad(&mut out);
    let header = out.len() as u32;
    out[12..16].copy_from_slice(&header.to_le_bytes());
    for c in &frame.columns {
        if c.has_nulls() {
            bitmap(&mut out, (0..frame.rows).map(|i| !c.is_null(i)));
        }
        match &c.data {
            Data::Time(v) | Data::Wall(v) => times(&mut out, v, time_as_f64),
            Data::Int(v) => v.iter().for_each(|x| out.extend_from_slice(&x.to_le_bytes())),
            Data::Date(v) => v.iter().for_each(|x| out.extend_from_slice(&x.to_le_bytes())),
            Data::Float(v) => v.iter().for_each(|x| out.extend_from_slice(&x.to_le_bytes())),
            Data::Decimal { mantissa, .. } => mantissa.iter().for_each(|x| out.extend_from_slice(&x.to_le_bytes())),
            Data::Text { offsets, bytes } => {
                offsets.iter().for_each(|x| out.extend_from_slice(&x.to_le_bytes()));
                pad(&mut out);
                out.extend_from_slice(bytes);
            }
            Data::Bool(v) => bitmap(&mut out, v.iter().copied()),
        }
        pad(&mut out);
    }
    out
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or("frame ends early")?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn text(&mut self) -> Result<String, String> {
        let n = self.u16()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| "a name is not UTF-8".into())
    }
    fn align(&mut self) {
        self.at = self.at.next_multiple_of(8);
    }
    fn bits(&mut self, rows: usize) -> Result<Vec<bool>, String> {
        let raw = self.take(rows.div_ceil(8))?;
        self.align();
        Ok((0..rows).map(|i| raw[i / 8] >> (i % 8) & 1 == 1).collect())
    }
    fn words<const N: usize, T>(&mut self, rows: usize, from: fn([u8; N]) -> T) -> Result<Vec<T>, String> {
        let raw = self.take(rows * N)?;
        Ok(raw.chunks_exact(N).map(|c| from(c.try_into().unwrap())).collect())
    }
}

pub fn decode(bytes: &[u8]) -> Result<Frame, String> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != MAGIC {
        return Err("not a frame".into());
    }
    let rows = r.u32()? as usize;
    let count = r.u32()? as usize;
    let header = r.u32()? as usize;
    let mut heads = Vec::with_capacity(count);
    for _ in 0..count {
        let [kind, flags, scale, _] = r.take(4)?.try_into().unwrap();
        let text_bytes = r.u32()? as usize;
        heads.push((kind, flags, scale as i8, text_bytes, r.text()?, r.text()?));
    }
    r.at = header;
    let mut columns = Vec::with_capacity(count);
    for (kind, flags, scale, text_bytes, name, unit) in heads {
        let nulls = if flags & VALIDITY != 0 { Some(r.bits(rows)?.into_iter().map(|v| !v).collect()) } else { None };
        let time = |r: &mut Reader| {
            if flags & TIME_AS_F64 != 0 { r.words(rows, |b| f64::from_le_bytes(b) as i64) } else { r.words(rows, i64::from_le_bytes) }
        };
        let data = match kind {
            1 => Data::Time(time(&mut r)?),
            2 => Data::Wall(time(&mut r)?),
            3 => Data::Date(r.words(rows, i32::from_le_bytes)?),
            4 => Data::Int(r.words(rows, i64::from_le_bytes)?),
            5 => Data::Float(r.words(rows, f64::from_le_bytes)?),
            6 => Data::Decimal { mantissa: r.words(rows, i128::from_le_bytes)?, scale },
            7 => {
                let offsets: Vec<u32> = r.words(rows + 1, u32::from_le_bytes)?;
                r.align();
                let bytes = r.take(text_bytes)?.to_vec();
                if offsets.windows(2).any(|w| w[0] > w[1]) || offsets.last() != Some(&(text_bytes as u32)) {
                    return Err(format!("column {name}: offsets out of order"));
                }
                std::str::from_utf8(&bytes).map_err(|_| format!("column {name}: text is not UTF-8"))?;
                Data::Text { offsets, bytes }
            }
            8 => Data::Bool(r.bits(rows)?),
            other => return Err(format!("column {name}: unknown type {other}")),
        };
        r.align();
        columns.push(Column { name, unit: (!unit.is_empty()).then_some(unit), data, nulls });
    }
    Ok(Frame { rows, columns })
}
