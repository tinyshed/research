//! H2.3's harness: readers beside one writer, on a copy of an index.
//!
//! `churn <engine> <out> <index> <readers> <rate> <seconds> <seed> <results>`
//!
//! `readers` threads each answer the sample's queries in a seeded order, one
//! after another, each timed from its terms to its ten. One writer sets
//! `rate` passages a second, open loop: every 100 ms it commits the sets due
//! since the last commit, each a random passage given another random
//! passage's tokens, and a set's latency runs from when it was due to when
//! its commit returned. The compact blocks pack their head beside the writes:
//! after a commit that leaves 20,000 rows or more in the head, one step of
//! 4,000 rows, timed as the writer's too. Readers warm for five seconds
//! before the writer starts; only what follows is kept.
//!
//! Each engine's reader sees the writer's commits as a store's would: the
//! compact reader reads the statistics and the gone again when SQLite's
//! data_version moves, FTS5 reads each query in a fresh snapshot, and
//! Tantivy's reader reloads on commit and each query takes its latest
//! searcher.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::analyze::{Analysis, Analyzer};
use crate::compact1::{Executor, Reader};
use crate::corpus::Doc;

const TICK: Duration = Duration::from_millis(100);
const WARM: Duration = Duration::from_secs(5);
const PACK_AT: i64 = 20_000;
const PACK_ROWS: usize = 4_000;

/// A small seeded generator, splitmix64, so that a run's choices repeat.
pub struct Seeded(u64);

impl Seeded {
    pub fn new(seed: u64) -> Seeded {
        Seeded(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let at = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[at]
}

fn summary(mut values: Vec<f64>) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    json!({"count": values.len(), "p50": quantile(&values, 0.5), "p99": quantile(&values, 0.99),
           "max": values.last().copied().unwrap_or(f64::NAN)})
}

/// The tokens of the passages a run sets, read once from tokens.tsv.
fn passages(out: &Path, wanted: &HashSet<u32>) -> std::collections::HashMap<u32, String> {
    let mut found = std::collections::HashMap::new();
    for line in BufReader::with_capacity(1 << 22, File::open(out.join("tokens.tsv")).unwrap()).lines() {
        let line = line.unwrap();
        let (pid, tokens) = line.split_once('\t').unwrap();
        let pid: u32 = pid.parse().unwrap();
        if wanted.contains(&pid) {
            found.insert(pid, tokens.to_owned());
        }
    }
    found
}

enum Writer {
    Compact(crate::compact::Writer, Analyzer),
    Fts5(crate::fts1::Writer),
    Tantivy(crate::tant1::Writer),
}

impl Writer {
    /// Commits the sets; says the milliseconds of a head-packing step it took after, if any.
    fn set(&mut self, sets: &[(u32, String)]) -> Option<f64> {
        match self {
            Writer::Compact(writer, analyzer) => {
                let docs: Vec<Doc> = sets
                    .iter()
                    .map(|(pid, tokens)| Doc {
                        id: pid.to_string(),
                        title: String::new(),
                        text: tokens.clone(),
                    })
                    .collect();
                let refs: Vec<(u32, &Doc)> = docs.iter().map(|doc| (0, doc)).collect();
                writer.set(analyzer, &refs);
                if writer.head_rows() >= PACK_AT {
                    return Some(writer.pack_step(PACK_ROWS).millis);
                }
                None
            }
            Writer::Fts5(writer) => {
                let rows: Vec<(u32, String)> = sets.iter().map(|(pid, tokens)| (pid + 1, tokens.clone())).collect();
                writer.set(&rows);
                None
            }
            Writer::Tantivy(writer) => {
                let rows: Vec<(u64, String)> =
                    sets.iter().map(|(pid, tokens)| (u64::from(*pid), tokens.clone())).collect();
                writer.set(&rows);
                None
            }
        }
    }
}

pub fn churn(args: &[String]) {
    let [engine, out, index, readers, rate, seconds, seed, results] = args else {
        panic!("churn <engine> <out> <index> <readers> <rate> <seconds> <seed> <results>");
    };
    let (out, index, results) = (Path::new(out), Path::new(index), Path::new(results));
    let readers: usize = readers.parse().unwrap();
    let rate: f64 = rate.parse().unwrap();
    let seconds: f64 = seconds.parse().unwrap();
    let seed: u64 = seed.parse().unwrap();
    let asked = Arc::new(crate::queries::read(&out.join("q/sample.tsv")));
    let documents = crate::collection::Collection::open(out).documents;
    let total_sets = (rate * seconds).ceil() as usize;
    let mut chooser = Seeded::new(seed);
    let plan: Vec<(u32, u32)> = (0..total_sets)
        .map(|_| (chooser.below(u64::from(documents)) as u32, chooser.below(u64::from(documents)) as u32))
        .collect();
    let wanted: HashSet<u32> = plan.iter().map(|(_, source)| *source).collect();
    let texts = passages(out, &wanted);
    let stop = Arc::new(AtomicBool::new(false));
    let start = Arc::new(Barrier::new(readers + 1));
    let mut handles = Vec::new();
    for reader in 0..readers {
        let (asked, stop, start) = (Arc::clone(&asked), Arc::clone(&stop), Arc::clone(&start));
        let (engine, index) = (engine.clone(), index.to_path_buf());
        handles.push(std::thread::spawn(move || {
            let mut open: Box<dyn crate::Engine> = match engine.as_str() {
                "compact" => Box::new(Reader::open(&index, Executor::Window, 8)),
                "fts5" => Box::new(crate::fts1::Reader::open(&index)),
                "tantivy" => Box::new(Refreshing(crate::tant1::Reader::open(
                    &index,
                    tantivy::ReloadPolicy::OnCommitWithDelay,
                ))),
                other => panic!("no engine {other}"),
            };
            let mut order = Seeded::new(seed ^ (reader as u64 + 1));
            start.wait();
            let began = Instant::now();
            let mut kept = Vec::new();
            while !stop.load(Ordering::Relaxed) {
                let (_, terms) = &asked[order.below(asked.len() as u64) as usize];
                let took = Instant::now();
                std::hint::black_box(open.top(terms, 10));
                let ms = took.elapsed().as_secs_f64() * 1e3;
                if began.elapsed() >= WARM {
                    kept.push(ms);
                }
            }
            kept
        }));
    }
    let mut writer = match engine.as_str() {
        "compact" => Writer::Compact(crate::compact::Writer::open(index), Analyzer::new(Analysis::named("surface"))),
        "fts5" => Writer::Fts5(crate::fts1::Writer::open(index)),
        "tantivy" => Writer::Tantivy(crate::tant1::Writer::open(index)),
        other => panic!("no engine {other}"),
    };
    let head_at_start = match &writer {
        Writer::Compact(writer, _) => Some(writer.head_rows()),
        _ => None,
    };
    start.wait();
    std::thread::sleep(WARM);
    let began = Instant::now();
    let (mut set_ms, mut commit_ms, mut pack_ms) = (Vec::new(), Vec::new(), Vec::new());
    let mut next = 0usize;
    let mut tick = 1u32;
    while next < plan.len() {
        let due_by = TICK * tick;
        if let Some(wait) = due_by.checked_sub(began.elapsed()) {
            std::thread::sleep(wait);
        }
        let now = began.elapsed().as_secs_f64();
        let mut due = Vec::new();
        while next < plan.len() && (next as f64) / rate <= now {
            due.push(next);
            next += 1;
        }
        tick += 1;
        if due.is_empty() {
            continue;
        }
        let sets: Vec<(u32, String)> = due
            .iter()
            .map(|at| (plan[*at].0, texts[&plan[*at].1].clone()))
            .collect();
        let committing = Instant::now();
        let packed = writer.set(&sets);
        commit_ms.push(committing.elapsed().as_secs_f64() * 1e3);
        let done = began.elapsed().as_secs_f64();
        for at in &due {
            set_ms.push((done - *at as f64 / rate) * 1e3);
        }
        if let Some(ms) = packed {
            pack_ms.push(ms);
        }
    }
    let wrote = began.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let mut reads = Vec::new();
    for handle in handles {
        reads.extend(handle.join().unwrap());
    }
    let mut raw = File::create(results).unwrap();
    writeln!(raw, "{}", json!({"reads_ms": reads, "sets_ms": set_ms, "commits_ms": commit_ms, "packs_ms": pack_ms}))
        .unwrap();
    let head = match &writer {
        Writer::Compact(writer, _) => Some(writer.head_rows()),
        _ => None,
    };
    crate::emit(&json!({"step": "churn", "engine": engine, "readers": readers, "rate": rate,
        "seconds": wrote, "sets": set_ms.len(), "reads": summary(reads), "set_visible_ms": summary(set_ms),
        "commit_ms": summary(commit_ms), "pack_ms": summary(pack_ms), "head_rows_at_start": head_at_start,
        "head_rows": head, "memory": crate::memory()}));
}

/// A Tantivy reader that takes its latest searcher before each query.
struct Refreshing(crate::tant1::Reader);

impl crate::Engine for Refreshing {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        self.0.refresh_if_reloaded();
        self.0.top(terms, k)
    }
}
