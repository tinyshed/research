// the frames alone, so the other three can be measured against it
use wire_probe::data;

fn main() {
    let frame = data::mixed(std::env::args().count());
    println!("{}", std::hint::black_box(frame).columns.len());
}
