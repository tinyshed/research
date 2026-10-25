// writes every dataset in every encoding, proves each reads back as it was, and prints sizes and times
use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::time::Instant;
use wire_probe::{arrow, data, json, own, same, Frame};

type Encode = fn(&Frame) -> Vec<u8>;
type Decode = fn(&[u8]) -> Result<Frame, String>;

const ROUNDS: usize = 21;

fn median_ms(mut f: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..ROUNDS)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[ROUNDS / 2]
}

fn gzip(bytes: &[u8]) -> usize {
    let mut z = GzEncoder::new(Vec::new(), Compression::new(6));
    z.write_all(bytes).unwrap();
    z.finish().unwrap().len()
}

// what a browser accepts only over https, so a LAN install without TLS gets gzip
fn zstd(bytes: &[u8]) -> usize {
    zstd::bulk::compress(bytes, 3).unwrap().len()
}

fn brotli(bytes: &[u8]) -> usize {
    let mut out = Vec::new();
    let params = brotli::enc::BrotliEncoderParams { quality: 5, ..Default::default() };
    brotli::BrotliCompress(&mut &bytes[..], &mut out, &params).unwrap();
    out.len()
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "out".into());
    let rows: usize = std::env::args().nth(2).map_or(50_000, |r| r.parse().unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let encodings: [(&str, Encode, Decode); 8] = [
        ("json", |f| json::encode(f, false), json::decode),
        ("arrow", |f| arrow::encode(f, false), arrow::decode),
        ("own-i64", |f| own::encode(f, false, false), own::decode),
        ("own-f64", |f| own::encode(f, true, false), own::decode),
        ("json-dict", |f| json::encode(f, true), json::decode),
        ("arrow-dict", |f| arrow::encode(f, true), arrow::decode),
        ("own-i64-dict", |f| own::encode(f, false, true), own::decode),
        ("own-f64-dict", |f| own::encode(f, true, true), own::decode),
    ];
    println!("rows {rows}, gzip 6, zstd 3, brotli 5, median of {ROUNDS}");
    println!(
        "{:<7} {:<12} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "dataset", "format", "raw", "gzip", "zstd", "brotli", "encode ms", "decode ms"
    );
    for (name, frame) in [("mixed", data::mixed(rows)), ("series", data::series(rows))] {
        for (format, encode, decode) in encodings {
            let bytes = encode(&frame);
            if let Err(e) = decode(&bytes).and_then(|back| same(&frame, &back)) {
                panic!("{name} {format} does not read back: {e}");
            }
            let encode_ms = median_ms(|| drop(std::hint::black_box(encode(&frame))));
            let decode_ms = median_ms(|| drop(std::hint::black_box(decode(&bytes))));
            std::fs::write(format!("{out}/{name}.{format}"), &bytes).unwrap();
            println!(
                "{name:<7} {format:<12} {:>10} {:>10} {:>10} {:>10} {encode_ms:>10.2} {decode_ms:>10.2}",
                bytes.len(),
                gzip(&bytes),
                zstd(&bytes),
                brotli(&bytes)
            );
        }
    }
}
