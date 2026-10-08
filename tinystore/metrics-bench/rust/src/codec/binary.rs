use super::Result;
use std::cell::RefCell;

thread_local! {
    static VALUE_WRITER: RefCell<Option<zstd::bulk::Compressor<'static>>> = const {RefCell::new(None)};
    static METADATA_WRITER: RefCell<Option<zstd::bulk::Compressor<'static>>> = const {RefCell::new(None)};
    static VALUE_READER: RefCell<Option<zstd::bulk::Decompressor<'static>>> = const {RefCell::new(None)};
    static METADATA_READER: RefCell<Option<zstd::bulk::Decompressor<'static>>> = const {RefCell::new(None)};
}

pub fn fold(value: i64) -> u64 {
    (value as u64).wrapping_shl(1) ^ ((value >> 63) as u64)
}
pub fn unfold(value: u64) -> i64 {
    (value >> 1) as i64 ^ -((value & 1) as i64)
}

pub fn put_unsigned(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        out.push(value as u8 | 128);
        value >>= 7;
    }
    out.push(value as u8);
}

pub fn put_float(out: &mut Vec<u8>, value: f64) {
    for (tag, factor) in [1.0, 100.0].iter().enumerate() {
        let product = (value * factor).round();
        if !product.is_finite() || product.abs() >= 2f64.powi(60) {
            continue;
        }
        let integer = product as i64;
        if ((integer as f64) / factor).to_bits() == value.to_bits() {
            put_unsigned(out, (fold(integer) << 2) | tag as u64);
            return;
        }
    }
    out.push(2);
    out.extend_from_slice(&value.to_bits().to_le_bytes());
}

pub struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }
    pub fn position(&self) -> usize {
        self.at
    }
    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.at..]
    }
    pub fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        if size > self.data.len() - self.at {
            return Err("truncated binary field".into());
        }
        let result = &self.data[self.at..self.at + size];
        self.at += size;
        Ok(result)
    }
    pub fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn word(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn unsigned(&mut self) -> Result<u64> {
        let mut value = 0;
        for i in 0..10 {
            let byte = self.byte()?;
            if byte < 128 {
                if i == 9 && byte > 1 {
                    return Err("invalid varint".into());
                }
                return Ok(value | u64::from(byte) << (7 * i));
            }
            value |= u64::from(byte & 127) << (7 * i);
        }
        Err("invalid varint".into())
    }
    pub fn size(&mut self, maximum: usize) -> Result<usize> {
        let value = self.unsigned()?;
        if value > maximum as u64 {
            return Err("binary size exceeds limit".into());
        }
        Ok(value as usize)
    }
    pub fn float(&mut self) -> Result<f64> {
        let token = self.unsigned()?;
        if token == 2 {
            return Ok(f64::from_bits(self.word()?));
        }
        let tag = token & 3;
        if tag > 1 {
            return Err("scalar tag".into());
        }
        let value = unfold(token >> 2) as f64;
        Ok(if tag == 1 { value / 100.0 } else { value })
    }
    pub fn finish(&self) -> Result<()> {
        if self.at == self.data.len() {
            Ok(())
        } else {
            Err("trailing binary fields".into())
        }
    }
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    VALUE_WRITER.with(|slot| compress_with(&mut slot.borrow_mut(), data))
}

fn compress_with(
    slot: &mut Option<zstd::bulk::Compressor<'static>>,
    data: &[u8],
) -> Result<Vec<u8>> {
    if slot.is_none() {
        let mut writer = zstd::bulk::Compressor::new(3).map_err(|e| e.to_string())?;
        writer
            .set_parameter(zstd::zstd_safe::CParameter::WindowLog(13))
            .map_err(|e| e.to_string())?;
        *slot = Some(writer);
    }
    slot.as_mut()
        .unwrap()
        .compress(data)
        .map_err(|e| e.to_string())
}

pub fn expand(data: &[u8], maximum: usize) -> Result<Vec<u8>> {
    VALUE_READER.with(|slot| expand_with(&mut slot.borrow_mut(), data, maximum))
}

fn expand_with(
    slot: &mut Option<zstd::bulk::Decompressor<'static>>,
    data: &[u8],
    maximum: usize,
) -> Result<Vec<u8>> {
    if slot.is_none() {
        let mut reader = zstd::bulk::Decompressor::new().map_err(|e| e.to_string())?;
        reader
            .set_parameter(zstd::zstd_safe::DParameter::WindowLogMax(13))
            .map_err(|e| e.to_string())?;
        *slot = Some(reader);
    }
    slot.as_mut()
        .unwrap()
        .decompress(data, maximum)
        .map_err(|e| format!("zstd: {e}"))
}

pub fn metadata_encode(data: &[u8]) -> Result<Vec<u8>> {
    let compressed = METADATA_WRITER.with(|slot| compress_with(&mut slot.borrow_mut(), data))?;
    let mut out = Vec::new();
    if compressed.len() < data.len() {
        out.push(1);
        out.extend(compressed);
    } else {
        out.push(0);
        out.extend_from_slice(data);
    }
    Ok(out)
}

pub fn metadata_decode(data: &[u8]) -> Result<Vec<u8>> {
    if data.is_empty() || data.len() > 8192 {
        return Err("compressed field size".into());
    }
    match data[0] {
        0 => Ok(data[1..].to_vec()),
        1 => METADATA_READER.with(|slot| expand_with(&mut slot.borrow_mut(), &data[1..], 8192)),
        _ => Err("compression mode".into()),
    }
}

pub fn crc32c(parts: &[&[u8]]) -> u32 {
    let mut crc = 0;
    for part in parts {
        crc = crc32c::crc32c_append(crc, part);
    }
    crc
}
