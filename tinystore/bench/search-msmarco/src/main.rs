//! MS MARCO passage v1 for the search candidate: one token stream for every
//! engine, the lab's compact blocks, FTS5 and Tantivy over it, an exhaustive
//! scorer apart from them, and the timing and writing harnesses.
//!
//! ```text
//! msmarco prepare <corpus> <out> [threads] [passages]   the token stream and PISA's binary collection
//! msmarco queries <corpus> <out>                        the query sets in the same tokens
//! msmarco build <compact|fts5|tantivy|tantivy-keyed> <out> <index>   an index from the prepared collection
//! msmarco verify <subset> <passages>                    the streamed compact build against the lab's
//! msmarco answer <engine> <index> <queries> <results>   each query's ten, with their scores
//! msmarco exact <out> <queries> <results> [threads]     every posting scored, each BM25 variant
//! msmarco time <engine> <index> <queries> <results> <warm|cold>   the timing harness's pass
//! msmarco churn <engine> <out> <index> <readers> <rate> <seconds> <seed>   readers beside a writer
//! msmarco fast-build <compact.db>                     the fast reader's fronts and seeds, fast.db beside it
//! msmarco profile <compact.db> <queries> <results>    the fast reader's parts timed a query, warm
//! msmarco fast2-build <compact.db>                    the read layout, fast2.db beside it
//! msmarco profile2 <compact.db> <queries> <results>   the fast2 reader's parts a query, warm
//! msmarco fields-...                                   search-fields' two fields and prefixes: see fields.rs
//! ```
//!
//! An engine is `compact` (window executor, read ahead 8), `compact-window`
//! (no read ahead), `compact-cursor`, `compact-exhaustive`, `compact-lab`
//! (the lab's own four-field reader, LAB_EXECUTOR=window), `fts5`,
//! `tantivy` or `tantivy-exhaustive`.

#[path = "../../../search-plan-lab/src/analyze.rs"]
mod analyze;
#[path = "../../../search-plan-lab/src/blocks.rs"]
mod blocks;
#[path = "../../../search-plan-lab/src/compact.rs"]
mod compact;
#[path = "../../../search-plan-lab/src/corpus.rs"]
mod corpus;
#[path = "../../../search-plan-lab/src/fts.rs"]
mod fts;
#[path = "../../../search-plan-lab/src/plan.rs"]
mod plan;

mod churn;
mod collection;
mod compact1;
mod exact;
mod fast;
mod fast2;
mod fields;
mod fts1;
mod prepare;
mod queries;
mod sizes;
mod tant1;
mod tiny1;

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use serde_json::json;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |at: usize| args.get(at).map(String::as_str).unwrap_or_else(|| panic!("argument {at} missing"));
    match arg(0) {
        "prepare" => prepare::prepare(
            Path::new(arg(1)),
            Path::new(arg(2)),
            args.get(3).map_or(4, |n| n.parse().unwrap()),
            args.get(4).map_or(usize::MAX, |n| n.parse().unwrap()),
        ),
        "queries" => queries::queries(Path::new(arg(1)), Path::new(arg(2))),
        "build" => {
            let (dir, index) = (Path::new(arg(2)), Path::new(arg(3)));
            let line = match arg(1) {
                "compact" => compact1::build(dir, index),
                "fts5" => fts1::build(dir, index),
                "tantivy" => tant1::build(dir, index, 1024, false),
                "tantivy-keyed" => tant1::build(dir, index, 1024, true),
                "tinystore" => tiny1::build(dir, index),
                other => panic!("no engine {other}"),
            };
            emit(&line);
        }
        "verify" => emit(&compact1::verify(Path::new(arg(1)), arg(2).parse().unwrap())),
        "answer" => answer(arg(1), Path::new(arg(2)), Path::new(arg(3)), Path::new(arg(4))),
        "exact" => exact::exact(
            Path::new(arg(1)),
            Path::new(arg(2)),
            Path::new(arg(3)),
            args.get(4).map_or(4, |n| n.parse().unwrap()),
        ),
        "time" => time(arg(1), Path::new(arg(2)), Path::new(arg(3)), Path::new(arg(4)), arg(5)),
        "churn" => churn::churn(&args[1..]),
        "sizes" => emit(&sizes::files(Path::new(arg(1)))),
        "fast-build" => emit(&fast::build(Path::new(arg(1)), &Path::new(arg(1)).with_file_name("fast.db"))),
        "profile" => profile(Path::new(arg(1)), Path::new(arg(2)), Path::new(arg(3))),
        "fast2-build" => emit(&fast2::build(Path::new(arg(1)), &Path::new(arg(1)).with_file_name("fast2.db"))),
        "profile2" => profile2(Path::new(arg(1)), Path::new(arg(2)), Path::new(arg(3))),
        "fields-queries" => emit(&fields::queries(Path::new(arg(1)), Path::new(arg(2)))),
        "fields-build" => emit(&match arg(1) {
            "lab" => fields::build_lab(Path::new(arg(2)), Path::new(arg(3)), Path::new(arg(4))),
            "native" => fields::build_native(Path::new(arg(2)), Path::new(arg(3))),
            "tantivy" => fields::build_tantivy(Path::new(arg(2)), Path::new(arg(3)), Path::new(arg(4))),
            other => panic!("no layout {other}"),
        }),
        "fields-answer" => emit(&fields::answer(arg(1), Path::new(arg(2)), Path::new(arg(3)), Path::new(arg(4)))),
        "fields-time" => emit(&fields::time(arg(1), Path::new(arg(2)), Path::new(arg(3)), Path::new(arg(4)), arg(5))),
        other => panic!("no command {other}"),
    }
}

/// An engine that answers a query's terms with its ten and their scores.
pub trait Engine {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)>;
    /// Blocks opened and statements by the last query, where the engine counts them.
    fn counted(&self) -> Option<(usize, usize)> {
        None
    }
}

impl Engine for compact1::Reader {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        compact1::Reader::top(self, terms, k)
    }

    fn counted(&self) -> Option<(usize, usize)> {
        Some((self.opened, self.statements))
    }
}

impl Engine for fast::Reader {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        fast::Reader::top(self, terms, k)
    }

    fn counted(&self) -> Option<(usize, usize)> {
        Some((self.opened, self.statements))
    }
}

impl Engine for fast2::Reader {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        fast2::Reader::top(self, terms, k)
    }

    fn counted(&self) -> Option<(usize, usize)> {
        Some((self.spent.blocks, self.spent.blocks))
    }
}

impl Engine for fts1::Reader {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        fts1::Reader::top(self, terms, k)
    }
}

impl Engine for tiny1::Reader {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        tiny1::Reader::top(self, terms, k)
    }
}

impl Engine for tant1::Reader {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        tant1::Reader::top(self, terms, k)
    }
}

/// The lab's own compact reader, which asks a term's four keys, on this
/// round's one-field build: it must answer exactly as `compact1::Reader`.
struct Lab(compact::Compact);

impl Engine for Lab {
    fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        use plan::Backend;
        let words: Vec<plan::Word> = terms
            .iter()
            .map(|term| plan::Word {
                surface: term.clone(),
                stem: None,
                typos: Vec::new(),
            })
            .collect();
        let ids = self.0.top(&words, false, k);
        self.0.scores.iter().copied().zip(ids).collect()
    }
}

pub fn open_engine(name: &str, index: &Path) -> Box<dyn Engine> {
    use compact1::{Executor, Reader};
    match name {
        "compact" => Box::new(Reader::open(index, Executor::Window, 8)),
        "compact-window" => Box::new(Reader::open(index, Executor::Window, 1)),
        "compact-cursor" => Box::new(Reader::open(index, Executor::Cursor, 1)),
        "compact-exhaustive" => Box::new(Reader::open(index, Executor::Exhaustive, 1)),
        "compact-lab" => {
            assert_eq!(std::env::var("LAB_EXECUTOR").as_deref(), Ok("window"));
            Box::new(Lab(compact::Compact::open(index)))
        }
        "fast" => Box::new(fast::Reader::open(index, fast::Switches::from_env())),
        "fast2" => Box::new(fast2::Reader::open(index)),
        "fts5" => Box::new(fts1::Reader::open(index)),
        "tantivy" => Box::new(tant1::Reader::open(index, tantivy::ReloadPolicy::Manual)),
        "tinystore" => Box::new(tiny1::Reader::open(index)),
        "tantivy-exhaustive" => {
            let mut reader = tant1::Reader::open(index, tantivy::ReloadPolicy::Manual);
            reader.exhaustive = true;
            Box::new(reader)
        }
        other => panic!("no engine {other}"),
    }
}

/// Answers every query of a file; the pids are the documents' numbers less one.
fn answer(name: &str, index: &Path, queries: &Path, results: &Path) {
    let started = Instant::now();
    let mut engine = open_engine(name, index);
    let asked = queries::read(queries);
    let mut out = std::io::BufWriter::new(std::fs::File::create(results).unwrap());
    let mut opened = 0usize;
    for (qid, terms) in &asked {
        let took = Instant::now();
        let found = engine.top(terms, 10);
        let micros = took.elapsed().as_secs_f64() * 1e6;
        let ten: Vec<_> = found.iter().map(|(score, doc)| json!([doc - 1, score])).collect();
        let mut line = json!({"qid": qid, "ten": ten, "us": micros});
        if let Some((blocks, statements)) = engine.counted() {
            line["blocks"] = json!(blocks);
            line["statements"] = json!(statements);
            opened += blocks;
        }
        writeln!(out, "{line}").unwrap();
    }
    out.flush().unwrap();
    emit(&json!({"step": "answer", "engine": name, "queries": asked.len(), "blocks_opened": opened,
        "seconds": started.elapsed().as_secs_f64(), "memory": memory(),
        "note": "a run for answers; its times were taken on a busy machine and are not a measurement"}));
}

/// One pass of the timing harness: with `warm`, every query once untimed
/// first; then every query once, timed from its terms to its ten ids. The
/// caller evicts the index's pages before a `cold` pass.
fn time(name: &str, index: &Path, queries: &Path, results: &Path, mode: &str) {
    let opened = Instant::now();
    let mut engine = open_engine(name, index);
    let open_ms = opened.elapsed().as_secs_f64() * 1e3;
    let asked = queries::read(queries);
    if mode == "warm" {
        for (_, terms) in &asked {
            std::hint::black_box(engine.top(terms, 10));
        }
    } else {
        assert_eq!(mode, "cold", "a pass is warm or cold");
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(results).unwrap());
    for (qid, terms) in &asked {
        let took = Instant::now();
        let found = engine.top(terms, 10);
        let nanos = took.elapsed().as_nanos();
        let ids: Vec<u32> = found.iter().map(|(_, doc)| doc - 1).collect();
        writeln!(out, "{}", json!({"qid": qid, "ns": nanos as u64, "ids": ids})).unwrap();
    }
    out.flush().unwrap();
    emit(&json!({"step": "time", "engine": name, "mode": mode, "queries": asked.len(), "open_ms": open_ms,
        "memory": memory()}));
}

/// The fast reader's parts a query, every query once untimed first: its
/// lists, its statements, its decoding and lengths, and the whole, with the
/// blocks, statements, postings decoded and postings scored.
fn profile(index: &Path, queries: &Path, results: &Path) {
    let mut switches = fast::Switches::from_env();
    switches.profile = true;
    let mut reader = fast::Reader::open(index, switches);
    let asked = queries::read(queries);
    for (_, terms) in &asked {
        std::hint::black_box(reader.top(terms, 10));
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(results).unwrap());
    for (qid, terms) in &asked {
        let found = reader.top(terms, 10);
        let ids: Vec<u32> = found.iter().map(|(_, doc)| doc - 1).collect();
        let s = reader.spent;
        writeln!(out, "{}", json!({"qid": qid, "terms": terms.len(), "ns": s.total.as_nanos() as u64,
            "lists_ns": s.lists.as_nanos() as u64, "statements_ns": s.statements.as_nanos() as u64,
            "decode_ns": s.decode.as_nanos() as u64, "blocks": reader.opened, "statements": reader.statements,
            "postings": s.postings, "scored": s.scored, "ids": ids})).unwrap();
    }
    out.flush().unwrap();
    emit(&json!({"step": "profile", "engine": "fast", "switches": switches.to_json(), "queries": asked.len(),
        "memory": memory()}));
}

/// The fast2 reader's lists and whole a query, warm, with blocks and postings scored.
fn profile2(index: &Path, queries: &Path, results: &Path) {
    let mut reader = fast2::Reader::open(index);
    let asked = queries::read(queries);
    for (_, terms) in &asked {
        std::hint::black_box(reader.top(terms, 10));
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(results).unwrap());
    for (qid, terms) in &asked {
        let found = reader.top(terms, 10);
        let ids: Vec<u32> = found.iter().map(|(_, doc)| doc - 1).collect();
        let s = reader.spent;
        writeln!(out, "{}", json!({"qid": qid, "terms": terms.len(), "ns": s.total.as_nanos() as u64,
            "lists_ns": s.lists.as_nanos() as u64, "blocks": s.blocks, "scored": s.scored, "ids": ids})).unwrap();
    }
    out.flush().unwrap();
    emit(&json!({"step": "profile2", "engine": "fast2", "queries": asked.len(), "memory": memory()}));
}

/// The process's peak and current resident memory in KiB, on Linux.
pub fn memory() -> serde_json::Value {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return json!(null);
    };
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().trim_end_matches(" kB").parse::<u64>().ok())
    };
    json!({"hwm_kib": field("VmHWM:"), "rss_kib": field("VmRSS:")})
}

pub fn emit(line: &serde_json::Value) {
    println!("{line}");
}
