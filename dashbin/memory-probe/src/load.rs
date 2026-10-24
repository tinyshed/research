// a keep-alive http/1.1 load client: N connections, each sending its share of requests in turn
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

async fn connection(path: String, count: usize) -> Vec<Duration> {
    let stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let (read, mut write) = stream.into_split();
    let mut read = BufReader::new(read);
    let request = format!("GET /{path} HTTP/1.1\r\nHost: probe\r\n\r\n");
    let mut took = Vec::with_capacity(count);
    let mut line = String::new();
    for _ in 0..count {
        let started = Instant::now();
        write.write_all(request.as_bytes()).await.unwrap();
        let mut length = 0usize;
        loop {
            line.clear();
            read.read_line(&mut line).await.unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        read.read_exact(&mut body).await.unwrap();
        took.push(started.elapsed());
    }
    took
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (path, total, conns) = (args[1].clone(), args[2].parse::<usize>().unwrap(), args[3].parse::<usize>().unwrap());
    let started = Instant::now();
    let tasks: Vec<_> = (0..conns).map(|_| tokio::spawn(connection(path.clone(), total / conns))).collect();
    let mut all = Vec::new();
    for t in tasks {
        all.extend(t.await.unwrap());
    }
    let wall = started.elapsed();
    all.sort();
    let at = |q: f64| all[((all.len() as f64 * q) as usize).min(all.len() - 1)];
    println!(
        "{:>6.0} req/s  p50 {:>6.2} ms  p95 {:>6.2} ms  p99 {:>6.2} ms",
        all.len() as f64 / wall.as_secs_f64(),
        at(0.50).as_secs_f64() * 1000.0,
        at(0.95).as_secs_f64() * 1000.0,
        at(0.99).as_secs_f64() * 1000.0
    );
}
