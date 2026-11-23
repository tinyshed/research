//! The `rhai` crate, a scripting language: what passing an eval would cost. Only an expression is compiled.

use rhai::{Engine, Scope};

fn main() {
    let (count, formula, condition) = expr_probe::asked();
    let (fails, flows) = expr_probe::rows(count);
    let engine = Engine::new();
    let formula = engine.compile_expression(&formula).unwrap_or_else(|why| panic!("{why}"));
    let condition = engine.compile_expression(&condition).unwrap_or_else(|why| panic!("{why}"));
    let (mut kept, mut sum) = (0, 0.0);
    for (fails, flows) in fails.iter().zip(&flows) {
        let mut scope = Scope::new();
        scope.push("fails", *fails).push("flows", *flows);
        if !matches!(engine.eval_ast_with_scope::<bool>(&mut scope, &condition), Ok(true)) {
            continue;
        }
        if let Ok(number) = engine.eval_ast_with_scope::<f64>(&mut scope, &formula) {
            if number.is_finite() {
                kept += 1;
                sum += number;
            }
        }
    }
    expr_probe::say(kept, sum);
}
