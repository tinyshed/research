//! The `cel` crate: a formula compiled once, the standard context built once, and a row's values added to a
//! scope inside it a row. A context built afresh a row took 15 µs of it: thirty times this.

use cel::{Context, Program, Value};

fn main() {
    let (count, formula, condition) = expr_probe::asked();
    let (fails, flows) = expr_probe::rows(count);
    let formula = Program::compile(&formula).unwrap_or_else(|why| panic!("{why}"));
    let condition = Program::compile(&condition).unwrap_or_else(|why| panic!("{why}"));
    let (mut kept, mut sum) = (0, 0.0);
    let standard = Context::default();
    for (fails, flows) in fails.iter().zip(&flows) {
        let mut context = standard.new_inner_scope();
        context.add_variable_from_value("fails", *fails);
        context.add_variable_from_value("flows", *flows);
        if !matches!(condition.execute(&context), Ok(Value::Bool(true))) {
            continue;
        }
        if let Ok(Value::Float(number)) = formula.execute(&context) {
            if number.is_finite() {
                kept += 1;
                sum += number;
            }
        }
    }
    expr_probe::say(kept, sum);
}
