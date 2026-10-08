//! Go's aggregate arithmetic: integers in units of 2^-1074, rounded at 53 bits.
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, ToPrimitive, Zero};

pub fn finite_units(value: f64) -> Result<BigInt, String> {
    let bits = value.to_bits();
    let exponent = (bits >> 52) & 0x7ff;
    if exponent == 0x7ff {
        return Err("nonfinite metric value".into());
    }
    let mut mantissa = bits & ((1_u64 << 52) - 1);
    if exponent != 0 {
        mantissa |= 1_u64 << 52;
    }
    let mut units = BigInt::from(mantissa);
    if exponent != 0 {
        units <<= (exponent - 1) as usize;
    }
    if bits >> 63 != 0 {
        units = -units;
    }
    Ok(units)
}

fn rounded_ratio(numerator: &BigUint, denominator: &BigUint) -> BigUint {
    let quotient = numerator / denominator;
    let twice_remainder = (numerator % denominator) << 1_usize;
    if twice_remainder > *denominator
        || (twice_remainder == *denominator && (&quotient & BigUint::one()) != BigUint::zero())
    {
        quotient + BigUint::one()
    } else {
        quotient
    }
}

/// Emulates big.Float precision 53 followed by SetMantExp(-1074), including
/// its second rounding when Float64 produces a subnormal result.
pub fn rounded_quotient(numerator: &BigInt, denominator: &BigInt) -> (f64, bool) {
    if numerator.is_zero() {
        return (0.0, false);
    }
    assert!(denominator.sign() == Sign::Plus);
    let negative = numerator.sign() == Sign::Minus;
    let n = numerator.magnitude();
    let d = denominator.magnitude();
    let mut exponent = n.bits() as i64 - d.bits() as i64;
    let below = if exponent >= 0 {
        n < &(d << exponent as usize)
    } else {
        &(n << (-exponent) as usize) < d
    };
    if below {
        exponent -= 1;
    }
    let shift = 52 - exponent;
    let rounded = if shift >= 0 {
        rounded_ratio(&(n << shift as usize), d)
    } else {
        rounded_ratio(n, &(d << (-shift) as usize))
    };
    let mut significand = rounded.to_u64().expect("rounded 53-bit significand");
    if significand >= 1_u64 << 53 {
        significand >>= 1;
        exponent += 1;
    }
    let sign = if negative { 1_u64 << 63 } else { 0 };
    let ieee_exponent = exponent - 1074;
    if ieee_exponent > 1023 {
        return (f64::from_bits(sign | 0x7ff0000000000000), true);
    }
    if ieee_exponent >= -1022 {
        let bits =
            sign | (((ieee_exponent + 1023) as u64) << 52) | (significand & ((1_u64 << 52) - 1));
        return (f64::from_bits(bits), false);
    }
    let discarded = 52 - exponent;
    let subnormal = if discarded <= 0 {
        significand << (-discarded) as u32
    } else if discarded >= 64 {
        0
    } else {
        let quotient = significand >> discarded as u32;
        let remainder = significand & ((1_u64 << discarded as u32) - 1);
        let halfway = 1_u64 << (discarded as u32 - 1);
        quotient + u64::from(remainder > halfway || (remainder == halfway && quotient & 1 != 0))
    };
    (f64::from_bits(sign | subnormal), false)
}

pub fn rounded_exact(numerator: &BigInt) -> (f64, bool) {
    rounded_quotient(numerator, &BigInt::one())
}

pub fn minimum(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if b < a || (a == 0.0 && b == 0.0 && b.is_sign_negative()) {
        b
    } else {
        a
    }
}

pub fn maximum(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if b > a || (a == 0.0 && b == 0.0 && !b.is_sign_negative()) {
        b
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_roundtrips_finite_bits() {
        for bits in [
            0,
            1,
            2,
            0x8000000000000001,
            0x0010000000000000,
            0x3ff0000000000000,
            0x3fb999999999999a,
            0x7fefffffffffffff,
        ] {
            let value = f64::from_bits(bits);
            assert_eq!(
                rounded_exact(&finite_units(value).unwrap()).0.to_bits(),
                bits
            );
        }
    }
    #[test]
    fn cancellation_overflow_and_subnormal_ties() {
        let max = finite_units(f64::MAX).unwrap();
        assert!(rounded_exact(&(&max + &max)).1);
        assert_eq!(rounded_exact(&(&max - &max)).0.to_bits(), 0);
        assert_eq!(
            rounded_quotient(&BigInt::from(1), &BigInt::from(2))
                .0
                .to_bits(),
            0
        );
        assert_eq!(
            rounded_quotient(&BigInt::from(3), &BigInt::from(2))
                .0
                .to_bits(),
            2
        );
    }
}
