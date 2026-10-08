use crate::{
    Digest, Page, Query, Record, Result, Sha256, alloc, canon, codec::optimized::Rows, emit,
    engine, hex, json, optimized, rss,
};
use std::{collections::HashMap, hint::black_box, path::Path, time::Instant};
enum Driver {
    Previous(engine::Engine),
    Optimized(optimized::Engine),
}
#[allow(dead_code)]
enum Output {
    PreviousPage(Page),
    Page(optimized::Page),
    PreviousFollow(Vec<Record>, (i64, usize)),
    Follow(Rows, (i64, usize)),
}
impl Driver {
    fn open(path: &Path, variant: &str) -> Result<Self> {
        if variant == "previous" {
            return Ok(Self::Previous(engine::Engine::open(path)?));
        };
        let mut options = optimized::Options::default();
        match variant {
            "opt" => {}
            "prune_off" => options.selective = false,
            "heap_off" => options.bounded = false,
            "sharing_off" => options.share = false,
            "cache_off" => options.cache = 0,
            "cache_raw" => options.cache = 1,
            "strict" => options.strict = true,
            _ => return Err("unknown variant".into()),
        };
        Ok(Self::Optimized(optimized::Engine::open(path, options)?))
    }
    fn counters(&self) -> serde_json::Value {
        match self {
            Self::Previous(e) => e.counters(),
            Self::Optimized(e) => e.counters(),
        }
    }
    fn operation(&self, case: &str) -> Result<Output> {
        match self {
            Self::Previous(e) => {
                if case == "follow_walk" {
                    let mut all = Vec::new();
                    let mut cursor = (0, 0);
                    loop {
                        let (rs, next) = e.follow(cursor, 127)?;
                        if rs.is_empty() {
                            break;
                        };
                        all.extend(rs);
                        cursor = next;
                        if all.len() > 20000 {
                            return Err("walk bound".into());
                        }
                    }
                    return Ok(Output::PreviousFollow(all, cursor));
                };
                if case.starts_with("follow") {
                    let (rs, next) = e.follow((0, 0), 127)?;
                    Ok(Output::PreviousFollow(rs, next))
                } else {
                    Ok(Output::PreviousPage(e.scan(&Query::for_case(case))?))
                }
            }
            Self::Optimized(e) => {
                if case == "follow_walk" {
                    let mut all = Rows::default();
                    let mut cursor = (0, 0);
                    loop {
                        let (rs, next) = e.follow(cursor, 127)?;
                        if rs.is_empty() {
                            break;
                        };
                        all.extend(rs);
                        cursor = next;
                        if all.len() > 20000 {
                            return Err("walk bound".into());
                        }
                    }
                    return Ok(Output::Follow(all, cursor));
                };
                if case.starts_with("follow") {
                    let (rs, next) = e.follow((0, 0), 127)?;
                    Ok(Output::Follow(rs, next))
                } else {
                    Ok(Output::Page(e.scan(&Query::for_case(case))?))
                }
            }
        }
    }
    fn verify_walk(&self) -> Result<()> {
        for newest in [false, true] {
            let mut q = Query::for_case("scan_page");
            q.newest = newest;
            let mut hash = Sha256::new();
            let (mut count, mut pages) = (0, 0);
            loop {
                let (n, more, from, to) = match self {
                    Self::Previous(e) => {
                        let p = e.scan(&q)?;
                        hash_records(&p.records, &mut hash);
                        (p.records.len(), p.more, p.from, p.to)
                    }
                    Self::Optimized(e) => {
                        let p = e.scan(&q)?;
                        hash_rows(&p.records, &mut hash)?;
                        (p.records.len(), p.more, p.from, p.to)
                    }
                };
                count += n;
                pages += 1;
                if !more {
                    break;
                };
                q.from = from;
                q.to = to;
                if pages > 1000 {
                    return Err("walk bound".into());
                }
            }
            emit(
                json!({"walk":"scan","newest":newest,"count":count,"pages":pages,"hash":hex(&hash.finalize())}),
            )
        }
        let output = self.operation("follow_walk")?;
        let mut v = output.verification()?;
        v["walk"] = "follow".into();
        emit(v);
        Ok(())
    }
}
fn hash_records(rs: &[Record], h: &mut Sha256) {
    for r in rs {
        let mut b = vec![];
        crate::codec::serialize_record(&mut b, r);
        h.update(b);
        h.update(r.stream.as_bytes());
        h.update([0])
    }
}
fn hash_rows(rs: &Rows, h: &mut Sha256) -> Result<()> {
    hash_records(&rs.owned, h);
    for r in &rs.shared {
        let mut b = vec![];
        r.serialize(&mut b)?;
        h.update(b);
        h.update(r.stream().as_bytes());
        h.update([0])
    }
    Ok(())
}
impl Output {
    fn verification(&self) -> Result<serde_json::Value> {
        Ok(match self {
            Self::PreviousPage(p) => {
                json!({"count":p.records.len(),"hash":canon(&p.records),"more":p.more,"from":p.from,"to":p.to})
            }
            Self::Page(p) => {
                let mut h = Sha256::new();
                hash_rows(&p.records, &mut h)?;
                json!({"count":p.records.len(),"hash":hex(&h.finalize()),"more":p.more,"from":p.from,"to":p.to})
            }
            Self::PreviousFollow(rs, next) => {
                json!({"count":rs.len(),"hash":canon(rs),"segment":next.0,"row":next.1,"expired":0})
            }
            Self::Follow(rs, next) => {
                let mut h = Sha256::new();
                hash_rows(rs, &mut h)?;
                json!({"count":rs.len(),"hash":hex(&h.finalize()),"segment":next.0,"row":next.1,"expired":0})
            }
        })
    }
}
pub fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut options = HashMap::new();
    while let Some(k) = args.next() {
        options.insert(k, args.next().ok_or("argument value")?);
    }
    let get = |k: &str, default: &str| options.get(k).cloned().unwrap_or_else(|| default.into());
    let mode = get("--mode", "verify");
    let variant = get("--variant", "opt");
    let case = get("--case", "scan_full");
    let path = get("--db", "");
    let iterations = get("--iterations", "32").parse::<usize>()?;
    let warm = get("--warm", "4").parse::<usize>()?;
    let mut driver = Driver::open(Path::new(&path), &variant)?;
    if mode == "verify" {
        for case in [
            "scan_full",
            "scan_filtered",
            "scan_search",
            "scan_trace",
            "scan_page",
            "scan_newest",
            "scan_budget",
            "scan_none",
            "scan_no_context",
            "follow",
        ] {
            let mut row = match driver.operation(case) {
                Ok(output) => output.verification()?,
                Err(e) if e.to_string().starts_with("limit:") => json!({"error":"limit"}),
                Err(e) => return Err(e),
            };
            row["case"] = case.into();
            emit(row)
        }
        return Ok(());
    }
    if mode == "verify-walk" {
        return driver.verify_walk();
    }
    if mode == "snapshot" {
        emit(driver.operation(&case)?.verification()?);
        return Ok(());
    }
    if mode == "cursor" {
        let cursor = (
            get("--segment", "0").parse::<i64>()?,
            get("--row", "0").parse::<usize>()?,
        );
        let limit = get("--limit", "127").parse::<usize>()?;
        let out = match &driver {
            Driver::Previous(e) => {
                let (rs, next) = e.follow(cursor, limit)?;
                Output::PreviousFollow(rs, next)
            }
            Driver::Optimized(e) => {
                let (rs, next) = e.follow(cursor, limit)?;
                Output::Follow(rs, next)
            }
        };
        emit(out.verification()?);
        return Ok(());
    }
    if mode == "lifetime" {
        let output = driver.operation(&case)?;
        let before = output.verification()?;
        let sibling = driver.operation(&case)?;
        drop(sibling);
        drop(driver);
        let after = output.verification()?;
        assert_eq!(before, after);
        emit(json!({"lifetime":"engine and connections destroyed","result":after}));
        return Ok(());
    }
    if mode == "cache-guards" {
        let Driver::Optimized(e) = driver else {
            return Err("optimized only".into());
        };
        let result = e.follow((0, 0), 127)?;
        if result.0.is_empty() {
            return Err("sealed fixture required".into());
        };
        let before = {
            let mut h = Sha256::new();
            hash_rows(&result.0, &mut h)?;
            hex(&h.finalize())
        };
        let (id, count, body): (i64, i64, Vec<u8>) = e.base.writer.query_row(
            "select id,count,body from blocks order by id limit 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        e.base
            .writer
            .execute("update blocks set count=count-1 where id=?", [id])?;
        assert!(e.follow((0, 0), 127).is_err());
        e.base.writer.execute(
            "update blocks set count=? where id=?",
            rusqlite::params![count, id],
        )?;
        let mut corrupt = body.clone();
        let n = corrupt.len();
        corrupt[n - 1] ^= 1;
        e.base.writer.execute(
            "update blocks set body=? where id=?",
            rusqlite::params![corrupt, id],
        )?;
        let cached = e.follow((0, 0), 127)?;
        let mut h = Sha256::new();
        hash_rows(&cached.0, &mut h)?;
        assert_eq!(before, hex(&h.finalize()));
        e.clear_cache();
        assert!(e.follow((0, 0), 127).is_err());
        e.base.writer.execute(
            "update blocks set body=? where id=?",
            rusqlite::params![body, id],
        )?;
        drop(cached);
        drop(e);
        let mut h = Sha256::new();
        hash_rows(&result.0, &mut h)?;
        assert_eq!(before, hex(&h.finalize()));
        emit(
            json!({"cache_index_mutation_rejected":true,"immutable_cached_bytes_survive_external_same_id_mutation":true,"cleared_cache_crc_mutation_rejected":true,"result_outlives_cache_engine_and_sibling":true}),
        );
        return Ok(());
    }
    if mode == "guards" {
        let Driver::Optimized(e) = &driver else {
            return Err("guards optimized only".into());
        };
        let mut q = Query::for_case("scan_full");
        q.limit = 10001;
        assert!(e.scan(&q).is_err());
        q.limit = 1;
        assert!(e.scan(&q).err().unwrap().to_string().starts_with("limit:"));
        q.limit = 10000;
        q.bytes = 1;
        assert!(e.scan(&q).is_err());
        assert!(e.follow((-1, 0), 127).is_err());
        assert!(e.follow((0, 0), 10001).is_err());
        let _ = e.follow((0, 0), 127)?;
        e.clear_cache();
        assert_eq!(e.counters()["follow_cache_bytes"], 0);
        emit(json!({"guards":"ok"}));
        return Ok(());
    }
    if get("--cold", "0") == "1" {
        let mut nanos = 0u128;
        let (mut calls, mut bytes) = (0u64, 0u64);
        let mut sink = None;
        for _ in 0..iterations {
            driver = Driver::open(Path::new(&path), &variant)?;
            let before = alloc();
            let start = Instant::now();
            sink = Some(driver.operation(&case)?);
            nanos += start.elapsed().as_nanos();
            let after = alloc();
            calls += after.0 - before.0;
            bytes += after.1 - before.1
        }
        emit(
            json!({"language":"rust","variant":variant,"case":case,"iterations":iterations,"cold":true,"setup_excluded":true,"ns_per_op":nanos as f64/iterations as f64,"allocations_per_op":calls as f64/iterations as f64,"allocated_bytes_per_op":bytes as f64/iterations as f64,"sqlite_after":driver.counters()}),
        );
        black_box(sink);
        return Ok(());
    }
    for _ in 0..warm {
        black_box(driver.operation(&case)?);
    }
    let before_counters = driver.counters();
    let before = alloc();
    let start = Instant::now();
    let mut sink = None;
    for _ in 0..iterations {
        sink = Some(black_box(driver.operation(&case)?));
    }
    let elapsed = start.elapsed();
    let after = alloc();
    let mut retained = Vec::new();
    if mode == "memory" {
        for _ in 0..iterations {
            retained.push(driver.operation(&case)?);
        }
    };
    emit(
        json!({"language":"rust","variant":variant,"case":case,"iterations":iterations,"warm":warm,"ns_per_op":elapsed.as_nanos() as f64/iterations as f64,"allocations_per_op":(after.0-before.0)as f64/iterations as f64,"allocated_bytes_per_op":(after.1-before.1)as f64/iterations as f64,"rss_kib":rss(),"retained":retained.len(),"sqlite_before":before_counters,"sqlite_after":driver.counters(),"sqlite_version":rusqlite::version()}),
    );
    black_box(sink);
    black_box(retained);
    Ok(())
}
