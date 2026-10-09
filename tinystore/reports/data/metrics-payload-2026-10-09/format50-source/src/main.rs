//! Experimental format 0x50: current v4 metadata plus compact pack addresses.
//! This program does not write a production-compatible pack database.
#![allow(dead_code, unused_imports)]
mod engine {
    #[derive(Clone, Copy, Debug)]
    pub struct Sample {
        pub at: i64,
        pub value: f64,
    }
}
#[path = "../../metrics-bench/rust/src/codec.rs"]
mod codec;
#[path = "../../metrics-bench/rust/src/exact.rs"]
mod exact;
mod pack;
mod study;
use std::{env, path::Path};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("build") => study::build(Path::new(&args[2]), Path::new(&args[3]), &args[4])?,
        Some("inspect") => println!("{}", study::inspect(Path::new(&args[2]))?),
        Some("verify") => println!("{}", study::verify(Path::new(&args[2]), Path::new(&args[3]))?),
        Some("verify-bytes") => println!("{}", study::verify_bytes(Path::new(&args[2]), Path::new(&args[3]))?),
        Some("bench") => study::bench(Path::new(&args[2]), &args[3], &args[4], &args[5], args[6].parse()?)?,
        Some("diagnose") => study::diagnose(Path::new(&args[2]), &args[3], &args[4], &args[5])?,
        Some("lifecycle") => study::lifecycle(Path::new(&args[2]), Path::new(&args[3]))?,
        _ => return Err("usage: build input.db output.db baseline|countN|bytesN; inspect db; verify original candidate; bench db whole|range point|sparse|range|full|summary warm|cold iterations; lifecycle input output-directory".into()),
    }
    Ok(())
}
