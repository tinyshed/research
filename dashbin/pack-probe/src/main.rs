//! Compiles v2fly's domain lists and DB-IP's ranges into the tables a pack would hold, then
//! measures each: its text on disk, the time to load it, the memory it keeps and one lookup.
//!
//! `pack-probe <data>`, where `<data>` holds `geosite/data/`, `dbip-country-lite.csv` and
//! `dbip-asn-lite.csv`, as the README's commands leave them. It writes `<data>/out/*.tsv`.

mod compact;
mod counting;
mod domains;
mod probed;
mod ranges;

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOCATOR: counting::Counting = counting::Counting;

const LOOKUPS: usize = 1_000_000;
const RUNS: usize = 5;

fn main() {
    let data = PathBuf::from(std::env::args().nth(1).expect("the data directory"));
    let out = data.join("out");
    std::fs::create_dir_all(&out).expect("out");

    println!("| table | rows | file KiB | load ms | held KiB | lookup ns | found |");
    println!("|---|---|---|---|---|---|---|");
    measure_domains(&data, &out);
    measure_ranges(&data, &out, "country", "dbip-country-lite.csv");
    measure_ranges(&data, &out, "asn", "dbip-asn-lite.csv");
}

fn measure_domains(data: &Path, out: &Path) {
    let compiled = domains::compile(&data.join("geosite/data"));
    eprintln!(
        "domains: {} lists, {} suffix, {} full, {} includes; left out {} regexp, {} keyword; \
         {} keys written in several lists",
        compiled.lists, compiled.suffix, compiled.full, compiled.includes, compiled.regexp, compiled.keyword,
        compiled.contested,
    );
    let text = out.join("domains.tsv");
    std::fs::write(&text, &compiled.text).expect("domains.tsv");
    let asked = domains::asked(&compiled.text, LOOKUPS);

    let (load, held, hashed) = loaded(|| domains::Hashed::read(&text));
    let (each, found) = timed(&asked, |name| hashed.find(name).is_some());
    row("domains, a hash map", compiled.suffix + compiled.full, &text, load, held, each, found);
    drop(hashed);

    let (load, held, sorted) = loaded(|| domains::Sorted::read(&text));
    let (each, found) = timed(&asked, |name| sorted.find(name).is_some());
    row("domains, one sorted text", compiled.suffix + compiled.full, &text, load, held, each, found);
    drop(sorted);

    let hashed = domains::Hashed::read(&text);
    let (load, held, probed) = loaded(|| probed::Probed::read(&text));
    assert!(asked.iter().all(|name| probed.find(name) == hashed.find(name)), "the two hashes agree");
    let (each, found) = timed(&asked, |name| probed.find(name).is_some());
    row("domains, one text and slots", compiled.suffix + compiled.full, &text, load, held, each, found);
}

fn measure_ranges(data: &Path, out: &Path, name: &str, file: &str) {
    let compiled = ranges::compile(&data.join(file));
    eprintln!(
        "{name}: {} v4 ranges, {} v6, {} of them whole /64s, {} values, in order: {}",
        compiled.v4, compiled.v6, compiled.v6_whole, compiled.values, compiled.ordered,
    );
    let text = out.join(format!("{name}.tsv"));
    std::fs::write(&text, &compiled.text).expect("ranges");
    let asked = ranges::asked(LOOKUPS);

    let (load, held, table) = loaded(|| ranges::Table::read(&text));
    let (each, found) = timed(&asked, |address| table.find(*address).is_some());
    row(&format!("{name}, sorted ranges"), compiled.v4 + compiled.v6, &text, load, held, each, found);

    let (load, held, small) = loaded(|| compact::Compact::read(&text));
    assert!(asked.iter().all(|address| small.find(*address) == table.find(*address)), "the two tables agree");
    let (each, found) = timed(&asked, |address| small.find(*address).is_some());
    row(&format!("{name}, starts alone, from text"), compiled.v4 + compiled.v6, &text, load, held, each, found);

    let bytes = out.join(format!("{name}.bin"));
    std::fs::write(&bytes, small.bytes()).expect("bytes");
    drop(small);
    let (load, held, small) = loaded(|| compact::Compact::load(&bytes));
    assert!(asked.iter().all(|address| small.find(*address) == table.find(*address)), "the bytes agree");
    let (each, found) = timed(&asked, |address| small.find(*address).is_some());
    row(&format!("{name}, starts alone, from bytes"), compiled.v4 + compiled.v6, &bytes, load, held, each, found);
}

/// The median time of `RUNS` loads, and what the last one keeps allocated.
fn loaded<T>(load: impl Fn() -> T) -> (Duration, usize, T) {
    let mut times: Vec<Duration> = (0..RUNS - 1)
        .map(|_| {
            let started = Instant::now();
            drop(black_box(load()));
            started.elapsed()
        })
        .collect();
    let before = counting::live();
    let started = Instant::now();
    let kept = load();
    times.push(started.elapsed());
    let held = counting::live() - before;
    times.sort();
    (times[RUNS / 2], held, kept)
}

/// The nanoseconds one lookup takes, the median of `RUNS` passes over `asked`, and how many found.
fn timed<A>(asked: &[A], find: impl Fn(&A) -> bool) -> (f64, usize) {
    let mut found = 0;
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            found = asked.iter().filter(|one| black_box(find(one))).count();
            started.elapsed()
        })
        .collect();
    times.sort();
    (times[RUNS / 2].as_nanos() as f64 / asked.len() as f64, found)
}

fn row(name: &str, rows: usize, text: &Path, load: Duration, held: usize, each: f64, found: usize) {
    let size = std::fs::metadata(text).expect("text").len();
    println!(
        "| {name} | {rows} | {} | {:.1} | {} | {each:.0} | {:.0}% |",
        size / 1024,
        load.as_secs_f64() * 1000.0,
        held / 1024,
        found as f64 * 100.0 / LOOKUPS as f64,
    );
}
