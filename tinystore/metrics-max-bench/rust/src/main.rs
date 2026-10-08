mod adapter;
mod adapter_probe;
mod codec;
mod engine;
mod exact;
mod profile;
mod query;
mod tuning;

use engine::{Batch, Engine, Maintenance, Options, Result, Sample, Series};
use query::{AggregateRequest, AggregateResult, Condition, Limits, Query, ReadResult};
use serde_json::json;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

const EPOCH: i64 = 1700000000000;
const NORMAL_NOW: i64 = EPOCH + 6000;
const FNV_OFFSET: u64 = 14695981039346656037;

#[derive(Default)]
struct Output {
    reads: Vec<ReadResult>,
    aggregates: Vec<AggregateResult>,
    maintenance: Maintenance,
    stream_samples: usize,
}
struct Arguments {
    database: PathBuf,
    mode: String,
    case: String,
    iterations: u64,
    warm: u64,
    now: i64,
    threads: usize,
    force_parallel: bool,
    fast_exact: bool,
    fast_codec: bool,
    tuning: usize,
    adapter: usize,
}
impl Arguments {
    fn parse() -> Result<Self> {
        let mut out = Self {
            database: PathBuf::new(),
            mode: "verify".into(),
            case: "read_sealed_full8".into(),
            iterations: 16,
            warm: 32,
            now: NORMAL_NOW,
            threads: 0,
            force_parallel: false,
            fast_exact: false,
            fast_codec: false,
            tuning: 0,
            adapter: 0,
        };
        let mut args = std::env::args().skip(1);
        while let Some(k) = args.next() {
            let v = args.next().ok_or("missing argument value")?;
            match k.as_str() {
                "--db" => out.database = PathBuf::from(v),
                "--mode" => out.mode = v,
                "--case" => out.case = v,
                "--iterations" => out.iterations = v.parse()?,
                "--warm" => out.warm = v.parse()?,
                "--now" => out.now = v.parse()?,
                "--threads" => out.threads = v.parse()?,
                "--force-parallel" => out.force_parallel = v == "1",
                "--fast-exact" => out.fast_exact = v == "1",
                "--fast-codec" => out.fast_codec = v == "1",
                "--tuning" => out.tuning = v.parse()?,
                "--adapter" => out.adapter = v.parse()?,
                _ => return Err(format!("unknown flag {k}").into()),
            }
        }
        if out.database.as_os_str().is_empty() || out.iterations == 0 {
            return Err("--db and positive iterations required".into());
        }
        if out
            .database
            .file_name()
            .is_none_or(|name| name != "metrics.db")
        {
            return Err("--db must name metrics.db: the public engine owns that filename".into());
        }
        if out.case == "maintain_ready" || out.case == "expire_all" {
            if out.iterations != 1 {
                return Err("one-shot maintenance requires iterations=1".into());
            }
            out.warm = 0;
        }
        Ok(out)
    }
}
fn options() -> Options {
    Options {
        max_head_samples: 1 << 20,
        max_head_bytes: 16 << 20,
        max_batch_samples: 100000,
        max_batch_bytes: 64 << 20,
        max_readers: 1,
        max_concurrent_reads: 1,
        maintenance_series: 64,
        limits: Limits::defaults(),
        ..Options::default()
    }
}
fn series(name: &str, id: usize, kind: &str) -> Series {
    Series {
        name: name.into(),
        kind: kind.into(),
        labels: BTreeMap::from([
            ("group".into(), "bench".into()),
            ("id".into(), id.to_string()),
        ]),
    }
}
fn points(id: u64, n: usize) -> Vec<Sample> {
    (0..n)
        .map(|i| Sample {
            at: EPOCH + i as i64,
            value: ((i as u64 * 17 + id) % 997) as f64 / 10.0,
        })
        .collect()
}
fn read_range(name: &str) -> Query {
    Query {
        name: name.into(),
        from: EPOCH,
        to: EPOCH + 4801,
        ..Query::default()
    }
}
fn query_for(name: &str) -> Query {
    let mut q = read_range("bench_gauge");
    match name {
        "read_wide64" | "stream_wide64" => {
            q = read_range("wide");
            q.to = EPOCH + 1441;
        }
        "read_head_point" => {
            q = read_range("head");
            q.match_labels.insert("id".into(), "0".into());
            q.from = EPOCH + 1000;
            q.to = EPOCH + 1001;
        }
        "read_head_full8" => {
            q = read_range("head");
            q.to = EPOCH + 2001;
        }
        "read_sealed_point" => {
            q.match_labels.insert("id".into(), "3".into());
            q.from = EPOCH + 1000;
            q.to = EPOCH + 1001;
        }
        "read_sealed_boundary" => q.from = EPOCH + 4797,
        "read_filtered_one" => {
            q.match_labels.insert("id".into(), "3".into());
        }
        "read_sealed_full16" => {
            q.name.clear();
            q.match_labels.insert("group".into(), "bench".into());
        }
        "read_edge" => {
            q.name.clear();
            q.match_labels.insert("group".into(), "edge".into());
            q.to = EPOCH + 5000;
        }
        "read_where" => {
            q.conditions.insert(
                "id".into(),
                Condition::OneOf(vec!["1".into(), "3".into(), "5".into()]),
            );
        }
        "read_prefix" => {
            q.conditions
                .insert("id".into(), Condition::Prefix("1".into()));
        }
        "read_noneof" => {
            q.conditions
                .insert("id".into(), Condition::NoneOf(vec!["0".into(), "1".into()]));
        }
        "read_since" => {
            q = Query {
                name: "bench_gauge".into(),
                since: 2000,
                ..Query::default()
            }
        }
        _ => {}
    }
    q
}
fn aggregate_for(name: &str) -> AggregateRequest {
    let op = name.trim_start_matches("aggregate_");
    let mut r = AggregateRequest {
        query: read_range("bench_gauge"),
        width: 5000,
        op: op.into(),
        by: None,
        without: None,
    };
    if op.starts_with("edge_") {
        let parts: Vec<_> = op.split('_').collect();
        assert_eq!(parts.len(), 3);
        r.op = parts[1].into();
        r.query = Query {
            name: if parts[2] == "10" {
                "edge_counter".into()
            } else {
                "edge".into()
            },
            match_labels: BTreeMap::from([("id".into(), parts[2].into())]),
            from: EPOCH,
            to: EPOCH + 5000,
            ..Query::default()
        };
        return r;
    }
    let op = if let Some(wide_op) = op.strip_prefix("wide_") {
        r.query = query_for("read_wide64");
        let inner = if let Some(grouped_op) = wide_op.strip_prefix("grouped_") {
            r.by = Some(vec!["group".into()]);
            grouped_op
        } else {
            wide_op
        };
        r.op = inner.into();
        inner
    } else {
        op
    };
    match op {
        "increase" | "rate" => r.query.name = "bench_counter".into(),
        "cut_sum" | "cut_avg" | "cut_count" => {
            r.query.from += 1;
            r.width = 481;
            r.op = op.trim_start_matches("cut_").into();
        }
        "grouped_sum" => {
            r.op = "sum".into();
            r.by = Some(vec!["group".into()]);
        }
        "grouped_increase" => {
            r.op = "increase".into();
            r.query.name = "bench_counter".into();
            r.by = Some(vec!["group".into()]);
        }
        _ => {}
    }
    r
}
fn operation(
    engine: &Engine,
    name: &str,
    index: u64,
    verify: bool,
    last_stream: &mut Option<ReadResult>,
) -> Result<Output> {
    let mut out = Output::default();
    if name.starts_with("read_") {
        out.reads = engine.read(&query_for(name))?;
        return Ok(out);
    }
    if name.starts_with("aggregate_") {
        out.aggregates = engine.aggregate(&aggregate_for(name))?;
        return Ok(out);
    }
    if name == "stream_sealed_full8" || name == "stream_wide64" {
        engine.stream(&query_for(name), |r| {
            out.stream_samples += r.samples.len();
            if verify {
                out.reads.push(r);
            } else {
                *last_stream = Some(r);
            }
            Ok(())
        })?;
        return Ok(out);
    }
    if name == "maintain_ready" || name == "expire_all" {
        out.maintenance = engine.maintain()?;
        return Ok(out);
    }
    let mut batches = Vec::new();
    match name {
        "ingest_append1" => batches.push(Batch {
            series: series("head", 0, "gauge"),
            samples: vec![Sample {
                at: EPOCH + 2001 + index as i64,
                value: (index * 17 % 997) as f64 / 10.0,
            }],
        }),
        "ingest_replace1" => batches.push(Batch {
            series: series("head", 0, "gauge"),
            samples: vec![Sample {
                at: EPOCH + 1000,
                value: (index + 1) as f64 / 10.0,
            }],
        }),
        "ingest_scrape100" => {
            for id in 0..100 {
                batches.push(Batch {
                    series: series("scrape", id, "gauge"),
                    samples: vec![Sample {
                        at: EPOCH + 240 + index as i64,
                        value: ((index * 17 + id as u64) % 997) as f64 / 10.0,
                    }],
                });
            }
        }
        "ingest_register8" => {
            for j in 0..8 {
                let q = index * 8 + j;
                let mut s = series("register", 0, "gauge");
                s.labels.insert("id".into(), format!("{q:06}"));
                batches.push(Batch {
                    series: s,
                    samples: points(q, 240),
                });
            }
        }
        "ingest_shuffled240" | "ingest_duplicates240" => {
            let samples = (0..240)
                .map(|j| {
                    let k = if name == "ingest_duplicates240" {
                        j % 120
                    } else {
                        j * 53 % 240
                    };
                    Sample {
                        at: EPOCH + 1000 + k as i64,
                        value: ((index * 13 + j as u64 * 17) % 997) as f64 / 10.0,
                    }
                })
                .collect();
            batches.push(Batch {
                series: series("head", 0, "gauge"),
                samples,
            });
        }
        _ => return Err(format!("unknown case {name}").into()),
    }
    engine.ingest(&batches)?;
    Ok(out)
}
fn hash_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    h
}
fn hash_word(h: u64, n: u64) -> u64 {
    hash_bytes(h, &n.to_le_bytes())
}
fn hash_string(h: u64, s: &str) -> u64 {
    hash_bytes(hash_word(h, s.len() as u64), s.as_bytes())
}
fn hash_series(mut h: u64, s: &Series) -> u64 {
    h = hash_string(h, &s.name);
    h = hash_string(h, &s.kind);
    h = hash_word(h, s.labels.len() as u64);
    for (k, v) in &s.labels {
        h = hash_string(h, k);
        h = hash_string(h, v);
    }
    h
}
fn hash_reads(mut h: u64, reads: &[ReadResult]) -> u64 {
    h = hash_word(h, reads.len() as u64);
    for r in reads {
        h = hash_series(h, &r.series);
        h = hash_word(h, r.samples.len() as u64);
        for p in &r.samples {
            h = hash_word(h, p.at as u64);
            h = hash_word(h, p.value.to_bits());
        }
    }
    h
}
fn hash_output(mut h: u64, out: &Output) -> u64 {
    h = hash_reads(h, &out.reads);
    h = hash_word(h, out.aggregates.len() as u64);
    for r in &out.aggregates {
        h = hash_series(h, &r.series);
        h = hash_word(h, r.buckets.len() as u64);
        for b in &r.buckets {
            for n in [
                b.from as u64,
                b.to as u64,
                b.count as u64,
                b.resets as u64,
                b.value.to_bits(),
                b.overflow as u64,
                b.partial as u64,
            ] {
                h = hash_word(h, n);
            }
        }
    }
    let m = &out.maintenance;
    for n in [
        m.sealed_blocks,
        m.expired_samples,
        m.conflicts,
        m.quarantined_series,
        m.reclaimed_series,
        out.stream_samples,
    ] {
        h = hash_word(h, n as u64);
    }
    h
}
fn digest(engine: &Engine) -> Result<u64> {
    let q = Query {
        match_labels: BTreeMap::from([("group".into(), "bench".into())]),
        from: EPOCH,
        to: EPOCH + 1000000,
        ..Query::default()
    };
    Ok(hash_reads(FNV_OFFSET, &engine.read(&q)?))
}
fn rss_kib() -> Result<u64> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    status
        .lines()
        .find_map(|s| s.strip_prefix("VmRSS:"))
        .and_then(|s| s.split_whitespace().next())
        .ok_or("VmRSS absent")?
        .parse()
        .map_err(Into::into)
}
fn error_contains<T>(r: Result<T>, token: &str) -> bool {
    r.err()
        .is_some_and(|e| e.to_string().to_lowercase().contains(token))
}
fn guards(engine: &Engine) -> Result<serde_json::Value> {
    let before = digest(engine)?;
    let s = series("bench_gauge", 0, "gauge");
    let old = error_contains(
        engine.ingest(&[Batch {
            series: s.clone(),
            samples: vec![Sample {
                at: EPOCH + 10,
                value: 1.0,
            }],
        }]),
        "tooold",
    );
    let new = error_contains(
        engine.ingest(&[Batch {
            series: s.clone(),
            samples: vec![Sample {
                at: NORMAL_NOW + 600001,
                value: 1.0,
            }],
        }]),
        "toonew",
    );
    let rollback = error_contains(
        engine.ingest(&[
            Batch {
                series: s,
                samples: vec![Sample {
                    at: EPOCH + 4801,
                    value: 123.0,
                }],
            },
            Batch {
                series: series("bench_gauge", 1, "gauge"),
                samples: vec![Sample {
                    at: EPOCH + 10,
                    value: 456.0,
                }],
            },
        ]),
        "tooold",
    ) && digest(engine)? == before;
    let mut q = query_for("read_sealed_point");
    q.limits = Limits::zero();
    q.limits.decoded_samples = 1;
    let decode = error_contains(engine.read(&q), "limit");
    q = read_range("bench_gauge");
    q.limits = Limits::zero();
    q.limits.output_samples = 1;
    let output = error_contains(engine.read(&q), "limit");
    q.limits = Limits::zero();
    q.limits.series = 1;
    let series = error_contains(engine.read(&q), "limit");
    Ok(
        json!({"too_old":old,"too_new":new,"atomic_rollback":rollback,"decode_limit":decode,"output_limit":output,"series_limit":series,"final_digest":format!("{:016x}",digest(engine)?)}),
    )
}
fn inspection(engine: &Engine) -> Result<serde_json::Value> {
    let mut out = serde_json::Map::new();
    for (role, conn) in [("reader", &engine.reader), ("writer", &engine.writer)] {
        let mut info = serde_json::Map::new();
        for name in [
            "page_size",
            "synchronous",
            "fullfsync",
            "checkpoint_fullfsync",
            "foreign_keys",
            "busy_timeout",
            "cache_size",
            "trusted_schema",
            "query_only",
            "wal_autocheckpoint",
            "mmap_size",
        ] {
            let n: i64 = conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?;
            info.insert(name.into(), json!(n));
        }
        let (version, source): (String, String) =
            conn.query_row("SELECT sqlite_version(),sqlite_source_id()", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        let journal: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        info.insert("version".into(), json!(version));
        info.insert("source_id".into(), json!(source));
        info.insert("journal_mode".into(), json!(journal));
        out.insert(role.into(), serde_json::Value::Object(info));
    }
    let mut stmt = engine.reader.prepare("PRAGMA compile_options")?;
    let opts = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    out.insert("compile_options".into(), json!(opts));
    Ok(serde_json::Value::Object(out))
}
fn main() -> Result<()> {
    let args = Arguments::parse()?;
    query::configure_parallel(args.threads, args.force_parallel)?;
    exact::configure_fast(args.fast_exact);
    codec::set_optimized(args.fast_codec);
    tuning::configure(args.tuning);
    adapter::configure(args.adapter);
    let engine = Engine::open(&args.database, options(), args.now)?;
    let mut last_stream = None;
    let result = match args.mode.as_str() {
        "wrapper" => adapter_probe::run(&engine.reader, &args.case, args.iterations, args.warm)?,
        "digest" => json!({"final_digest":format!("{:016x}",digest(&engine)?)}),
        "guards" => guards(&engine)?,
        "edge-guards" => {
            json!({"nonfinite":error_contains(engine.aggregate(&aggregate_for("aggregate_edge_sum_7")),"nonfinite")})
        }
        "inspect" => inspection(&engine)?,
        "kernel-residuals" => codec::residuals::kernel_bench(args.iterations, args.warm)?,
        "kernel-huffman" => codec::huffman::kernel_bench(args.iterations, args.warm)?,
        "plan" => {
            let (q, width) = if args.case.starts_with("aggregate_") {
                let r = aggregate_for(&args.case);
                (r.query, Some(r.width))
            } else {
                (query_for(&args.case), None)
            };
            let p = engine.query_spending(&q, width)?;
            json!({"series":p.series,"blocks":p.blocks,"summarized":p.summarized,"bytes":p.bytes,"decoded":p.decoded,"stops":null})
        }
        "verify" => {
            let (mut h, mut samples, mut buckets) = (FNV_OFFSET, 0usize, 0usize);
            for i in 0..args.iterations {
                let out = operation(&engine, &args.case, i, true, &mut last_stream)?;
                h = hash_output(h, &out);
                samples += out.reads.iter().map(|r| r.samples.len()).sum::<usize>();
                buckets += out
                    .aggregates
                    .iter()
                    .map(|r| r.buckets.len())
                    .sum::<usize>();
            }
            json!({"implementation":"rust_metrics","case":args.case,"iterations":args.iterations,"checksum":format!("{h:016x}"),"samples":samples,"buckets":buckets,"final_digest":format!("{:016x}",digest(&engine)?)})
        }
        "bench" => {
            for i in 0..args.warm {
                black_box(operation(&engine, &args.case, i, false, &mut last_stream)?);
            }
            let start = Instant::now();
            for i in 0..args.iterations {
                black_box(operation(
                    &engine,
                    &args.case,
                    i + args.warm,
                    false,
                    &mut last_stream,
                )?);
            }
            json!({"implementation":"rust_metrics","case":args.case,"iterations":args.iterations,"warm":args.warm,"ns_per_op":start.elapsed().as_nanos()as f64/args.iterations as f64})
        }
        "profile" => {
            if !cfg!(feature = "telemetry") || args.threads != 0 {
                return Err("profile requires the telemetry build and threads=0".into());
            }
            for i in 0..args.warm {
                black_box(operation(&engine, &args.case, i, false, &mut last_stream)?);
            }
            profile::reset();
            adapter::counters(&engine.reader, true)?;
            adapter::counters(&engine.writer, true)?;
            profile::configure(true);
            for i in 0..args.iterations {
                let _scope = profile::scope("operation");
                black_box(operation(
                    &engine,
                    &args.case,
                    i + args.warm,
                    false,
                    &mut last_stream,
                )?);
            }
            let mut out = profile::finish();
            out["case"] = json!(args.case);
            out["iterations"] = json!(args.iterations);
            out["sqlite_reader"] = adapter::counters(&engine.reader, false)?;
            out["sqlite_writer"] = adapter::counters(&engine.writer, false)?;
            out["sqlite_memory_used"] = json!(unsafe { rusqlite::ffi::sqlite3_memory_used() });
            out
        }
        "memory" => {
            let mut held = Vec::new();
            for i in 0..args.iterations {
                held.push(operation(&engine, &args.case, i, false, &mut last_stream)?);
            }
            let samples: usize = held
                .iter()
                .flat_map(|o| &o.reads)
                .map(|r| r.samples.len())
                .sum();
            let result = json!({"implementation":"rust_metrics","case":args.case,"iterations":args.iterations,"rss_kib":rss_kib()?,"retained_samples":samples,"sqlite_memory_used":unsafe{rusqlite::ffi::sqlite3_memory_used()}});
            black_box(&held);
            black_box(&last_stream);
            result
        }
        _ => return Err(format!("unknown mode {}", args.mode).into()),
    };
    println!("{result}");
    Ok(())
}
