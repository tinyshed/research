//! An opt-in experiment. See README.md here for the build flag and boundaries.

mod bench;
mod demo;
mod journal;
mod node;
mod outbox;
mod packet;
mod pages;
mod probe;
mod transport;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(std::io::Error::other(message).into())
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let command = args.next().unwrap_or_else(|| "demo".into());
    if command == "probe" {
        return probe::run();
    }
    if command == "bench" {
        return bench::run();
    }
    if command == "batch" {
        return bench::batching();
    }
    require(
        command == "demo",
        "usage: tinystore-replication-spike [demo [directory] | probe | bench | batch]",
    )?;
    let dir = args.next().map(std::path::PathBuf::from);
    let temporary = tempfile::tempdir()?;
    demo::run(dir.as_deref().unwrap_or_else(|| temporary.path()))
}

#[cfg(test)]
mod tests;
