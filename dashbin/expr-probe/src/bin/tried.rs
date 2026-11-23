//! What each evaluator does with what a formula can go wrong with, a line a case, for the report's table.

use expr_probe::own;

const CASES: [(&str, &str); 9] = [
    ("the formula as a person writes it", "fails / flows * 100"),
    ("a division by zero", "fails / zero"),
    ("a name with no value in the row", "fails / absent"),
    ("a name that is no read's", "fails / flow"),
    ("whole numbers divided", "5 / 200"),
    ("an assignment", "fails = 1"),
    ("two statements", "fails; flows"),
    ("a loop", "while true {}"),
    ("a call of something", "exit(1)"),
];

fn main() {
    for (what, formula) in CASES {
        println!("\n## {what}: `{formula}`");
        println!("own       {}", ours(formula));
        println!("cel       {}", cel(formula));
        println!("evalexpr  {}", evalexpr(formula));
        println!("rhai      {}", rhai(formula));
    }
    println!("\n## the names a formula reaches, before any row: `fails / flows * 100`");
    let program = ::cel::Program::compile("fails / flows * 100").unwrap();
    let references = program.references();
    let mut reached: Vec<&str> = references.variables().into_iter().collect();
    reached.sort_unstable();
    println!("cel       {reached:?}");
    let tree = ::evalexpr::build_operator_tree::<::evalexpr::DefaultNumericTypes>("fails / flows * 100").unwrap();
    println!("evalexpr  {:?}", tree.iter_variable_identifiers().collect::<Vec<_>>());
    println!("own       refuses a name that is no read's as it parses: see above");
}

fn ours(formula: &str) -> String {
    match own::parse(formula, &["fails", "flows", "zero", "absent"]) {
        Err(why) => format!("refused before any row: {why}"),
        Ok(parsed) => format!("{:?}", own::value(&parsed, &[Some(5.0), Some(200.0), Some(0.0), None])),
    }
}

fn cel(formula: &str) -> String {
    let program = match ::cel::Program::compile(formula) {
        Ok(program) => program,
        Err(why) => return format!("refused before any row: {}", why.to_string().lines().next().unwrap_or_default()),
    };
    let mut context = ::cel::Context::default();
    context.add_variable_from_value("fails", 5.0);
    context.add_variable_from_value("flows", 200.0);
    context.add_variable_from_value("zero", 0.0);
    match program.execute(&context) {
        Ok(value) => format!("{value:?}"),
        Err(why) => format!("fails at the row: {why}"),
    }
}

fn evalexpr(formula: &str) -> String {
    use ::evalexpr::{ContextWithMutableVariables as _, DefaultNumericTypes, HashMapContext, Value};
    let tree = match ::evalexpr::build_operator_tree::<DefaultNumericTypes>(formula) {
        Ok(tree) => tree,
        Err(why) => return format!("refused before any row: {why}"),
    };
    let mut context = HashMapContext::<DefaultNumericTypes>::new();
    for (name, value) in [("fails", 5.0), ("flows", 200.0), ("zero", 0.0)] {
        context.set_value(name.to_owned(), Value::Float(value)).unwrap();
    }
    match tree.eval_with_context(&context) {
        Ok(value) => format!("{value:?}"),
        Err(why) => format!("fails at the row: {why}"),
    }
}

fn rhai(formula: &str) -> String {
    let engine = ::rhai::Engine::new();
    let ast = match engine.compile_expression(formula) {
        Ok(ast) => ast,
        Err(why) => return format!("refused before any row: {why}"),
    };
    let mut scope = ::rhai::Scope::new();
    scope.push("fails", 5.0_f64).push("flows", 200.0_f64).push("zero", 0.0_f64);
    match engine.eval_ast_with_scope::<::rhai::Dynamic>(&mut scope, &ast) {
        Ok(value) => format!("{value:?}"),
        Err(why) => format!("fails at the row: {why}"),
    }
}
