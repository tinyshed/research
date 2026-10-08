use crate::{Case, Payload, Sample, fingerprint_samples};
use std::collections::HashMap;
use std::hint::black_box;

fn prepare_samples(input: &[Sample]) -> Vec<Sample> {
    let mut ordered: Vec<Sample> = Vec::new();
    let mut by_time: Option<HashMap<i64, Sample>> = None;
    for point in input {
        if by_time.is_none() && ordered.last().is_some_and(|last| point.at <= last.at) {
            let mut map = HashMap::with_capacity(ordered.len() + 1);
            for earlier in &ordered {
                map.insert(earlier.at, *earlier);
            }
            ordered = Vec::new();
            by_time = Some(map);
        }
        match &mut by_time {
            Some(map) => {
                map.insert(point.at, *point);
            }
            None => ordered.push(*point),
        }
    }
    if let Some(map) = by_time {
        // Preserve incremental growth rather than reserving a different output budget.
        for point in map.values() {
            ordered.push(*point);
        }
        ordered.sort_unstable_by_key(|p| p.at);
    }
    ordered
}

fn fixture(n: usize, kind: &str) -> Vec<Sample> {
    let mut input: Vec<Sample> = (0..n)
        .map(|i| {
            let bits = (i as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15);
            let mut value = f64::from_bits(0x3ff0000000000000 | bits & 0xfffffffffffff);
            if i % 17 == 0 {
                value = f64::from_bits(0x8000000000000000)
            }
            if i % 29 == 0 {
                value = f64::from_bits(0x7ff8000000000042)
            }
            Sample {
                at: 1000000 + i as i64 * 10,
                value,
            }
        })
        .collect();
    if kind == "duplicates" {
        for i in (3..n).step_by(4) {
            input[i].at = input[i - 1].at;
        }
    }
    if kind != "ordered" {
        let mut state = 0x123456789abcdefu64;
        for i in (1..n).rev() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = (state % (i as u64 + 1)) as usize;
            input.swap(i, j);
        }
    }
    input
}

pub fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for n in [240, 4096] {
        for kind in ["ordered", "shuffled", "duplicates"] {
            let input = fixture(n, kind);
            out.push(Case {
                name: format!("ingest/{kind}/{n}"),
                units: n,
                input_bytes: n * 16,
                input_hash: fingerprint_samples(&input),
                run: Box::new(move || Payload::Samples(prepare_samples(black_box(&input)))),
            });
            if kind != "ordered" {
                let input = fixture(n, kind);
                // Go's comparison changes; Rust's typed sort is already the baseline.
                out.push(Case {
                    name: format!("ingest/{kind}/{n}/typed_sort"),
                    units: n,
                    input_bytes: n * 16,
                    input_hash: fingerprint_samples(&input),
                    run: Box::new(move || Payload::Samples(prepare_samples(black_box(&input)))),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_last_arrival_bits_for_duplicate_times() {
        let input = [
            Sample { at: 2, value: 1.0 },
            Sample {
                at: 1,
                value: f64::NAN,
            },
            Sample { at: 2, value: -0.0 },
        ];
        let out = prepare_samples(&input);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].at, 1);
        assert_eq!(out[1].value.to_bits(), (-0.0f64).to_bits());
    }
    #[test]
    fn sorts_fixture_and_deduplicates() {
        for n in [240, 4096] {
            for kind in ["ordered", "shuffled", "duplicates"] {
                let input = fixture(n, kind);
                let out = prepare_samples(&input);
                assert!(out.windows(2).all(|p| p[0].at < p[1].at));
                assert_eq!(out.len(), if kind == "duplicates" { n - n / 4 } else { n });
                for p in out {
                    let last = input.iter().rev().find(|s| s.at == p.at).unwrap();
                    assert_eq!(last.value.to_bits(), p.value.to_bits());
                }
            }
        }
    }
}
