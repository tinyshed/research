//! TSBS DevOps samples kept by TinyStore's metrics, VictoriaMetrics and
//! Prometheus: the bytes each keeps, how long it takes them, and TSBS' own
//! queries answered from what it kept.
//!
//! Every contender is given the same samples in the same batches: ten minutes
//! of the corpus at a time, cut into calls of at most 10,000 samples, the most
//! one TinyStore ingest takes. TinyStore takes them in process; VictoriaMetrics
//! as Prometheus' text with timestamps, one POST a call; Prometheus has no
//! write of old samples but its backfill, so promtool writes its blocks from
//! the same samples as OpenMetrics, and a Prometheus then serves them.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tinystore::metrics::{Condition, Op, Series};
use tinystore::{Options, Store};

use crate::out::{Args, Fallible, Service, disk, get, mark, ok, post, result, scraped, timed, until};

/// A TSBS corpus's ten-minute window of samples, by the order they come.
const WINDOW_MS: i64 = 10 * 60 * 1000;
/// The most samples one call carries: TinyStore's bound on one ingest.
const CALL_SAMPLES: usize = 10_000;
const DAY_MS: i64 = 86_400_000;

struct Key {
    name: String,
    labels: Vec<(String, String)>,
}

/// One call's samples, by series: what every contender is given at once.
type Call = Vec<(u32, Vec<(i64, f64)>)>;

/// Reads the corpus a window at a time and hands each window's calls on,
/// every time moved by `shift_ms`.
struct Corpus {
    keys: Vec<Key>,
    ids: HashMap<String, u32>,
    samples: u64,
}

impl Corpus {
    fn read(path: &Path, shift_ms: i64, mut each: impl FnMut(&Corpus, &Call) -> Fallible<()>) -> Fallible<Corpus> {
        let mut corpus = Corpus { keys: Vec::new(), ids: HashMap::new(), samples: 0 };
        let mut window: Vec<Vec<(i64, f64)>> = Vec::new();
        let mut window_end = i64::MIN;
        let reader = BufReader::with_capacity(1 << 20, File::open(path)?);
        for line in reader.lines() {
            let line = line?;
            let (head, rest) = line.split_once(' ').ok_or("a line without fields")?;
            let (fields, stamp) = rest.rsplit_once(' ').ok_or("a line without its time")?;
            let at = stamp.parse::<i64>()? / 1_000_000 + shift_ms;
            if at >= window_end {
                corpus.flush(&mut window, &mut each)?;
                window_end = at - at.rem_euclid(WINDOW_MS) + WINDOW_MS;
            }
            for field in fields.split(',') {
                let (field, value) = field.split_once('=').ok_or("a field without its value")?;
                let value: f64 = value.trim_end_matches(['i', 'u']).parse()?;
                let id = corpus.id(head, field);
                if window.len() <= id as usize {
                    window.resize_with(id as usize + 1, Vec::new);
                }
                window[id as usize].push((at, value));
                corpus.samples += 1;
            }
        }
        corpus.flush(&mut window, &mut each)?;
        Ok(corpus)
    }

    fn id(&mut self, head: &str, field: &str) -> u32 {
        let whole = format!("{head}|{field}");
        if let Some(id) = self.ids.get(&whole) {
            return *id;
        }
        let mut tags = head.split(',');
        let measurement = tags.next().unwrap_or_default();
        let labels = tags.filter_map(|tag| tag.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let id = self.keys.len() as u32;
        self.keys.push(Key { name: format!("{measurement}_{field}"), labels });
        self.ids.insert(whole, id);
        id
    }

    fn flush(&self, window: &mut [Vec<(i64, f64)>], each: &mut impl FnMut(&Corpus, &Call) -> Fallible<()>) -> Fallible<()> {
        let mut call: Call = Vec::new();
        let mut held = 0;
        for (id, samples) in window.iter_mut().enumerate() {
            if samples.is_empty() {
                continue;
            }
            if held + samples.len() > CALL_SAMPLES {
                each(self, &call)?;
                call.clear();
                held = 0;
            }
            held += samples.len();
            call.push((id as u32, std::mem::take(samples)));
        }
        if !call.is_empty() {
            each(self, &call)?;
        }
        Ok(())
    }
}

/// Moves the corpus by whole days to end within the day before now: its
/// times of day stay, and every contender keeps it by its default retention.
/// Gives the shift, the corpus's first time moved, and its length.
fn shift_of(path: &Path) -> Fallible<(i64, i64, i64)> {
    let length = File::open(path)?.metadata()?.len();
    let mut tail = String::new();
    let mut reader = BufReader::new(File::open(path)?);
    reader.seek(SeekFrom::Start(length.saturating_sub(8192)))?;
    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            tail = line;
        }
    }
    let last = tail.rsplit(' ').next().ok_or("no last line")?.parse::<i64>()? / 1_000_000;
    let first = {
        let mut line = String::new();
        BufReader::new(File::open(path)?).read_line(&mut line)?;
        line.trim_end().rsplit(' ').next().ok_or("no first line")?.parse::<i64>()? / 1_000_000
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
    let days = (now - 3_600_000 - last).div_euclid(DAY_MS);
    Ok((days * DAY_MS, first + days * DAY_MS, last - first + 10_000))
}

/// A query of TSBS DevOps, as each contender asks it.
struct Asked {
    name: &'static str,
    metrics: Vec<&'static str>,
    hosts: Vec<String>,
    span_ms: i64,
    every_ms: i64,
    op: &'static str,
    by_host: bool,
}

const CPU: [&str; 10] = [
    "cpu_usage_user", "cpu_usage_system", "cpu_usage_idle", "cpu_usage_nice", "cpu_usage_iowait",
    "cpu_usage_irq", "cpu_usage_softirq", "cpu_usage_steal", "cpu_usage_guest", "cpu_usage_guest_nice",
];

/// TSBS' DevOps queries, their spans cut to what the corpus holds.
fn queries(hosts: usize, length_ms: i64) -> Vec<Asked> {
    let host = |n: usize| (0..n.min(hosts)).map(|i| format!("host_{i}")).collect::<Vec<_>>();
    let hour = 3_600_000;
    let span = |want: i64| want.min(length_ms - length_ms % 60_000);
    vec![
        Asked { name: "raw-1-host-all", metrics: vec![CPU[0]], hosts: host(1), span_ms: span(i64::MAX), every_ms: 0, op: "raw", by_host: false },
        Asked { name: "single-groupby-1-1-1", metrics: vec![CPU[0]], hosts: host(1), span_ms: span(hour), every_ms: 60_000, op: "max", by_host: false },
        Asked { name: "single-groupby-1-8-1", metrics: vec![CPU[0]], hosts: host(8), span_ms: span(hour), every_ms: 60_000, op: "max", by_host: false },
        Asked { name: "single-groupby-5-1-12", metrics: CPU[..5].to_vec(), hosts: host(1), span_ms: span(12 * hour), every_ms: 60_000, op: "max", by_host: false },
        Asked { name: "cpu-max-all-8", metrics: CPU.to_vec(), hosts: host(8), span_ms: span(8 * hour), every_ms: hour, op: "max", by_host: false },
        Asked { name: "double-groupby-1", metrics: vec![CPU[0]], hosts: Vec::new(), span_ms: span(12 * hour), every_ms: hour, op: "avg", by_host: true },
    ]
}

pub fn run(args: &Args) -> Fallible<()> {
    let corpus = PathBuf::from(args.get("corpus")?);
    let dir = PathBuf::from(args.get("dir")?);
    std::fs::create_dir_all(&dir)?;
    // every query ends one sample past the corpus's last time, moved
    let (shift, first, length) = shift_of(&corpus)?;
    let end = first + length;
    let repeat: usize = args.or("repeat", "10").parse()?;
    match args.get("contender")?.as_str() {
        "tinystore" => tinystore(&corpus, &dir, shift, end, length, repeat),
        "victoria" => victoria(&corpus, &dir, shift, end, length, repeat),
        "prometheus" => prometheus(&corpus, &dir, shift, end, length, repeat),
        other => Err(format!("no contender {other}").into()),
    }
}

fn ingested(began: Instant, inside: Duration, corpus: &Corpus, calls: u64) {
    let seconds = began.elapsed().as_secs_f64();
    result(json!({
        "ingest": { "samples": corpus.samples, "series": corpus.keys.len(), "calls": calls, "seconds": seconds,
                    "insideSeconds": inside.as_secs_f64(), "perSecond": corpus.samples as f64 / seconds },
    }));
}

fn tinystore(path: &Path, dir: &Path, shift: i64, end: i64, length: i64, repeat: usize) -> Fallible<()> {
    let store = Store::open(dir.join("store"), Options::default())?;
    let metrics = store.metrics()?;
    mark("ingest");
    let began = Instant::now();
    let mut inside = Duration::ZERO;
    let mut calls = 0;
    let corpus = Corpus::read(path, shift, |corpus, call| {
        let series: Vec<Series> = call
            .iter()
            .map(|(id, samples)| {
                let key = &corpus.keys[*id as usize];
                let mut series = Series::gauge(key.name.clone());
                for (name, value) in &key.labels {
                    series = series.label(name.clone(), value);
                }
                for (at, value) in samples {
                    series = series.sample(UNIX_EPOCH + Duration::from_millis(*at as u64), *value);
                }
                series
            })
            .collect();
        let before = Instant::now();
        metrics.ingest(&series)?;
        inside += before.elapsed();
        calls += 1;
        Ok(())
    })?;
    ingested(began, inside, &corpus, calls);
    mark("settle");
    let began = Instant::now();
    let mut passes = Vec::new();
    loop {
        let done = tinystore::metrics::maintain(&store)?;
        passes.push(json!({ "packed": done.packed, "blocks": done.blocks, "expired": done.expired }));
        if done.packed == 0 && done.blocks == 0 {
            break;
        }
    }
    result(json!({ "settle": { "seconds": began.elapsed().as_secs_f64(), "passes": passes } }));
    mark("query");
    let end = UNIX_EPOCH + Duration::from_millis(end as u64);
    for asked in queries(hosts(&corpus), length) {
        let from = end - Duration::from_millis(asked.span_ms as u64);
        timed(asked.name, repeat, || {
            let mut points = 0u64;
            for name in &asked.metrics {
                let mut query = metrics.query().name(*name).from(from).to(end);
                if !asked.hosts.is_empty() {
                    query = query.filter("hostname", Condition::OneOf(asked.hosts.clone()));
                }
                if asked.op == "raw" {
                    points += query.read()?.iter().map(|series| series.samples().len() as u64).sum::<u64>();
                    continue;
                }
                query = query.every(Duration::from_millis(asked.every_ms as u64));
                if asked.by_host {
                    query = query.by(&["hostname"]);
                }
                let op = if asked.op == "max" { Op::Max } else { Op::Avg };
                points += query.aggregate(op)?.iter().map(|group| group.buckets.len() as u64).sum::<u64>();
            }
            Ok(points)
        })?;
    }
    mark("closing");
    drop(metrics);
    store.close()?;
    result(json!({ "disk": { "closed": disk(&dir.join("store")) } }));
    Ok(())
}

fn hosts(corpus: &Corpus) -> usize {
    corpus.keys.iter().filter(|key| key.name == CPU[0]).count()
}

/// Prometheus' text of a series' key, as an exposition writes it.
fn selector(key: &Key) -> String {
    let labels: Vec<String> = key.labels.iter().map(|(k, v)| format!("{k}=\"{v}\"")).collect();
    format!("{}{{{}}}", key.name, labels.join(","))
}

fn victoria(path: &Path, dir: &Path, shift: i64, end: i64, length: i64, repeat: usize) -> Fallible<()> {
    let data = dir.join("victoria");
    // the result cache is off, so that a repeated query is answered from the
    // samples as the others answer it, not from the answer before it
    let (service, ready) = Service::start(
        "victoria-metrics",
        Command::new("victoria-metrics")
            .arg(format!("-storageDataPath={}", data.display()))
            .args(["-httpListenAddr=127.0.0.1:8428", "-search.disableCache", "-loggerLevel=ERROR"]),
        "http://127.0.0.1:8428/health",
        &dir.join("victoria.log"),
    )?;
    result(json!({ "ready": ready }));
    mark("ingest");
    let began = Instant::now();
    let mut inside = Duration::ZERO;
    let mut calls = 0;
    let mut text = String::new();
    let corpus = Corpus::read(path, shift, |corpus, call| {
        text.clear();
        for (id, samples) in call {
            let key = selector(&corpus.keys[*id as usize]);
            for (at, value) in samples {
                text.push_str(&key);
                text.push(' ');
                text.push_str(&value.to_string());
                text.push(' ');
                text.push_str(&at.to_string());
                text.push('\n');
            }
        }
        let before = Instant::now();
        ok("victoria import", post("http://127.0.0.1:8428/api/v1/import/prometheus", "text/plain", text.as_bytes()))?;
        inside += before.elapsed();
        calls += 1;
        Ok(())
    })?;
    ingested(began, inside, &corpus, calls);
    mark("settle");
    let began = Instant::now();
    ok("victoria flush", get("http://127.0.0.1:8428/internal/force_flush"))?;
    let flushed = disk(&data);
    let months = months(end - length, end);
    for month in &months {
        ok("victoria merge", get(&format!("http://127.0.0.1:8428/internal/force_merge?partition_prefix={month}")))?;
    }
    let mut quiet = 0;
    let merged = until(Duration::from_secs(600), || {
        let page = get("http://127.0.0.1:8428/metrics").map(|(_, body)| String::from_utf8_lossy(&body).into_owned());
        let active = page.ok().and_then(|page| scraped(&page, "vm_active_merges")).unwrap_or(1.0);
        quiet = if active == 0.0 { quiet + 1 } else { 0 };
        quiet >= 6
    });
    result(json!({ "settle": { "seconds": began.elapsed().as_secs_f64(), "merged": merged, "months": months } }));
    let after_merge = disk(&data);
    mark("query");
    for asked in queries(hosts(&corpus), length) {
        promql(&asked, "http://127.0.0.1:8428", end, repeat)?;
    }
    mark("closing");
    service.stop()?;
    result(json!({ "disk": { "flushed": flushed, "merged": after_merge, "closed": disk(&data) } }));
    Ok(())
}

/// VictoriaMetrics' monthly partitions the range touches, `2026_10`.
fn months(from_ms: i64, to_ms: i64) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = from_ms;
    while at <= to_ms + DAY_MS {
        let days = at.div_euclid(DAY_MS);
        let (year, month, _) = civil(days);
        let name = format!("{year}_{month:02}");
        if !out.contains(&name) {
            out.push(name);
        }
        at += DAY_MS;
    }
    out
}

/// Howard Hinnant's days-to-civil, so that no date crate is needed.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// A query in PromQL over HTTP, as Grafana asks it, timed to its whole body;
/// the points are counted after the timing, from the last answer.
fn promql(asked: &Asked, base: &str, end_ms: i64, repeat: usize) -> Fallible<()> {
    let selector = |name: &str| {
        if asked.hosts.is_empty() {
            name.to_string()
        } else {
            format!("{name}{{hostname=~\"{}\"}}", asked.hosts.join("|"))
        }
    };
    let urls: Vec<String> = asked
        .metrics
        .iter()
        .map(|name| {
            let end = end_ms as f64 / 1000.0;
            if asked.op == "raw" {
                let range = asked.span_ms / 1000;
                return format!("{base}/api/v1/query?query={}&time={end}", encode(&format!("{}[{range}s]", selector(name))));
            }
            let every = asked.every_ms / 1000;
            let inner = format!("{}_over_time({}[{every}s])", asked.op, selector(name));
            let query = if asked.by_host { format!("{} by (hostname) ({inner})", asked.op) } else { format!("{}({inner})", asked.op) };
            let start = (end_ms - asked.span_ms + asked.every_ms) as f64 / 1000.0;
            format!("{base}/api/v1/query_range?query={}&start={start}&end={end}&step={every}", encode(&query))
        })
        .collect();
    let mut last = Vec::new();
    timed(asked.name, repeat, || {
        last.clear();
        for url in &urls {
            last.push(ok(asked.name, get(url))?);
        }
        Ok(0)
    })?;
    let points: u64 = last.iter().map(|body| points_of(body)).sum();
    result(json!({ "points": { "query": asked.name, "points": points } }));
    Ok(())
}

fn points_of(body: &[u8]) -> u64 {
    let Ok(answer) = serde_json::from_slice::<Value>(body) else { return 0 };
    answer["data"]["result"]
        .as_array()
        .map(|series| series.iter().map(|one| one["values"].as_array().map_or(1, Vec::len) as u64).sum())
        .unwrap_or(0)
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

fn prometheus(path: &Path, dir: &Path, shift: i64, end: i64, length: i64, repeat: usize) -> Fallible<()> {
    let data = dir.join("prometheus");
    let input = dir.join("input.om");
    mark("write-openmetrics");
    let began = Instant::now();
    let mut out = BufWriter::with_capacity(1 << 20, File::create(&input)?);
    let corpus = Corpus::read(path, shift, |corpus, call| {
        for (id, samples) in call {
            let key = selector(&corpus.keys[*id as usize]);
            for (at, value) in samples {
                writeln!(out, "{key} {value} {}.{:03}", at / 1000, at % 1000)?;
            }
        }
        Ok(())
    })?;
    writeln!(out, "# EOF")?;
    out.flush()?;
    drop(out);
    result(json!({ "openmetrics": { "seconds": began.elapsed().as_secs_f64(), "bytes": std::fs::metadata(&input)?.len() } }));
    mark("ingest");
    let began = Instant::now();
    let status = Command::new("promtool")
        .args(["tsdb", "create-blocks-from", "openmetrics"])
        .arg(&input)
        .arg(&data)
        .stdout(File::create(dir.join("promtool.log"))?)
        .status()?;
    if !status.success() {
        return Err(format!("promtool exited with {status}").into());
    }
    ingested(began, began.elapsed(), &corpus, 1);
    std::fs::remove_file(&input)?;
    let backfilled = disk(&data);
    mark("settle");
    let began = Instant::now();
    let (service, ready) = Service::start(
        "prometheus",
        Command::new("prometheus")
            .arg(format!("--config.file={}", write_config(dir)?.display()))
            .arg(format!("--storage.tsdb.path={}", data.display()))
            .args(["--web.listen-address=127.0.0.1:9090", "--log.level=error"]),
        "http://127.0.0.1:9090/-/ready",
        &dir.join("prometheus.log"),
    )?;
    // Prometheus compacts the backfilled two-hour blocks into longer ones on
    // its own schedule, a minute apart: settled once a few minutes pass with no
    // compaction, or ten at most.
    let mut seen = -1.0;
    let mut quiet = 0;
    until(Duration::from_secs(600), || {
        let page = get("http://127.0.0.1:9090/metrics").map(|(_, body)| String::from_utf8_lossy(&body).into_owned());
        let done = page.ok().and_then(|page| scraped(&page, "prometheus_tsdb_compactions_total")).unwrap_or(-1.0);
        quiet = if done == seen { quiet + 1 } else { 0 };
        seen = done;
        quiet >= 300
    });
    result(json!({ "settle": { "seconds": began.elapsed().as_secs_f64(), "ready": ready, "compactions": seen } }));
    let compacted = disk(&data);
    mark("query");
    for asked in queries(hosts(&corpus), length) {
        promql(&asked, "http://127.0.0.1:9090", end, repeat)?;
    }
    mark("closing");
    service.stop()?;
    result(json!({ "disk": { "backfilled": backfilled, "compacted": compacted, "closed": disk(&data) } }));
    Ok(())
}

fn write_config(dir: &Path) -> Fallible<PathBuf> {
    let path = dir.join("prometheus.yml");
    std::fs::write(&path, "global:\n  scrape_interval: 15s\n")?;
    Ok(path)
}
