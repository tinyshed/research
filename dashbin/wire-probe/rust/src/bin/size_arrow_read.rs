// the frames, and Arrow only to read them
use wire_probe::{arrow, data};

fn main() {
    let frame = data::mixed(std::env::args().count());
    let bytes = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let back = arrow::decode(&bytes).unwrap();
    println!("{}", back.columns.len() + frame.columns.len());
}
