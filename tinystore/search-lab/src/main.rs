mod analyzer;
mod experiment;
mod fts;
#[cfg(feature = "text")]
mod tant;
fn main() {
    experiment::main();
}
