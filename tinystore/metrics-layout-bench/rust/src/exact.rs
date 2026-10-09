//! Go's aggregate arithmetic: integers in units of 2^-1074, rounded at 53 bits.
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, ToPrimitive, Zero};
use std::sync::atomic::{AtomicBool, Ordering};

static FAST_EXACT: AtomicBool = AtomicBool::new(false);

/// Select the experimental sum/average accumulator before starting a workload.
pub fn configure_fast(enabled: bool) {
    FAST_EXACT.store(enabled, Ordering::Relaxed);
}

pub fn fast_enabled() -> bool {
    FAST_EXACT.load(Ordering::Relaxed)
}

// A finite f64 occupies at most 2098 bits in units of 2^-1074. Even the sum
// of u64::MAX such values occupies at most 2162 magnitude bits. The extra
// signed limb leaves room for carries and makes overflow checking explicit.
const SUM_LIMBS: usize = 35;

/// Exact signed sum in units of 2^-1074, stored as two's-complement limbs.
///
/// The 280-byte accumulator allocates nothing when adding a sample. Query
/// buckets keep it behind a lazy Box, so only experimental sum/avg buckets
/// pay for this storage. Rounding still uses the reference BigInt routines.
#[derive(Clone, Debug)]
pub struct ExactSum {
    limbs: [u64; SUM_LIMBS],
}

impl Default for ExactSum {
    fn default() -> Self {
        Self {
            limbs: [0; SUM_LIMBS],
        }
    }
}

impl ExactSum {
    pub fn add_value(&mut self, value: f64) -> Result<(), String> {
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as usize;
        if exponent == 0x7ff {
            return Err("nonfinite metric value".into());
        }
        let mut mantissa = bits & ((1_u64 << 52) - 1);
        if exponent != 0 {
            mantissa |= 1_u64 << 52;
        }
        if mantissa == 0 {
            return Ok(());
        }
        let shift = exponent.saturating_sub(1);
        let index = shift / 64;
        let within = shift % 64;
        let negative = bits >> 63 != 0;
        let was_negative = self.negative();
        self.add_digit(index, mantissa << within, negative);
        if within != 0 {
            self.add_digit(index + 1, mantissa >> (64 - within), negative);
        }
        self.check_overflow(was_negative, negative)
    }

    /// Add an already-decoded exact block summary without changing its value.
    pub fn add_bigint(&mut self, value: &BigInt) -> Result<(), String> {
        if value.bits() >= (SUM_LIMBS * 64) as u64 {
            return Err("metrics resource limit: exact sum capacity".into());
        }
        let negative = value.sign() == Sign::Minus;
        let was_negative = self.negative();
        for (index, digit) in value.iter_u64_digits().enumerate() {
            self.add_digit(index, digit, negative);
        }
        self.check_overflow(was_negative, negative)
    }

    /// Merge a completed series bucket into a grouped bucket exactly.
    pub fn merge(&mut self, other: &Self) -> Result<(), String> {
        let was_negative = self.negative();
        let mut carry = false;
        for (current, source) in self.limbs.iter_mut().zip(other.limbs.iter()) {
            let (sum, first_carry) = current.overflowing_add(*source);
            let (sum, second_carry) = sum.overflowing_add(u64::from(carry));
            *current = sum;
            carry = first_carry || second_carry;
        }
        self.check_overflow(was_negative, other.negative())
    }

    /// Convert once per output bucket, preserving the reference's two rounds.
    pub fn to_bigint(&self) -> BigInt {
        let negative = self.negative();
        let mut digits = [0_u32; SUM_LIMBS * 2];
        let mut carry = negative;
        for (index, &stored) in self.limbs.iter().enumerate() {
            let magnitude = if negative {
                let (word, next_carry) = (!stored).overflowing_add(u64::from(carry));
                carry = next_carry;
                word
            } else {
                stored
            };
            digits[index * 2] = magnitude as u32;
            digits[index * 2 + 1] = (magnitude >> 32) as u32;
        }
        let end = digits
            .iter()
            .rposition(|&digit| digit != 0)
            .map_or(0, |i| i + 1);
        BigInt::from_slice(
            if negative { Sign::Minus } else { Sign::Plus },
            &digits[..end],
        )
    }

    fn negative(&self) -> bool {
        self.limbs[SUM_LIMBS - 1] >> 63 != 0
    }

    fn check_overflow(&self, previous_negative: bool, added_negative: bool) -> Result<(), String> {
        if previous_negative == added_negative && self.negative() != previous_negative {
            Err("metrics resource limit: exact sum capacity".into())
        } else {
            Ok(())
        }
    }

    // Apply one unsigned magnitude limb, propagating carry/borrow only as far
    // as needed. Negative values use subtraction, including sign extension.
    fn add_digit(&mut self, mut index: usize, digit: u64, negative: bool) {
        if digit == 0 {
            return;
        }
        let (word, mut carry) = if negative {
            self.limbs[index].overflowing_sub(digit)
        } else {
            self.limbs[index].overflowing_add(digit)
        };
        self.limbs[index] = word;
        while carry && index + 1 < SUM_LIMBS {
            index += 1;
            let (word, next_carry) = if negative {
                self.limbs[index].overflowing_sub(1)
            } else {
                self.limbs[index].overflowing_add(1)
            };
            self.limbs[index] = word;
            carry = next_carry;
        }
    }
}

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

    fn compare_sum(values: &[f64]) {
        let mut fixed = ExactSum::default();
        let mut reference = BigInt::zero();
        for &value in values {
            fixed.add_value(value).unwrap();
            reference += finite_units(value).unwrap();
        }
        assert_eq!(fixed.to_bigint(), reference);
        let (actual, overflow) = rounded_exact(&fixed.to_bigint());
        let (expected, expected_overflow) = rounded_exact(&reference);
        assert_eq!(
            (actual.to_bits(), overflow),
            (expected.to_bits(), expected_overflow)
        );
        if !values.is_empty() {
            let count = BigInt::from(values.len());
            let (actual, overflow) = rounded_quotient(&fixed.to_bigint(), &count);
            let (expected, expected_overflow) = rounded_quotient(&reference, &count);
            assert_eq!(
                (actual.to_bits(), overflow),
                (expected.to_bits(), expected_overflow)
            );
        }
    }

    #[test]
    fn fixed_sum_extremes_cancellation_and_rounding() {
        let tiny = f64::from_bits(1);
        for values in [
            vec![0.0, -0.0],
            vec![-0.0, -0.0],
            vec![tiny, tiny, -tiny],
            vec![tiny, 0.0],
            vec![tiny, f64::from_bits(2)],
            vec![-tiny, 0.0],
            vec![f64::MAX, f64::MAX],
            vec![-f64::MAX, -f64::MAX],
            vec![f64::MAX, tiny, -f64::MAX],
            vec![-f64::MAX, -tiny, f64::MAX],
            vec![f64::MAX, -f64::MAX, -0.0],
            vec![f64::MIN_POSITIVE, -f64::from_bits(0x000fffffffffffff)],
            vec![1.0, f64::from_bits(0x3ca0000000000000), f64::from_bits(1)],
        ] {
            compare_sum(&values);
        }
        let mut sum = ExactSum::default();
        sum.add_value(1.0).unwrap();
        let before = sum.to_bigint();
        for bits in [0x7ff0000000000000, 0xfff0000000000000, 0x7ff8000000000042] {
            assert!(sum.add_value(f64::from_bits(bits)).is_err());
            assert_eq!(sum.to_bigint(), before);
        }
    }

    #[test]
    fn fixed_sum_every_finite_exponent_and_deterministic_random_bits() {
        let mut random = 0x9e3779b97f4a7c15_u64;
        let mut fixed = ExactSum::default();
        let mut reference = BigInt::zero();
        let mut values = Vec::new();
        for exponent in 0..0x7ff_u64 {
            for sign in [0, 1_u64 << 63] {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let bits = sign | (exponent << 52) | (random & ((1_u64 << 52) - 1));
                let value = f64::from_bits(bits);
                values.push(value);
                fixed.add_value(value).unwrap();
                reference += finite_units(value).unwrap();
                if exponent % 31 == 0 {
                    assert_eq!(fixed.to_bigint(), reference, "exponent {exponent}");
                }
            }
        }
        assert_eq!(fixed.to_bigint(), reference);
        // Reverse the order and force low/high-word cancellation carry chains.
        values.reverse();
        compare_sum(&values);
        values.extend(values.clone().into_iter().map(|value| -value));
        compare_sum(&values);
    }

    #[test]
    fn fixed_sum_summary_and_group_merges_match_reference() {
        let mut reference = BigInt::zero();
        let mut grouped = ExactSum::default();
        for series in 0..16_u64 {
            let mut current = ExactSum::default();
            let mut summary = BigInt::zero();
            for sample in 0..240_u64 {
                let exponent = ((sample * 19 + series * 131) % 0x7ff) << 52;
                let mantissa = (sample * 7919 + series * 104729) & ((1_u64 << 52) - 1);
                let sign = if (series + sample) % 3 == 0 {
                    1_u64 << 63
                } else {
                    0
                };
                let value = f64::from_bits(sign | exponent | mantissa);
                if sample < 120 {
                    current.add_value(value).unwrap();
                } else {
                    summary += finite_units(value).unwrap();
                }
                reference += finite_units(value).unwrap();
            }
            current.add_bigint(&summary).unwrap();
            grouped.merge(&current).unwrap();
        }
        assert_eq!(grouped.to_bigint(), reference);
        grouped.add_bigint(&(-reference)).unwrap();
        assert!(grouped.to_bigint().is_zero());
        let capacity = BigInt::one() << (SUM_LIMBS * 64);
        assert!(grouped.add_bigint(&capacity).is_err());
        assert_eq!(std::mem::size_of::<ExactSum>(), 280);
    }
}
