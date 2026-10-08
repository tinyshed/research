mod bits;
mod codec;
mod engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{hint::black_box, path::Path, time::Instant};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const EPOCH: i64 = 1700000000000000000;
const NOW: i64 = EPOCH + 3600000000000;
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub value: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub at: i64,
    pub stream: String,
    pub name: String,
    pub level: Option<i32>,
    pub body: Option<String>,
    pub trace: [u8; 16],
    pub span: [u8; 8],
    pub context: Vec<Field>,
    pub attrs: Vec<Field>,
}
pub enum Payload {
    Bytes(Vec<u8>),
    Words(Vec<u64>),
}
pub struct Case {
    pub name: String,
    pub units: usize,
    pub input_bytes: usize,
    pub input_hash: u64,
    pub run: Box<dyn Fn() -> Payload>,
}
pub fn fingerprint_bytes(b: &[u8], params: &[u64]) -> u64 {
    let mut h = 14695981039346656037u64;
    for b in params
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .chain(b.iter().copied())
    {
        h = (h ^ u64::from(b)).wrapping_mul(1099511628211)
    }
    h
}
pub fn fingerprint_words(ws: &[u64], params: &[u64]) -> u64 {
    fingerprint_bytes(
        &ws.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(),
        params,
    )
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn unhex<const N: usize>(s: &str) -> Result<[u8; N]> {
    if s.len() != N * 2 {
        return Err("ID width".into());
    };
    let mut b = [0; N];
    for i in 0..N {
        b[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)?
    }
    Ok(b)
}
fn from_json(v: Value) -> Result<Record> {
    let fs = |v: &Value| -> Result<Vec<Field>> {
        v.as_array()
            .ok_or("fields")?
            .iter()
            .map(|f| {
                Ok(Field {
                    key: f["key"].as_str().ok_or("key")?.into(),
                    value: f["value"].as_str().ok_or("value")?.into(),
                })
            })
            .collect()
    };
    Ok(Record {
        at: v["at"].as_i64().ok_or("at")?,
        stream: v["stream"].as_str().ok_or("stream")?.into(),
        name: v["name"].as_str().ok_or("name")?.into(),
        level: v["level"].as_i64().map(|n| n as i32),
        body: v["body"].as_str().map(str::to_owned),
        trace: unhex(v["trace"].as_str().ok_or("trace")?)?,
        span: unhex(v["span"].as_str().ok_or("span")?)?,
        context: fs(&v["context"])?,
        attrs: fs(&v["attrs"])?,
    })
}
fn load(path: &str) -> Result<Vec<Record>> {
    std::fs::read_to_string(path)?
        .lines()
        .map(|s| from_json(serde_json::from_str(s)?))
        .collect()
}
fn fixture(n: usize) -> Vec<Record> {
    (0..n)
        .map(|i| {
            let mut trace = [0; 16];
            let mut span = [0; 8];
            if i % 3 == 0 {
                trace[0] = (i % 251 + 1) as u8;
                trace[15] = (i % 13 + 1) as u8;
                span[0] = (i % 251 + 1) as u8
            };
            Record {
                at: EPOCH + ((i * 997) % (1.max(n / 2))) as i64 * 1000000,
                stream: format!("stream-{}", i % 2),
                name: ["request", "click", "log"][i % 3].into(),
                level: if i % 5 == 0 {
                    None
                } else {
                    Some((i % 4) as i32 * 4 - 4)
                },
                body: if i % 7 == 0 {
                    None
                } else {
                    Some(format!(
                        "request {i} completed route=/items/{} Unicode=Привет",
                        i % 17
                    ))
                },
                trace,
                span,
                context: vec![
                    Field {
                        key: "service".into(),
                        value: r#""api""#.into(),
                    },
                    Field {
                        key: "host".into(),
                        value: format!(r#""host-{}""#, i % 4),
                    },
                ],
                attrs: vec![
                    Field {
                        key: "seq".into(),
                        value: i.to_string(),
                    },
                    Field {
                        key: "status".into(),
                        value: [200, 404, 503][i % 3].to_string(),
                    },
                    Field {
                        key: "tag".into(),
                        value: r#""first""#.into(),
                    },
                    Field {
                        key: "tag".into(),
                        value: r#""second""#.into(),
                    },
                    Field {
                        key: "json".into(),
                        value: r#"{"a":[true,null,1.25],"s":"a\\b"}"#.into(),
                    },
                ],
            }
        })
        .collect()
}
#[derive(Clone)]
pub struct Query {
    from: i64,
    to: i64,
    limit: usize,
    newest: bool,
    blocks: usize,
    bytes: usize,
    decoded: usize,
    level: Option<i32>,
    attrs: Vec<Field>,
    context: Vec<Field>,
    search: String,
    trace: Option<[u8; 16]>,
}
impl Query {
    fn for_case(c: &str) -> Self {
        let mut q = Self {
            from: EPOCH - 30000000000,
            to: EPOCH + 3600000000000,
            limit: 10000,
            newest: false,
            blocks: 1024,
            bytes: 16 << 20,
            decoded: 1 << 20,
            level: None,
            attrs: vec![],
            context: vec![],
            search: String::new(),
            trace: None,
        };
        match c {
            "scan_filtered" => {
                q.level = Some(4);
                q.attrs.push(Field {
                    key: "status".into(),
                    value: "503".into(),
                });
                q.context.push(Field {
                    key: "host".into(),
                    value: r#""host-2""#.into(),
                })
            }
            "scan_search" => q.search = "completed".into(),
            "scan_trace" => {
                let mut t = [0; 16];
                t[0] = 1;
                t[15] = 1;
                q.trace = Some(t)
            }
            "scan_page" => q.limit = 127,
            "scan_newest" => {
                q.limit = 127;
                q.newest = true
            }
            "scan_budget" => {
                q.blocks = 2;
                q.decoded = 2048
            }
            _ => {}
        };
        q
    }
    fn matches(&self, r: &Record) -> bool {
        r.at >= self.from
            && r.at < self.to
            && self.level.is_none_or(|l| r.level.is_some_and(|v| v >= l))
            && self.trace.is_none_or(|t| r.trace == t)
            && self.attrs.iter().all(|f| r.attrs.contains(f))
            && self.context.iter().all(|f| r.context.contains(f))
            && (self.search.is_empty()
                || r.name.to_lowercase().contains(&self.search)
                || r.body
                    .as_ref()
                    .is_some_and(|s| s.to_lowercase().contains(&self.search)))
    }
}
pub struct Page {
    records: Vec<Record>,
    more: bool,
    from: i64,
    to: i64,
}
fn canon(rs: &[Record]) -> String {
    let mut h = Sha256::new();
    for r in rs {
        let mut b = Vec::new();
        codec::serialize_record(&mut b, r);
        h.update(&b);
        h.update(r.stream.as_bytes());
        h.update([0])
    }
    hex(&h.finalize())
}
fn verify(e: &engine::Engine) -> Result<()> {
    for c in [
        "scan_full",
        "scan_filtered",
        "scan_search",
        "scan_trace",
        "scan_page",
        "scan_newest",
        "scan_budget",
        "follow",
    ] {
        let mut v = if c == "follow" {
            let (rs, next) = e.follow((0, 0), 127)?;
            json!({"count":rs.len(),"hash":canon(&rs),"segment":next.0,"row":next.1,"expired":0})
        } else {
            match e.scan(&Query::for_case(c)) {
                Ok(p) => {
                    json!({"count":p.records.len(),"hash":canon(&p.records),"more":p.more,"from":p.from,"to":p.to})
                }
                Err(err) if err.to_string().starts_with("limit:") => json!({"error":"limit"}),
                Err(err) => return Err(err),
            }
        };
        v["case"] = c.into();
        emit(v)
    }
    Ok(())
}
fn verify_walk(e: &engine::Engine) -> Result<()> {
    for newest in [false, true] {
        let mut q = Query::for_case("scan_page");
        q.newest = newest;
        let mut all = Vec::new();
        let mut pages = 0;
        loop {
            let p = e.scan(&q)?;
            all.extend(p.records);
            pages += 1;
            if !p.more {
                break;
            };
            if pages > 1000 {
                return Err("pagination did not advance".into());
            };
            q.from = p.from;
            q.to = p.to
        }
        emit(
            json!({"walk":"scan","newest":newest,"count":all.len(),"pages":pages,"hash":canon(&all)}),
        )
    }
    let mut all = Vec::new();
    let mut cursor = (0, 0);
    let mut batches = 0;
    loop {
        let (rs, next) = e.follow(cursor, 127)?;
        batches += 1;
        if rs.is_empty() {
            break;
        };
        all.extend(rs);
        cursor = next;
        if batches > 1000 {
            return Err("follow did not advance".into());
        }
    }
    emit(
        json!({"walk":"follow","count":all.len(),"batches":batches,"hash":canon(&all),"segment":cursor.0,"row":cursor.1}),
    );
    Ok(())
}
fn emit(v: Value) {
    println!("{v}")
}
fn rss() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            if l.starts_with("VmRSS:") {
                l.split_whitespace().nth(1)?.parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}
#[allow(dead_code)]
enum Output {
    Page(Page),
    Follow(Vec<Record>, (i64, usize)),
    Count(usize),
}
fn op(e: &mut engine::Engine, c: &str, batch: &[Record]) -> Result<Output> {
    Ok(match c {
        "append" => Output::Count(e.append(batch)?),
        "seal" => Output::Count(e.seal()?),
        "follow" => {
            let (rs, next) = e.follow((0, 0), 127)?;
            Output::Follow(rs, next)
        }
        _ => Output::Page(e.scan(&Query::for_case(c))?),
    })
}
fn kernels() -> Vec<Case> {
    let mut cs = bits::cases();
    cs.push(Case {
        name: "head/serialize".into(),
        units: 1024,
        input_bytes: 0,
        input_hash: 0,
        run: Box::new({
            let rs = fixture(1024);
            move || {
                let mut b = vec![];
                for r in &rs {
                    codec::serialize_record(&mut b, r)
                }
                Payload::Bytes(b)
            }
        }),
    });
    for keyed in [false, true] {
        cs.push(Case {
            name: if keyed {
                "sort/arrival_key"
            } else {
                "sort/stable"
            }
            .into(),
            units: 1024,
            input_bytes: 0,
            input_hash: 0,
            run: Box::new({
                let rs = fixture(1024);
                move || {
                    let mut ids = (0..rs.len() as u64).collect::<Vec<_>>();
                    if keyed {
                        ids.sort_unstable_by_key(|i| (rs[*i as usize].at, *i))
                    } else {
                        ids.sort_by_key(|i| rs[*i as usize].at)
                    };
                    Payload::Words(ids)
                }
            }),
        })
    }
    cs
}
#[cfg(feature = "counting-allocator")]
mod allocation {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering};
    pub static COUNT: AtomicU64 = AtomicU64::new(0);
    pub static BYTES: AtomicU64 = AtomicU64::new(0);
    pub struct Counting;
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            COUNT.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            COUNT.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(n as u64, Ordering::Relaxed);
            unsafe { System.realloc(p, l, n) }
        }
    }
    pub fn snapshot() -> (u64, u64) {
        (COUNT.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed))
    }
}
#[cfg(feature = "counting-allocator")]
#[global_allocator]
static ALLOCATOR: allocation::Counting = allocation::Counting;
fn alloc() -> (u64, u64) {
    #[cfg(feature = "counting-allocator")]
    {
        allocation::snapshot()
    }
    #[cfg(not(feature = "counting-allocator"))]
    {
        (0, 0)
    }
}
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut values = std::collections::HashMap::new();
    while let Some(k) = args.next() {
        values.insert(k, args.next().ok_or("argument value")?);
    }
    let mode = values.get("--mode").map_or("verify", String::as_str);
    let case = values.get("--case").map_or("scan_full", String::as_str);
    let iterations = values
        .get("--iterations")
        .map_or(Ok(64), |v| v.parse::<usize>())?;
    if mode.starts_with("kernel") {
        for c in kernels() {
            if case != "all" && case != c.name {
                continue;
            };
            if mode == "kernel-verify" {
                let bytes = match (c.run)() {
                    Payload::Bytes(b) => b,
                    Payload::Words(w) => w.into_iter().flat_map(|v| v.to_le_bytes()).collect(),
                };
                emit(json!({"case":c.name,"units":c.units,"hex":hex(&bytes)}));
                continue;
            };
            let before = alloc();
            let start = Instant::now();
            let mut sink = None;
            for _ in 0..iterations {
                sink = Some(black_box((c.run)()));
            }
            let elapsed = start.elapsed();
            let after = alloc();
            emit(
                json!({"language":"rust","case":c.name,"iterations":iterations,"ns_per_op":elapsed.as_nanos() as f64/iterations as f64,"allocations_per_op":(after.0-before.0) as f64/iterations as f64,"allocated_bytes_per_op":(after.1-before.1) as f64/iterations as f64}),
            );
            black_box(sink);
        }
        return Ok(());
    }
    let path = Path::new(values.get("--db").ok_or("--db required")?);
    let mut e = engine::Engine::open(path)?;
    if mode == "fixture" {
        let rs = load(values.get("--input").ok_or("--input required")?)?;
        for batch in rs.chunks(1024) {
            e.append(batch)?;
        }
        if case == "sealed" {
            e.seal()?;
        };
        emit(
            json!({"count":rs.len(),"hash":canon(&rs),"input":rs.iter().map(codec::input).sum::<usize>()}),
        );
        return Ok(());
    }
    if mode == "verify" {
        return verify(&e);
    }
    if mode == "seal-now" {
        let count = e.seal()?;
        emit(json!({"count":count}));
        return Ok(());
    }
    if mode == "verify-walk" {
        return verify_walk(&e);
    }
    if mode == "guards" {
        let mut rs = fixture(1);
        rs[0].at = NOW + 601000000000;
        assert!(e.append(&rs).unwrap_err().to_string().contains("too new"));
        rs[0].at = EPOCH;
        rs[0].attrs[0].value = "invalid".into();
        assert!(
            e.append(&rs)
                .unwrap_err()
                .to_string()
                .contains("invalid JSON")
        );
        let mut q = Query::for_case("scan_full");
        q.limit = 10001;
        assert!(e.scan(&q).is_err());
        emit(json!({"guards":"ok"}));
        return Ok(());
    }
    let mut batch = fixture(256);
    if let Some(input) = values.get("--input") {
        for (r, source) in batch.iter_mut().zip(load(input)?) {
            r.body = source.body;
        }
    }
    for _ in 0..if case == "seal" { 0 } else { 4 } {
        black_box(op(&mut e, case, &batch)?);
    }
    let counters = e.counters();
    let before = alloc();
    let start = Instant::now();
    let mut sink = None;
    for _ in 0..iterations {
        sink = Some(black_box(op(&mut e, case, &batch)?));
    }
    let elapsed = start.elapsed();
    let after = alloc();
    let mut retained = vec![];
    if mode == "memory" {
        for _ in 0..iterations {
            retained.push(op(&mut e, case, &batch)?)
        }
    };
    emit(
        json!({"language":"rust","case":case,"iterations":iterations,"ns_per_op":elapsed.as_nanos() as f64/iterations as f64,"allocations_per_op":(after.0-before.0) as f64/iterations as f64,"allocated_bytes_per_op":(after.1-before.1) as f64/iterations as f64,"rss_kib":rss(),"retained":retained.len(),"sqlite_before":counters,"sqlite_after":e.counters(),"sqlite_version":rusqlite::version()}),
    );
    black_box(retained);
    black_box(sink);
    Ok(())
}
