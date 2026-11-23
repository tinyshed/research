//! A pack kept in TinyStore as a hub would keep it, and the other way a table could be held there.
//!
//! `stored <data>`, after `pack-probe <data>` wrote `<data>/out/`. Two questions:
//!
//! 1. Files. A pack's icons and its tables' bytes as a set of files inside the hub's database, put
//!    in one transaction. How long is the putting, what does the disk hold, how long is one icon
//!    read and one table read back and made ready?
//! 2. Rows. The same tables as SQL rows the store looks up itself, nothing held in memory. How
//!    long is a lookup then, beside the 39 ns and 153 ns of a table in memory?

#[path = "../compact.rs"]
mod compact;
#[path = "../domains.rs"]
#[allow(dead_code, reason = "the probe's first binary uses the rest")]
mod domains;
#[path = "../ranges.rs"]
#[allow(dead_code, reason = "the probe's first binary uses the rest")]
mod ranges;

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tinystore::{Store, sql};

const ASKED: usize = 100_000;

#[derive(serde::Deserialize)]
struct Found {
    value: String,
}

fn main() -> tinystore::Result<()> {
    let data = PathBuf::from(std::env::args().nth(1).expect("the data directory"));
    let out = data.join("out");
    let dir = data.join("store");
    let _ = std::fs::remove_dir_all(&dir);

    let store = Store::open(&dir, tinystore::Options::default())?;
    let db = store.database("hub").open()?;
    let packs = db.files("packs").open()?;
    println!("| what | count | ms | each µs |");
    println!("|---|---|---|---|");

    // 1. files
    let icons = files_of(&data.join("simple-icons/icons"));
    let (took, ()) = timed(|| {
        db.tx(|tx| {
            for (name, bytes) in &icons {
                tx.with(&packs).key(&format!("brands/icons/{name}")).content_type("image/svg+xml").put(bytes)?;
            }
            Ok::<_, tinystore::Error>(())
        })
    })?;
    row("3 475 icons put, one transaction", icons.len(), took);

    let apart = &icons[..200];
    let (took, ()) = timed(|| apart.iter().try_for_each(|(name, bytes)| packs.put(&format!("apart/{name}"), bytes).map(drop)))?;
    row("icons put, a transaction each", apart.len(), took);

    for table in ["country.bin", "asn.bin", "domains.tsv"] {
        let bytes = std::fs::read(out.join(table)).expect("a table");
        let (took, ()) = timed(|| packs.put(&format!("geo/tables/{table}"), &bytes).map(drop))?;
        row(&format!("{table} put, {} KiB", bytes.len() / 1024), 1, took);
    }
    println!("| the store's directory, KiB | {} | | |", held(&dir) / 1024);

    let (took, read) = timed(|| {
        icons.iter().try_fold(0, |read, (name, _)| {
            let mut file = packs.get(&format!("brands/icons/{name}"))?.expect("an icon put");
            Ok::<_, tinystore::Error>(read + file.read_all()?.len())
        })
    })?;
    row(&format!("icons read whole, {} KiB", read / 1024), icons.len(), took);

    drop((packs, db, store));
    let (took, store) = timed(|| Store::open(&dir, tinystore::Options::default()))?;
    row("the store opened again", 1, took);
    let db = store.database("hub").open()?;
    let packs = db.files("packs").open()?;
    for table in ["country.bin", "asn.bin"] {
        let (took, bytes) = timed(|| packs.get(&format!("geo/tables/{table}"))?.expect("a table put").read_all())?;
        row(&format!("{table} read whole, its SHA-256 checked"), 1, took);
        let started = Instant::now();
        let ready = compact::Compact::of(&bytes);
        row(&format!("{table} made a table from those bytes"), 1, started.elapsed());
        drop(ready);
    }

    // 2. rows
    db.exec(sql!("create table domain (key text primary key, value text not null) without rowid"))?;
    db.exec(sql!("create table range4 (first integer primary key, last integer not null, value text not null)"))?;
    let domains = std::fs::read_to_string(out.join("domains.tsv")).expect("domains");
    let (took, rows) = timed(|| {
        db.tx(|tx| {
            domains.lines().try_fold(0, |rows, line| {
                let (key, value) = line.split_once('\t').expect("a key and a list");
                tx.exec(sql!("insert into domain (key, value) values (?, ?)", key, value))?;
                Ok::<_, tinystore::Error>(rows + 1)
            })
        })
    })?;
    row("domains inserted as rows", rows, took);
    let countries = std::fs::read_to_string(out.join("country.tsv")).expect("countries");
    let (took, rows) = timed(|| {
        db.tx(|tx| {
            countries.lines().try_fold(0, |rows, line| {
                let mut fields = line.splitn(3, '\t');
                let (Some(Ok(IpAddr::V4(first))), Some(Ok(IpAddr::V4(last))), Some(value)) =
                    (fields.next().map(str::parse), fields.next().map(str::parse), fields.next())
                else {
                    return Ok(rows);
                };
                tx.exec(sql!("insert into range4 (first, last, value) values (?, ?, ?)", first.to_bits(), last.to_bits(), value))?;
                Ok::<_, tinystore::Error>(rows + 1)
            })
        })
    })?;
    row("v4 countries inserted as rows", rows, took);
    println!("| the store's directory with the rows, KiB | {} | | |", held(&dir) / 1024);

    let names = domains::asked(&domains, ASKED);
    let (took, found) = timed(|| {
        names.iter().try_fold(0, |found, name| {
            let exact = format!("={name}");
            let suffixes = std::iter::once(name.as_str()).chain(name.match_indices('.').map(|(at, _)| &name[at + 1..]));
            for key in std::iter::once(exact.as_str()).chain(suffixes) {
                if db.one::<Found>(sql!("select value from domain where key = ?", key))?.is_some() {
                    return Ok(found + 1);
                }
            }
            Ok::<_, tinystore::Error>(found)
        })
    })?;
    row(&format!("a domain looked up in rows, {found} found"), names.len(), took);

    let addresses: Vec<u32> = ranges::asked(ASKED * 2).into_iter().filter_map(|one| match one {
        IpAddr::V4(one) => Some(one.to_bits()),
        IpAddr::V6(_) => None,
    }).collect();
    let (took, found) = timed(|| {
        addresses.iter().try_fold(0, |found, address| {
            let one = db.one::<Found>(sql!(
                "select value from range4 where first <= ? and last >= ? order by first desc limit 1",
                address, address
            ))?;
            Ok::<_, tinystore::Error>(found + usize::from(one.is_some_and(|one| !one.value.is_empty())))
        })
    })?;
    row(&format!("a v4 address looked up in rows, {found} found"), addresses.len(), took);
    Ok(())
}

fn timed<T>(work: impl FnOnce() -> tinystore::Result<T>) -> tinystore::Result<(Duration, T)> {
    let started = Instant::now();
    let done = work()?;
    Ok((started.elapsed(), done))
}

fn row(what: &str, count: usize, took: Duration) {
    let each = took.as_secs_f64() * 1e6 / count as f64;
    println!("| {what} | {count} | {:.1} | {each:.1} |", took.as_secs_f64() * 1e3);
}

fn files_of(directory: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .expect("a directory")
        .map(|entry| entry.expect("an entry").path())
        .map(|path| (path.file_name().expect("a name").to_string_lossy().into_owned(), std::fs::read(&path).expect("a file")))
        .collect();
    files.sort();
    files
}

fn held(directory: &Path) -> u64 {
    std::fs::read_dir(directory)
        .expect("a directory")
        .map(|entry| entry.expect("an entry"))
        .map(|entry| if entry.path().is_dir() { held(&entry.path()) } else { entry.metadata().expect("a file").len() })
        .sum()
}
