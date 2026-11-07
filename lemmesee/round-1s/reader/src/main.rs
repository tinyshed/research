//! Round 1s's key reader.
//!
//!     reader parity <keys.bin> <threshold> <hidden.jsonl> <split> <out>
//!     reader dump <hidden.jsonl> <schema id>
//!     reader bench <keys.bin> <threshold> <hidden.jsonl> <out.json> [<runs> <warm>]
//!     reader memory <keys.bin> <threshold> <hidden.jsonl> <schema id | ->
//!
//! `parity` scores every schema of the split that declares a key, as
//! widen.py builds them, and writes <out>.bin — for each pair a digest of
//! its names, a digest of its hashed features, the first stage's margin
//! (f32) and the second's (f64), little-endian — and <out>.keys.jsonl, the
//! keys drawn. `dump` writes one schema's pairs with their named features.
//! `bench` times each schema of the file, from a parsed schema to its keys,
//! the median of <runs> after <warm>, and reading its JSON apart. `memory`
//! loads the model, reads and scores one schema (none for `-`) and reports
//! the process's resident set.

use reader::{Model, Schema, Scratch, digest, name_digest, pairs_named};
use std::fs;
use std::io::{BufWriter, Write};
use std::time::Instant;

fn load(path: &str) -> Model {
    let data = fs::read(path).expect("read keys.bin");
    Model::read(&data).expect("parse keys.bin")
}

fn schemas(path: &str) -> Vec<String> {
    fs::read_to_string(path).expect("read schemas").lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

fn status() -> serde_json::Value {
    // Linux only: the peak and current resident set, in KiB
    let text = fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        text.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    serde_json::json!({"VmHWM_kib": field("VmHWM:"), "VmRSS_kib": field("VmRSS:")})
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

fn parity(args: &[String]) {
    let model = load(&args[0]);
    let threshold: f64 = args[1].parse().unwrap();
    let split = &args[3];
    let out = &args[4];
    let mut bin = BufWriter::new(fs::File::create(format!("{out}.bin")).unwrap());
    let mut keys = BufWriter::new(fs::File::create(format!("{out}.keys.jsonl")).unwrap());
    let mut scratch = Scratch::default();
    let (mut n_schemas, mut n_pairs) = (0usize, 0usize);
    for line in schemas(&args[2]) {
        let s: Schema = serde_json::from_str(&line).unwrap();
        if s.split != *split || s.fk.is_empty() {
            continue;
        }
        let scored = model.score(&s, threshold, &mut scratch);
        for (i, p) in scored.pairs.iter().enumerate() {
            let t = &s.tables[p.table];
            let names = name_digest(&[&s.id, &t.name, &t.columns[p.column].name, &s.tables[p.target].name]);
            bin.write_all(&names.to_le_bytes()).unwrap();
            bin.write_all(&digest(&p.features).to_le_bytes()).unwrap();
            bin.write_all(&scored.first[i].to_le_bytes()).unwrap();
            bin.write_all(&scored.second[i].to_le_bytes()).unwrap();
        }
        for &i in &scored.drawn {
            let p = &scored.pairs[i];
            let t = &s.tables[p.table];
            let prob = 1.0 / (1.0 + (-scored.second[i]).exp());
            let row = serde_json::json!([s.id, t.name, t.columns[p.column].name, s.tables[p.target].name, prob]);
            writeln!(keys, "{row}").unwrap();
        }
        n_schemas += 1;
        n_pairs += scored.pairs.len();
    }
    eprintln!("{n_schemas} schemas, {n_pairs} pairs");
}

fn dump(args: &[String]) {
    for line in schemas(&args[0]) {
        let s: Schema = serde_json::from_str(&line).unwrap();
        if s.id != args[1] {
            continue;
        }
        let (ps, names) = pairs_named(&s, true);
        for (p, n) in ps.iter().zip(names) {
            let t = &s.tables[p.table];
            let f: serde_json::Map<String, serde_json::Value> =
                n.items.into_iter().map(|(k, v)| (k, serde_json::json!(v))).collect();
            println!("{}", serde_json::json!([t.name, t.columns[p.column].name, s.tables[p.target].name, f]));
        }
    }
}

fn bench(args: &[String]) {
    let t0 = Instant::now();
    let model = load(&args[0]);
    let load_ms = t0.elapsed().as_secs_f64() * 1e3;
    let threshold: f64 = args[1].parse().unwrap();
    let runs: usize = args.get(4).map(|v| v.parse().unwrap()).unwrap_or(21);
    let warm: usize = args.get(5).map(|v| v.parse().unwrap()).unwrap_or(3);
    // A schema whose first run takes longer than this many seconds is timed
    // this many times instead, after one warm run
    let heavy_seconds: f64 = args.get(6).map(|v| v.parse().unwrap()).unwrap_or(f64::INFINITY);
    let heavy_runs: usize = args.get(7).map(|v| v.parse().unwrap()).unwrap_or(runs);
    let mut scratch = Scratch::default();
    let mut rows = Vec::new();
    for line in schemas(&args[2]) {
        let mut parse = Vec::with_capacity(runs);
        let mut s: Option<Schema> = None;
        for _ in 0..runs {
            let t = Instant::now();
            let x: Schema = serde_json::from_str(&line).unwrap();
            parse.push(t.elapsed().as_secs_f64() * 1e6);
            s = Some(x);
        }
        let s = s.unwrap();
        let mut drawn = 0;
        let mut n_pairs = 0;
        let mut first_run = 0.0;
        for w in 0..warm.max(1) {
            let t = Instant::now();
            let r = model.score(&s, threshold, &mut scratch);
            if w == 0 {
                first_run = t.elapsed().as_secs_f64();
            }
            drawn = r.drawn.len();
            n_pairs = r.pairs.len();
            if first_run > heavy_seconds {
                break;
            }
        }
        let these = if first_run > heavy_seconds { heavy_runs } else { runs };
        let mut score = Vec::with_capacity(these);
        for _ in 0..these {
            let t = Instant::now();
            let r = model.score(&s, threshold, &mut scratch);
            score.push(t.elapsed().as_secs_f64() * 1e6);
            drawn = r.drawn.len();
            n_pairs = r.pairs.len();
            std::hint::black_box(&r);
        }
        let columns: usize = s.tables.iter().map(|t| t.columns.len()).sum();
        let min = score.iter().copied().fold(f64::INFINITY, f64::min);
        rows.push(serde_json::json!({
            "id": s.id, "split": s.split, "tables": s.tables.len(), "columns": columns, "pairs": n_pairs,
            "keys drawn": drawn, "declares a key": !s.fk.is_empty(),
            "parse_us": median(&mut parse), "score_us": median(&mut score), "score_us_min": min, "runs": these,
        }));
    }
    let out = serde_json::json!({
        "model bytes": model.bytes, "load_ms": load_ms, "runs": runs, "warm": warm, "schemas": rows,
        "process": status(),
    });
    fs::write(&args[3], serde_json::to_string_pretty(&out).unwrap()).unwrap();
}

fn memory(args: &[String]) {
    let before = status();
    let model = load(&args[0]);
    let loaded = status();
    let threshold: f64 = args[1].parse().unwrap();
    let mut result = serde_json::json!({"started": before, "model loaded": loaded, "model bytes": model.bytes});
    if args[3] != "-" {
        // One line read at a time, the wanted one kept, so the file is not
        // counted in the resident set
        use std::io::BufRead;
        let needle = format!("\"id\": {}", serde_json::to_string(&args[3]).unwrap());
        let reader = std::io::BufReader::new(fs::File::open(&args[2]).expect("read schemas"));
        let line = reader.lines().map_while(Result::ok).find(|l| l.contains(&needle));
        let s: Schema = serde_json::from_str(&line.expect("no such schema")).unwrap();
        let mut scratch = Scratch::default();
        let r = model.score(&s, threshold, &mut scratch);
        result["schema"] = serde_json::json!({"id": s.id, "tables": s.tables.len(), "pairs": r.pairs.len(),
                                              "keys drawn": r.drawn.len()});
        result["scored"] = status();
    }
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}

/// The fast path against the reference path on every schema of a file:
/// pairs, features, graph features and keys drawn.
fn selfcheck(args: &[String]) {
    let model = load(&args[0]);
    let threshold: f64 = args[1].parse().unwrap();
    let mut scratch = Scratch::default();
    let (mut schemas_n, mut pairs_n, mut bad_pairs, mut bad_graph, mut bad_keys) = (0, 0, 0, 0, 0);
    for line in schemas(&args[2]) {
        let s: Schema = serde_json::from_str(&line).unwrap();
        let fast = model.score(&s, threshold, &mut scratch);
        let (reference, g_ref) = model.score_reference(&s, threshold, &mut scratch);
        schemas_n += 1;
        pairs_n += reference.pairs.len();
        if fast.pairs.len() != reference.pairs.len() {
            eprintln!("{}: {} pairs fast, {} reference", s.id, fast.pairs.len(), reference.pairs.len());
            bad_pairs += reference.pairs.len();
            continue;
        }
        for (a, b) in fast.pairs.iter().zip(&reference.pairs) {
            if (a.table, a.column, a.target) != (b.table, b.column, b.target) || a.features != b.features
                || a.base != b.base {
                bad_pairs += 1;
            }
        }
        let g_fast = model.graph_of(&s, &fast);
        bad_graph += g_fast.iter().zip(&g_ref).filter(|(a, b)| a != b).count();
        if fast.drawn != reference.drawn {
            bad_keys += 1;
            eprintln!("{}: keys differ", s.id);
        }
    }
    println!("{}", serde_json::json!({"schemas": schemas_n, "pairs": pairs_n, "pairs differing": bad_pairs,
                                     "graph rows differing": bad_graph, "schemas whose keys differ": bad_keys}));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("parity") => parity(&args[1..]),
        Some("dump") => dump(&args[1..]),
        Some("bench") => bench(&args[1..]),
        Some("memory") => memory(&args[1..]),
        Some("selfcheck") => selfcheck(&args[1..]),
        Some("profile") => {
            let model = load(&args[1]);
            let mut scratch = Scratch::default();
            for line in schemas(&args[2]) {
                let s: Schema = serde_json::from_str(&line).unwrap();
                if s.id == args[3] {
                    let p = model.profile(&s, &mut scratch);
                    println!("{}", serde_json::json!({"features": p[0], "first stage": p[1], "graph": p[2], "second stage": p[3]}));
                }
            }
        }
        _ => eprintln!("reader parity | dump | bench | memory, as the source's first comment says"),
    }
}
