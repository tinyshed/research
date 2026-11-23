//! A parser and an evaluator of ours.

use expr_probe::own::{Value, parse, value};

fn main() {
    let (count, formula, condition) = expr_probe::asked();
    let (fails, flows) = expr_probe::rows(count);
    let names = ["fails", "flows"];
    let formula = parse(&formula, &names).unwrap_or_else(|why| panic!("{why}"));
    let condition = parse(&condition, &names).unwrap_or_else(|why| panic!("{why}"));
    let (mut kept, mut sum) = (0, 0.0);
    for (fails, flows) in fails.iter().zip(&flows) {
        let row = [Some(*fails), Some(*flows)];
        if value(&condition, &row) != Some(Value::Truth(true)) {
            continue;
        }
        if let Some(Value::Number(number)) = value(&formula, &row) {
            kept += 1;
            sum += number;
        }
    }
    expr_probe::say(kept, sum);
}
