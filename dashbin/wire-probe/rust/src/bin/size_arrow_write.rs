// the frames, and Arrow only to write them
use wire_probe::{arrow, data};

fn main() {
    let frame = data::mixed(std::env::args().count());
    let bytes = arrow::encode(&frame, true);
    println!("{}", std::hint::black_box(bytes).len());
}
