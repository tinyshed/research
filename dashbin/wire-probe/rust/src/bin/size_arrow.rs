// the frames, written and read back one way
use wire_probe::{data, arrow};

fn main() {
    let frame = data::mixed(std::env::args().count());
    let bytes = arrow::encode(&frame, true);
    let back = arrow::decode(std::hint::black_box(&bytes)).unwrap();
    println!("{} {}", bytes.len(), back.columns.len());
}
