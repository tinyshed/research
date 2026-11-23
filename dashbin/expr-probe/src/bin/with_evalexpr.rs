//! The `evalexpr` crate: a formula built into a tree once, a row's values set in a context a row.

use evalexpr::{ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Value, build_operator_tree};

fn main() {
    let (count, formula, condition) = expr_probe::asked();
    let (fails, flows) = expr_probe::rows(count);
    let formula = build_operator_tree::<DefaultNumericTypes>(&formula).unwrap_or_else(|why| panic!("{why}"));
    let condition = build_operator_tree::<DefaultNumericTypes>(&condition).unwrap_or_else(|why| panic!("{why}"));
    let mut context = HashMapContext::<DefaultNumericTypes>::new();
    let (mut kept, mut sum) = (0, 0.0);
    for (fails, flows) in fails.iter().zip(&flows) {
        context.set_value("fails".to_owned(), Value::Float(*fails)).unwrap_or_else(|why| panic!("{why}"));
        context.set_value("flows".to_owned(), Value::Float(*flows)).unwrap_or_else(|why| panic!("{why}"));
        if !matches!(condition.eval_boolean_with_context(&context), Ok(true)) {
            continue;
        }
        if let Ok(number) = formula.eval_float_with_context(&context) {
            if number.is_finite() {
                kept += 1;
                sum += number;
            }
        }
    }
    expr_probe::say(kept, sum);
}
