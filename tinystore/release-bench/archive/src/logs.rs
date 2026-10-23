//! LogHub's datasets kept by TinyStore's records and by Loki: the bytes each
//! keeps, how long it takes them, and four reads answered from what it kept.
//!
//! `prepare` reads a dataset once and writes what every contender is given:
//! each line with the time its own text spells, as a shipper parses it, the
//! lines in the order of those times. A line whose text spells none takes the
//! time of the line before it, and lines of one time take a nanosecond each
//! after it, so that Loki keeps two equal lines of one second as two, as
//! TinyStore does. It also writes the dataset's size under gzip and zstd.
//!
//! Both are given the same lines in batches of 1000: TinyStore appends each
//! as an event named `log` with the line its message, Loki takes each batch
//! as one push of one stream labelled with the dataset's name.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::json;
use tinystore::records::NewEvent;
use tinystore::{Options, Store, TestClock};

use crate::out::{Args, Fallible, Service, disk, get, mark, ok, post, result, scraped, timed, until};

const BATCH: usize = 1000;

/// How a dataset's text spells its time, and the year when it spells none.
struct Dataset {
    name: &'static str,
    pattern: &'static str,
    layout: Layout,
    year: i64,
}

#[derive(Clone, Copy)]
enum Layout {
    /// year? month day hour minute second fraction?, by the groups' names
    Named,
    /// unix seconds
    Unix,
}

const DATASETS: &[Dataset] = &[
    Dataset { name: "Apache", pattern: r"^\[\w{3} (?P<mon>\w{3}) (?P<d>\d{2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2}) (?P<y>\d{4})\]", layout: Layout::Named, year: 0 },
    // its month and day are not padded: 201811 is the first of January
    Dataset { name: "HealthApp", pattern: r"^(?P<y>\d{4})(?P<md>\d{2,4})-(?P<h>\d{1,2}):(?P<mi>\d{1,2}):(?P<s>\d{1,2}):(?P<f>\d{1,3})", layout: Layout::Named, year: 0 },
    Dataset { name: "Linux", pattern: r"^(?P<mon>\w{3}) +(?P<d>\d{1,2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})", layout: Layout::Named, year: 2005 },
    Dataset { name: "Mac", pattern: r"^(?P<mon>\w{3}) +(?P<d>\d{1,2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})", layout: Layout::Named, year: 2017 },
    Dataset { name: "OpenStack", pattern: r" (?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})\.(?P<f>\d{3}) ", layout: Layout::Named, year: 0 },
    Dataset { name: "Proxifier", pattern: r"^\[(?P<m>\d{2})\.(?P<d>\d{2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})\]", layout: Layout::Named, year: 2016 },
    Dataset { name: "SSH", pattern: r"^(?P<mon>\w{3}) +(?P<d>\d{1,2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})", layout: Layout::Named, year: 2017 },
    Dataset { name: "Zookeeper", pattern: r"^(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2}),(?P<f>\d{3})", layout: Layout::Named, year: 0 },
    Dataset { name: "HPC", pattern: r"^\S+ \S+ \S+ \S+ (?P<unix>\d{9,10}) ", layout: Layout::Unix, year: 0 },
    Dataset { name: "Hadoop", pattern: r"^(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2}),(?P<f>\d{3})", layout: Layout::Named, year: 0 },
    Dataset { name: "Android_v1", pattern: r"^(?P<m>\d{2})-(?P<d>\d{2}) (?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})\.(?P<f>\d{3})", layout: Layout::Named, year: 2017 },
    Dataset { name: "BGL", pattern: r"^\S+ \d+ \S+ \S+ (?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2})-(?P<h>\d{2})\.(?P<mi>\d{2})\.(?P<s>\d{2})\.(?P<f>\d{6})", layout: Layout::Named, year: 0 },
    Dataset { name: "HDFS_v1", pattern: r"^(?P<y>\d{2})(?P<m>\d{2})(?P<d>\d{2}) (?P<h>\d{2})(?P<mi>\d{2})(?P<s>\d{2})", layout: Layout::Named, year: 0 },
];

fn dataset(name: &str) -> Fallible<&'static Dataset> {
    Ok(DATASETS.iter().find(|d| d.name == name).ok_or_else(|| format!("no dataset {name}"))?)
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The line's time in nanoseconds, as its text spells it read as UTC.
fn time_of(set: &Dataset, pattern: &Regex, line: &str) -> Option<i64> {
    let found = pattern.captures(line)?;
    if let Layout::Unix = set.layout {
        return Some(found.name("unix")?.as_str().parse::<i64>().ok()? * 1_000_000_000);
    }
    let number = |name: &str| found.name(name).and_then(|m| m.as_str().parse::<i64>().ok());
    let year = match number("y") {
        Some(y) if y < 100 => 2000 + y,
        Some(y) => y,
        None => set.year,
    };
    let (month, day) = match (found.name("mon"), found.name("md")) {
        (Some(name), _) => (MONTHS.iter().position(|m| *m == name.as_str())? as i64 + 1, number("d")?),
        (None, Some(md)) => {
            let digits = md.as_str();
            // two digits are a month and a day; three, a month of one digit
            // unless that would be no month, then a day of one
            let split = match digits.len() {
                2 => 1,
                3 if digits.starts_with('1') && digits[1..].parse::<i64>().ok()? > 31 => 2,
                3 => 1,
                _ => 2,
            };
            (digits[..split].parse().ok()?, digits[split..].parse().ok()?)
        }
        (None, None) => (number("m")?, number("d")?),
    };
    let seconds = days_from_civil(year, month, day) * 86_400 + number("h")? * 3600 + number("mi")? * 60 + number("s")?;
    let fraction = found.name("f").map_or(0, |f| {
        let digits = f.as_str();
        digits.parse::<i64>().unwrap_or(0) * 10_i64.pow(9 - digits.len() as u32)
    });
    Some(seconds * 1_000_000_000 + fraction)
}

/// The dataset's files, in the order of their paths.
fn files(root: &Path) -> Fallible<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "log") && entry.metadata()?.len() > 1024 {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Writes `<name>.tsv`, each line its time then a tab then the line, and
/// `<name>.json`, what the dataset is and what gzip and zstd make of it.
pub fn prepare(args: &Args) -> Fallible<()> {
    let set = dataset(&args.get("dataset")?)?;
    let root = PathBuf::from(args.get("corpus")?).join(set.name);
    let out = PathBuf::from(args.get("out")?);
    std::fs::create_dir_all(&out)?;
    let pattern = Regex::new(set.pattern)?;
    let mut lines: Vec<(i64, String)> = Vec::new();
    let mut raw = Vec::new();
    let mut unparsed = 0u64;
    let mut sources = Vec::new();
    for file in files(&root)? {
        sources.push(file.display().to_string());
        let mut bytes = Vec::new();
        File::open(&file)?.read_to_end(&mut bytes)?;
        raw.extend_from_slice(&bytes);
        if !bytes.ends_with(b"\n") {
            raw.push(b'\n');
        }
        let mut last = lines.last().map_or(0, |(at, _)| *at);
        let mut year_bump = 0;
        for line in String::from_utf8_lossy(&bytes).lines() {
            if line.is_empty() {
                continue;
            }
            let at = match time_of(set, &pattern, line) {
                Some(mut at) => {
                    // a year the text leaves out turns over where the time goes back months
                    at += year_bump;
                    if set.year != 0 && at < last - 180 * 86_400 * 1_000_000_000 {
                        year_bump += 365 * 86_400 * 1_000_000_000;
                        at += 365 * 86_400 * 1_000_000_000;
                    }
                    at
                }
                None => {
                    unparsed += 1;
                    last
                }
            };
            last = at;
            lines.push((at, line.to_string()));
        }
    }
    // the lines before the first that spells a time take that line's
    if let Some(first) = lines.iter().map(|(at, _)| *at).find(|at| *at != 0) {
        for (at, _) in lines.iter_mut().take_while(|(at, _)| *at == 0) {
            *at = first;
        }
    }
    lines.sort_by_key(|(at, _)| *at);
    let mut previous = i64::MIN;
    let mut tsv = BufWriter::with_capacity(1 << 20, File::create(out.join(format!("{}.tsv", set.name)))?);
    for (at, line) in &mut lines {
        if *at <= previous {
            *at = previous + 1;
        }
        previous = *at;
        writeln!(tsv, "{at}\t{line}")?;
    }
    tsv.flush()?;
    let began = Instant::now();
    let gzip6 = {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
        encoder.write_all(&raw)?;
        encoder.finish()?.len()
    };
    let zstd3 = zstd::bulk::compress(&raw, 3)?.len();
    let zstd19 = {
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 19)?;
        encoder.multithread(8)?;
        encoder.write_all(&raw)?;
        encoder.finish()?.len()
    };
    let (first, last) = (lines.first().map_or(0, |l| l.0), lines.last().map_or(0, |l| l.0));
    let summary = json!({
        "dataset": set.name, "sources": sources, "lines": lines.len(), "bytes": raw.len(), "unparsed": unparsed,
        "first_ns": first, "last_ns": last, "gzip6": gzip6, "zstd3": zstd3, "zstd19": zstd19,
        "compressSeconds": began.elapsed().as_secs_f64(),
    });
    std::fs::write(out.join(format!("{}.json", set.name)), serde_json::to_vec_pretty(&summary)?)?;
    println!("{summary}");
    Ok(())
}

/// The prepared lines a batch at a time.
fn batches(path: &Path, mut each: impl FnMut(&[(i64, String)]) -> Fallible<()>) -> Fallible<u64> {
    let mut batch = Vec::with_capacity(BATCH);
    let mut count = 0;
    for line in BufReader::with_capacity(1 << 20, File::open(path)?).lines() {
        let line = line?;
        let (at, text) = line.split_once('\t').ok_or("a prepared line without its time")?;
        batch.push((at.parse()?, text.to_string()));
        if batch.len() == BATCH {
            each(&batch)?;
            count += batch.len() as u64;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        each(&batch)?;
        count += batch.len() as u64;
    }
    Ok(count)
}

/// What a dataset's reads ask: a token of its middle line, rare as an id is;
/// a word common to logs; its newest lines; a minute in its middle.
struct Reads {
    needle: String,
    first: i64,
    last: i64,
    middle: i64,
}

fn reads(path: &Path, summary: &serde_json::Value) -> Fallible<Reads> {
    let lines = summary["lines"].as_u64().unwrap_or(0);
    let middle_line = BufReader::new(File::open(path)?).lines().nth((lines / 2) as usize).ok_or("no middle line")??;
    let (at, text) = middle_line.split_once('\t').ok_or("a prepared line without its time")?;
    let needle = text
        .split(|c: char| c.is_whitespace() || "[](){},;\"'=:".contains(c))
        .filter(|token| token.len() >= 6 && token.chars().any(|c| c.is_ascii_digit()))
        .max_by_key(|token| token.len())
        .unwrap_or("error")
        .to_string();
    Ok(Reads {
        needle,
        first: summary["first_ns"].as_i64().unwrap_or(0),
        last: summary["last_ns"].as_i64().unwrap_or(0),
        middle: at.parse()?,
    })
}

pub fn run(args: &Args) -> Fallible<()> {
    let set = dataset(&args.get("dataset")?)?;
    let prepared = PathBuf::from(args.get("prepared")?);
    let path = prepared.join(format!("{}.tsv", set.name));
    let summary: serde_json::Value = serde_json::from_slice(&std::fs::read(prepared.join(format!("{}.json", set.name)))?)?;
    let dir = PathBuf::from(args.get("dir")?);
    std::fs::create_dir_all(&dir)?;
    let asked = reads(&path, &summary)?;
    result(json!({ "dataset": set.name, "needle": asked.needle, "lines": summary["lines"], "bytes": summary["bytes"] }));
    let repeat: usize = args.or("repeat", "5").parse()?;
    match args.get("contender")?.as_str() {
        "tinystore" => tinystore(set, &path, &dir, &asked, repeat),
        "loki" => loki(set, &path, &dir, &asked, repeat),
        other => Err(format!("no contender {other}").into()),
    }
}

fn at(ns: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_nanos(ns as u64)
}

fn tinystore(set: &Dataset, path: &Path, dir: &Path, asked: &Reads, repeat: usize) -> Fallible<()> {
    // A test clock, so that the hour a head waits before it is packed passes
    // at once after the ingest, as it would an hour later; the store's work
    // otherwise runs as it ships.
    let clock = Arc::new(TestClock::new(SystemTime::now()));
    let store = Store::open(dir.join("store"), Options { clock: Some(clock.clone()), ..Options::default() })?;
    // the datasets are from 2005 to 2017: a stream keeps them when told to
    let events = store.events(&set.name.to_lowercase()).keep(Duration::from_secs(40 * 365 * 86_400)).open()?;
    mark("ingest");
    let began = Instant::now();
    let mut inside = Duration::ZERO;
    let mut calls = 0;
    let lines = batches(path, |batch| {
        let list: Vec<NewEvent> = batch.iter().map(|(ns, line)| NewEvent::new("log").at(at(*ns)).message(line)).collect();
        let before = Instant::now();
        events.append_all(list)?;
        inside += before.elapsed();
        calls += 1;
        Ok(())
    })?;
    ingested(began, inside, lines, calls);
    let unpacked = disk(&dir.join("store"));
    mark("settle");
    let began = Instant::now();
    clock.advance(Duration::from_hours(2));
    let mut passes = Vec::new();
    loop {
        let done = tinystore::records::maintain(&store)?;
        passes.push(json!({ "packed": done.packed, "segments": done.segments, "merged": done.merged }));
        if done.packed == 0 && done.merged == 0 {
            break;
        }
    }
    result(json!({ "settle": { "seconds": began.elapsed().as_secs_f64(), "passes": passes } }));
    mark("query");
    let all = || events.list().from(at(asked.first)).to(at(asked.last + 1));
    timed("needle-count", repeat, || Ok(all().search(&asked.needle).count()?))?;
    timed("needle-lines", repeat, || Ok(all().search(&asked.needle).limit(100).all().take(100).count() as u64))?;
    timed("error-count", repeat, || Ok(all().search("error").count()?))?;
    timed("newest-100", repeat, || Ok(all().newest().limit(100).all().take(100).count() as u64))?;
    timed("minute", repeat, || {
        Ok(events.list().from(at(asked.middle)).to(at(asked.middle + 60_000_000_000)).limit(10_000).all().count() as u64)
    })?;
    mark("closing");
    drop(events);
    store.close()?;
    result(json!({ "disk": { "ingested": unpacked, "closed": disk(&dir.join("store")) } }));
    Ok(())
}

fn ingested(began: Instant, inside: Duration, lines: u64, calls: u64) {
    let seconds = began.elapsed().as_secs_f64();
    result(json!({
        "ingest": { "lines": lines, "calls": calls, "seconds": seconds, "insideSeconds": inside.as_secs_f64(),
                    "perSecond": lines as f64 / seconds },
    }));
}

/// Loki 3 as one binary on the filesystem, its getting-started config, with
/// what this corpus needs: dates back to 2005 taken and kept, a range of
/// months queried, its chunks written at shutdown, and no limit on the rate.
const LOKI: &str = "auth_enabled: false
server:
  http_listen_address: 127.0.0.1
  http_listen_port: 3100
  grpc_listen_address: 127.0.0.1
  grpc_listen_port: 9096
  log_level: warn
common:
  instance_addr: 127.0.0.1
  path_prefix: DATA
  storage:
    filesystem:
      chunks_directory: DATA/chunks
      rules_directory: DATA/rules
  replication_factor: 1
  ring:
    kvstore:
      store: inmemory
ingester:
  wal:
    flush_on_shutdown: true
schema_config:
  configs:
    - from: 2000-01-01
      store: tsdb
      object_store: filesystem
      schema: v13
      index:
        prefix: index_
        period: 24h
limits_config:
  reject_old_samples: false
  max_query_length: 0
  ingestion_rate_mb: 4096
  ingestion_burst_size_mb: 4096
  per_stream_rate_limit: 4096MB
  per_stream_rate_limit_burst: 4096MB
  max_entries_limit_per_query: 100000
analytics:
  reporting_enabled: false
";

fn loki(set: &Dataset, path: &Path, dir: &Path, asked: &Reads, repeat: usize) -> Fallible<()> {
    let data = dir.join("loki");
    std::fs::create_dir_all(&data)?;
    let config = dir.join("loki.yml");
    std::fs::write(&config, LOKI.replace("DATA", &data.display().to_string()))?;
    let (service, ready) = Service::start(
        "loki",
        Command::new("loki").arg(format!("-config.file={}", config.display())),
        "http://127.0.0.1:3100/ready",
        &dir.join("loki.log"),
    )?;
    result(json!({ "ready": ready }));
    mark("ingest");
    let began = Instant::now();
    let mut inside = Duration::ZERO;
    let mut calls = 0;
    let stream = json!({ "dataset": set.name });
    let lines = batches(path, |batch| {
        let values: Vec<[String; 2]> = batch.iter().map(|(ns, line)| [ns.to_string(), line.clone()]).collect();
        let body = serde_json::to_vec(&json!({ "streams": [{ "stream": stream, "values": values }] }))?;
        let before = Instant::now();
        ok("loki push", post("http://127.0.0.1:3100/loki/api/v1/push", "application/json", &body))?;
        inside += before.elapsed();
        calls += 1;
        Ok(())
    })?;
    ingested(began, inside, lines, calls);
    mark("settle");
    let began = Instant::now();
    ok("loki flush", post("http://127.0.0.1:3100/flush", "text/plain", b""))?;
    let flushed = until(Duration::from_secs(600), || {
        let page = get("http://127.0.0.1:3100/metrics").map(|(_, body)| String::from_utf8_lossy(&body).into_owned());
        page.ok().and_then(|page| scraped(&page, "loki_ingester_memory_chunks")) == Some(0.0)
    });
    result(json!({ "settle": { "seconds": began.elapsed().as_secs_f64(), "flushed": flushed } }));
    let after_flush = disk(&data);
    mark("query");
    let selector = format!("{{dataset=\"{}\"}}", set.name);
    let span = (asked.last - asked.first) / 1_000_000_000 + 2;
    let end = asked.last + 1_000_000_000;
    let instant = |query: String| format!("http://127.0.0.1:3100/loki/api/v1/query?query={}&time={end}", encode(&query));
    let range = |query: &str, from: i64, to: i64, limit: u32, direction: &str| {
        format!(
            "http://127.0.0.1:3100/loki/api/v1/query_range?query={}&start={from}&end={to}&limit={limit}&direction={direction}",
            encode(query)
        )
    };
    let needle = format!("{selector} |= \"{}\"", asked.needle.replace('\\', "\\\\").replace('"', "\\\""));
    loki_timed("needle-count", repeat, &instant(format!("sum(count_over_time({needle} [{span}s]))")))?;
    loki_timed("needle-lines", repeat, &range(&needle, asked.first, end, 100, "forward"))?;
    loki_timed("error-count", repeat, &instant(format!("sum(count_over_time({selector} |~ \"(?i)error\" [{span}s]))")))?;
    loki_timed("newest-100", repeat, &range(&selector, asked.first, end, 100, "backward"))?;
    loki_timed("minute", repeat, &range(&selector, asked.middle, asked.middle + 60_000_000_000, 10_000, "forward"))?;
    mark("closing");
    service.stop()?;
    result(json!({ "disk": { "flushed": after_flush, "closed": disk(&data) } }));
    Ok(())
}

fn loki_timed(name: &str, repeat: usize, url: &str) -> Fallible<()> {
    let mut last = Vec::new();
    timed(name, repeat, || {
        last = ok(name, get(url))?;
        Ok(0)
    })?;
    let answer: serde_json::Value = serde_json::from_slice(&last)?;
    let found = &answer["data"]["result"];
    let size: u64 = if answer["data"]["resultType"] == "streams" {
        found.as_array().map_or(0, |streams| streams.iter().map(|s| s["values"].as_array().map_or(0, Vec::len) as u64).sum())
    } else {
        found.as_array().and_then(|v| v.first()).and_then(|v| v["value"][1].as_str()).and_then(|v| v.parse().ok()).unwrap_or(0)
    };
    result(json!({ "points": { "query": name, "points": size } }));
    Ok(())
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
