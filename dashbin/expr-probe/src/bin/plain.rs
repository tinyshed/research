//! The floor: the same arithmetic written in Rust, with no formula read at all.

fn main() {
    let (count, _, _) = expr_probe::asked();
    let (fails, flows) = expr_probe::rows(count);
    let (mut kept, mut sum) = (0, 0.0);
    for (fails, flows) in fails.iter().zip(&flows) {
        if *flows >= 20.0 {
            kept += 1;
            sum += fails / flows * 100.0;
        }
    }
    expr_probe::say(kept, sum);
}
