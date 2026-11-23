//! What every variant computes: a formula and a condition over the rows of two columns.

pub mod own;

/// Rows of failures and flows, seeded: a flow count now and then under 20, and a zero.
pub fn rows(count: usize) -> (Vec<f64>, Vec<f64>) {
    let mut state = 7_u64;
    let mut next = move || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as f64 / (1_u64 << 31) as f64
    };
    let flows: Vec<f64> = (0..count).map(|at| if at % 97 == 0 { 0.0 } else { (next() * 4000.0).floor() }).collect();
    let fails = flows.iter().map(|flows| (flows * next() * 0.2).floor()).collect();
    (fails, flows)
}

/// The rows asked for, the formula and the condition, from the command line or as the probe sets them.
pub fn asked() -> (usize, String, String) {
    let mut args = std::env::args().skip(1);
    let count = args.next().and_then(|count| count.parse().ok()).unwrap_or(10_000);
    let formula = args.next().unwrap_or_else(|| "fails / flows * 100.0".to_owned());
    let condition = args.next().unwrap_or_else(|| "flows >= 20.0".to_owned());
    (count, formula, condition)
}

/// What a run came to: the rows that passed the condition and had a value, and their values' sum.
pub fn say(kept: usize, sum: f64) {
    println!("{kept} rows, sum {sum:.6}");
}
