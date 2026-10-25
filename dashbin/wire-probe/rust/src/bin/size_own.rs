// the frames, written and read back one way
use wire_probe::{data, own};

fn main() {
    let frame = data::mixed(std::env::args().count());
    let bytes = own::encode(&frame, true);
    let back = own::decode(std::hint::black_box(&bytes)).unwrap();
    println!("{} {}", bytes.len(), back.columns.len());
}
