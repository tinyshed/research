mod analyzer;
mod fts;
#[cfg(feature = "text")]
mod tant;
mod experiment;
fn main() { experiment::main(); }
