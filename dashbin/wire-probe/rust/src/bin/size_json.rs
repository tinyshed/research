// the frames, written and read back one way
use wire_probe::{data, json};

fn main() {
    let frame = data::mixed(std::env::args().count());
    let bytes = json::encode(&frame, true);
    let back = json::decode(std::hint::black_box(&bytes)).unwrap();
    println!("{} {}", bytes.len(), back.columns.len());
}
