mod capture;
mod feed;
mod load;
mod packet;
mod pages;
mod stats;
#[cfg(test)]
mod tests;
mod tree;
mod wal;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(std::io::Error::other(message).into())
    }
}
fn main() -> Result<()> {
    load::run()
}
