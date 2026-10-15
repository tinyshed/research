mod baseline;
mod bench;
mod capture;
mod feed;
mod packet;
mod pages;
mod replica;
mod tree;
mod wal;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(std::io::Error::other(message).into())
    }
}

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("bench") => bench::run(),
        Some("network") => bench::network(),
        _ => Err("usage: tinystore-wal-spike bench|network".into()),
    }
}

#[cfg(test)]
mod tests;
