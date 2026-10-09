use super::binary::{Reader, put_unsigned};
use super::{Result, Sample, Summary};
use num_bigint::{BigInt, Sign};
use num_traits::Zero;

pub fn finite_units(value: f64) -> Result<BigInt> {
    let bits = value.to_bits();
    let exponent = (bits >> 52) & 0x7ff;
    if exponent == 0x7ff {
        return Err("nonfinite value".into());
    }
    let mut mantissa = bits & ((1u64 << 52) - 1);
    if exponent != 0 {
        mantissa |= 1u64 << 52;
    }
    let mut result = BigInt::from(mantissa);
    if exponent != 0 {
        result <<= (exponent - 1) as usize;
    }
    if bits >> 63 != 0 {
        result = -result;
    }
    Ok(result)
}

pub fn encode_exact(value: &BigInt) -> Vec<u8> {
    if value.is_zero() {
        return vec![0];
    }
    let exponent = value.trailing_zeros().unwrap();
    let magnitude = value.magnitude() >> exponent as usize;
    let encoded = magnitude.to_bytes_be();
    let mut out = Vec::new();
    put_unsigned(&mut out, encoded.len() as u64);
    put_unsigned(
        &mut out,
        (exponent << 1) | u64::from(value.sign() == Sign::Minus),
    );
    out.extend(encoded);
    out
}

fn read_value(r: &mut Reader<'_>) -> Result<BigInt> {
    let length = r.size(264)?;
    if length == 0 {
        return Ok(BigInt::zero());
    }
    let token = r.unsigned()?;
    let exponent = token >> 1;
    if exponent >= 2106 {
        return Err("exact summary exponent".into());
    }
    let magnitude = r.take(length)?;
    if magnitude[0] == 0 || magnitude[length - 1] & 1 == 0 {
        return Err("exact summary magnitude".into());
    }
    let mut result = BigInt::from_bytes_be(
        if token & 1 == 0 {
            Sign::Plus
        } else {
            Sign::Minus
        },
        magnitude,
    );
    if result.bits() + exponent > 2106 {
        return Err("exact summary magnitude bound".into());
    }
    result <<= exponent as usize;
    Ok(result)
}

pub fn read_exact_value(encoded: &[u8]) -> Result<BigInt> {
    let mut r = Reader::new(encoded);
    let value = read_value(&mut r)?;
    r.finish()?;
    Ok(value)
}

pub(super) fn read_exact(r: &mut Reader<'_>) -> Result<Vec<u8>> {
    let before = r.remaining();
    let value = read_value(r)?;
    let encoded = before[..before.len() - r.remaining().len()].to_vec();
    if encoded != encode_exact(&value) {
        return Err("noncanonical exact summary".into());
    }
    Ok(encoded)
}

fn minimum(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::from_bits(0x7ff8000000000001)
    } else if a == 0.0 && b == 0.0 {
        f64::from_bits(a.to_bits() | b.to_bits())
    } else {
        a.min(b)
    }
}
fn maximum(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::from_bits(0x7ff8000000000001)
    } else if a == 0.0 && b == 0.0 {
        f64::from_bits(a.to_bits() & b.to_bits())
    } else {
        a.max(b)
    }
}

pub(super) fn summarize(points: &[Sample], kind: i32) -> Summary {
    let mut s = Summary {
        last: points.last().unwrap().value,
        min: points[0].value,
        max: points[0].value,
        valid: true,
        ..Default::default()
    };
    for (i, point) in points.iter().enumerate() {
        let value = point.value;
        if !value.is_finite() {
            s.valid = false;
        }
        if kind == 0 {
            s.min = minimum(s.min, value);
            s.max = maximum(s.max, value);
            s.sum += value;
        } else {
            if value < 0.0 {
                s.valid = false;
            }
            if i > 0 {
                let previous = points[i - 1].value;
                if value < previous {
                    s.resets += 1;
                    s.increase += value;
                } else {
                    s.increase += value - previous;
                }
            }
        }
    }
    if !s.sum.is_finite() || !s.increase.is_finite() {
        s.valid = false;
    }
    let mut sum = BigInt::zero();
    let mut increase = BigInt::zero();
    let mut previous = BigInt::zero();
    let (mut min, mut max) = (points[0].value, points[0].value);
    for (i, point) in points.iter().enumerate() {
        if kind == 1 && (!point.value.is_finite() || point.value < 0.0) {
            return s;
        }
        let current = match finite_units(point.value) {
            Ok(v) => v,
            Err(_) => return s,
        };
        sum += &current;
        if kind == 1 && i > 0 {
            if point.value < points[i - 1].value {
                increase += &current;
            } else {
                increase += &current - &previous;
            }
        }
        min = minimum(min, point.value);
        max = maximum(max, point.value);
        previous = current;
    }
    s.min = min;
    s.max = max;
    s.exact_sum = encode_exact(&sum);
    s.exact_increase = encode_exact(&increase);
    s
}
