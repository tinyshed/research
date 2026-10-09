use crate::{
    Result, codec,
    engine::Sample,
    pack::{self, Address},
};
use rusqlite::{Connection, MAIN_DB, OpenFlags, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::Instant,
};

#[derive(Clone)]
struct Saved {
    group: codec::Group,
    original: Vec<u8>,
    addresses: Vec<Address>,
    metadata_bytes: usize,
}

fn connect(path: &Path, read_only: bool) -> Result<Connection> {
    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let db = Connection::open_with_flags(path, flags)?;
    db.execute_batch("PRAGMA busy_timeout=5000; PRAGMA synchronous=FULL; PRAGMA fullfsync=1; PRAGMA checkpoint_fullfsync=1; PRAGMA cache_size=-1024; PRAGMA mmap_size=0; PRAGMA trusted_schema=0;")?;
    if read_only {
        db.execute_batch("PRAGMA query_only=1")?;
    } else {
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=0")?;
    }
    db.set_prepared_statement_cache_capacity(32);
    Ok(db)
}

fn saved(
    _db: &Connection,
    id: i64,
    start: i64,
    end: i64,
    data: Vec<u8>,
    clock_id: i64,
    clock: Vec<u8>,
) -> Result<Saved> {
    let metadata_bytes = data.len() + clock.len();
    let (original, addresses) = pack::unwrap(id, start, end, &data)?;
    let clock = codec::decode_clock_group(&clock)?;
    let group = codec::read_directory(id, start, end, clock_id, &original, &clock)?;
    let external = group
        .blocks
        .iter()
        .enumerate()
        .filter(|(slot, _)| group.is_external(*slot))
        .count();
    if data[0] != 4 && external != addresses.len() {
        return Err("pack address allocation mismatch".into());
    }
    Ok(Saved {
        group,
        original,
        addresses,
        metadata_bytes,
    })
}

fn all_groups(db: &Connection) -> Result<Vec<Saved>> {
    let mut statement=db.prepare("SELECT g.series_id,g.start_ts,g.end_ts,g.directory,g.clock_id,c.body FROM groups g JOIN clocks c ON c.id=g.clock_id ORDER BY g.series_id,g.start_ts")?;
    let rows = statement.query_map([], |r| {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
        ))
    })?;
    rows.map(|r| {
        let (id, start, end, data, clock_id, clock) = r?;
        saved(db, id, start, end, data, clock_id, clock)
    })
    .collect()
}

fn fetch_group(db: &Connection, id: i64, start: i64) -> Result<Saved> {
    let tuple=db.prepare_cached("SELECT g.end_ts,g.directory,g.clock_id,c.body FROM groups g JOIN clocks c ON c.id=g.clock_id WHERE g.series_id=? AND g.start_ts=?")?.query_row(params![id,start],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    saved(db, id, start, tuple.0, tuple.1, tuple.2, tuple.3)
}

pub fn build(input: &Path, output: &Path, spec: &str) -> Result<()> {
    if output.exists() {
        return Err("output already exists; use a fresh named file".into());
    }
    fs::create_dir_all(output.parent().ok_or("output parent")?)?;
    let src = connect(input, true)?;
    let groups = all_groups(&src)?;
    let mut bodies = Vec::new();
    // Chronological publication-like order, with stable original group order.
    for s in &groups {
        for (slot, b) in s.group.blocks.iter().enumerate() {
            if s.group.is_external(slot) && s.group.is_live(slot) {
                bodies.push((
                    b.payload,
                    src.query_row("SELECT body FROM payloads WHERE id=?", [b.payload], |r| {
                        r.get(0)
                    })?,
                ));
            }
        }
    }
    let (packs, addresses) = if spec == "baseline" {
        (vec![], BTreeMap::new())
    } else {
        pack::pack_bodies(&bodies, spec)?
    };
    let mut db = connect(output, false)?;
    db.execute(
        "ATTACH DATABASE ? AS original",
        [input.to_str().ok_or("input path")?],
    )?;
    let schema:Vec<(String,String,String)>=src.prepare("SELECT type,name,sql FROM sqlite_schema WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY rowid")?.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    let start = Instant::now();
    let tx = db.transaction()?;
    for (kind, name, sql) in &schema {
        if kind == "table" && (name != "payloads" || spec == "baseline") {
            tx.execute_batch(sql)?;
        }
    }
    if spec != "baseline" {
        tx.execute_batch("CREATE TABLE payload_packs(id INTEGER PRIMARY KEY, refs INTEGER NOT NULL CHECK(refs>0), live_bytes INTEGER NOT NULL CHECK(live_bytes>0), body BLOB NOT NULL CHECK(length(body)<=16384)) STRICT;")?;
    }
    for (kind, name, _) in &schema {
        if kind == "table" && name != "groups" && (name != "payloads" || spec == "baseline") {
            let quoted = format!("\"{}\"", name.replace('"', "\"\""));
            tx.execute_batch(&format!(
                "INSERT INTO main.{quoted} SELECT * FROM original.{quoted}"
            ))?;
        }
    }
    for s in &groups {
        let directory = if spec == "baseline" {
            s.original.clone()
        } else {
            let refs =
                s.group
                    .blocks
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| s.group.is_external(*i))
                    .map(|(_, b)| {
                        addresses.get(&b.payload).copied().ok_or(
                            "source contains dead external block; fresh full fixture required",
                        )
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?;
            pack::wrap(
                s.group.series_id,
                s.group.start,
                s.group.end,
                &s.original,
                &refs,
            )?
        };
        tx.execute(
            "INSERT INTO groups(series_id,start_ts,end_ts,directory,clock_id) VALUES(?,?,?,?,?)",
            params![
                s.group.series_id,
                s.group.start,
                s.group.end,
                directory,
                s.group.clock_id
            ],
        )?;
    }
    for (id, body, refs, live_bytes) in &packs {
        tx.execute(
            "INSERT INTO payload_packs VALUES(?,?,?,?)",
            params![id, *refs as i64, *live_bytes as i64, body],
        )?;
    }
    for (kind, _, sql) in &schema {
        if kind != "table" {
            tx.execute_batch(sql)?;
        }
    }
    tx.commit()?;
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    let wal_bytes = fs::metadata(format!("{}-wal", output.display()))
        .map(|m| m.len())
        .unwrap_or(0);
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    drop(db);
    println!(
        "{}",
        json!({"event":"build","spec":spec,"build_ms":build_ms,"wal_before_checkpoint_bytes":wal_bytes,"body_bytes":bodies.iter().map(|b|b.1.len()).sum::<usize>(),"body_rows":bodies.len(),"pack_rows":packs.len(),"inspect":inspect(output)?})
    );
    Ok(())
}

pub fn inspect(path: &Path) -> Result<Value> {
    let db = connect(path, true)?;
    // The matched production SQLite archive omits dbstat. The orchestrator
    // adds read-only physical scans using system SQLite's dbstat, outside all
    // timings, and the Go internal/dbstat cross-check uses the same closed file.
    let objects: Vec<Value> = vec![];
    let packed = db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name='payload_packs'",
        [],
        |r| r.get::<_, i64>(0),
    )? == 1;
    let pack_info = if packed {
        db.query_row("SELECT count(*),coalesce(sum(length(body)),0),coalesce(sum(refs),0),coalesce(sum(live_bytes),0),coalesce(sum(length(body)-24-live_bytes),0) FROM payload_packs",[],|r|Ok(json!({"rows":r.get::<_,i64>(0)?,"bytes":r.get::<_,i64>(1)?,"live_refs":r.get::<_,i64>(2)?,"live_body_bytes":r.get::<_,i64>(3)?,"dead_body_bytes":r.get::<_,i64>(4)?})))?
    } else {
        Value::Null
    };
    Ok(
        json!({"file_bytes":fs::metadata(path)?.len(),"wal_bytes":fs::metadata(format!("{}-wal",path.display())).map(|m|m.len()).unwrap_or(0),"page_size":db.query_row("PRAGMA page_size",[],|r|r.get::<_,i64>(0))?,"page_count":db.query_row("PRAGMA page_count",[],|r|r.get::<_,i64>(0))?,"freelist_pages":db.query_row("PRAGMA freelist_count",[],|r|r.get::<_,i64>(0))?,"directory_bytes":db.query_row("SELECT coalesce(sum(length(directory)),0) FROM groups",[],|r|r.get::<_,i64>(0))?,"packs":pack_info,"objects":objects,"sqlite_version":db.query_row("SELECT sqlite_version()",[],|r|r.get::<_,String>(0))?,"sqlite_source_id":db.query_row("SELECT sqlite_source_id()",[],|r|r.get::<_,String>(0))?}),
    )
}

#[derive(Default, Clone)]
struct Counters {
    requested: usize,
    copied: usize,
    slice_copied: usize,
    metadata: usize,
    packs: usize,
    blob_opens: usize,
    blob_reopens: usize,
    blocks: usize,
    decoded: usize,
    returned: usize,
    cache_hits: i32,
    cache_misses: i32,
}
impl Counters {
    fn value(&self) -> Value {
        json!({"requested_body_bytes":self.requested,"copied_blob_bytes":self.copied,"extra_body_slice_copy_bytes":self.slice_copied,"copied_metadata_bytes":self.metadata,"pack_fetches":self.packs,"blob_opens":self.blob_opens,"blob_reopens":self.blob_reopens,"blocks":self.blocks,"decoded_samples":self.decoded,"returned_samples":self.returned,"sqlite_cache_hits":self.cache_hits,"sqlite_cache_misses":self.cache_misses})
    }
}

fn status(db: &Connection, which: i32, reset: bool) -> Result<i32> {
    let (mut current, mut high) = (0, 0);
    // Read-only connection diagnostics. No pointer escapes, and SQLite owns it
    // for this immediate call. There is no unsafe BLOB access in this harness.
    let rc = unsafe {
        rusqlite::ffi::sqlite3_db_status(
            db.handle(),
            which,
            &mut current,
            &mut high,
            i32::from(reset),
        )
    };
    if rc != rusqlite::ffi::SQLITE_OK {
        return Err("sqlite db_status".into());
    }
    Ok(current)
}

fn fetch_bodies(
    db: &Connection,
    blocks: &mut [(codec::Block, Option<Address>)],
    mode: &str,
    c: &mut Counters,
) -> Result<()> {
    let mut by_pack: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    let mut baseline: BTreeMap<i64, usize> = BTreeMap::new();
    for (i, (b, a)) in blocks.iter_mut().enumerate() {
        c.blocks += 1;
        if let Some(a) = a {
            c.requested += b.body_bytes;
            by_pack.entry(a.pack).or_default().push(i);
        } else if b.payload != 0 {
            c.requested += b.body_bytes;
            if baseline.insert(b.payload, i).is_some() {
                return Err("duplicate selected payload reference".into());
            }
        }
    }
    if c.requested > 4 << 20 {
        return Err("payload request budget".into());
    }
    if !baseline.is_empty() {
        let ids = serde_json::to_string(&baseline.keys().collect::<Vec<_>>())?;
        let mut statement=db.prepare_cached("SELECT p.id,p.body FROM json_each(?) ids JOIN payloads p ON p.id=CAST(ids.value AS INTEGER)")?;
        let rows = statement.query_map([ids], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        let mut found = 0;
        for row in rows {
            let (id, body) = row?;
            let i = *baseline.get(&id).ok_or("unexpected payload id")?;
            if body.len() != blocks[i].0.body_bytes {
                return Err("baseline selected body length".into());
            }
            c.copied += body.len();
            c.packs += 1;
            blocks[i].0.body = body;
            found += 1;
        }
        if found != baseline.len() {
            return Err("missing selected payload".into());
        }
    }
    if mode == "whole" {
        let ids = serde_json::to_string(&by_pack.keys().collect::<Vec<_>>())?;
        if !by_pack.is_empty() {
            let mut statement=db.prepare_cached("SELECT p.id,CASE WHEN length(p.body)<=16384 THEN p.body ELSE NULL END FROM json_each(?) ids JOIN payload_packs p ON p.id=CAST(ids.value AS INTEGER)")?;
            let rows = statement.query_map([ids], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?;
            let mut found = 0;
            for row in rows {
                let (id, body) = row?;
                let indices = by_pack.get(&id).ok_or("unexpected pack id")?;
                found += 1;
                pack::check_header(
                    id,
                    body.len(),
                    body.get(..pack::HEADER).ok_or("truncated pack")?,
                )?;
                c.copied += body.len();
                c.packs += 1;
                if c.copied > 4 << 20 {
                    return Err("copied payload budget".into());
                }
                for i in indices {
                    let (b, a) = &mut blocks[*i];
                    let a = a.unwrap();
                    let end = a
                        .offset
                        .checked_add(b.body_bytes)
                        .ok_or("offset overflow")?;
                    let selected = body.get(a.offset..end).ok_or("body outside pack")?;
                    pack::check_body(a, b.body_bytes, body.len(), selected)?;
                    b.body = selected.to_vec();
                    c.slice_copied += selected.len();
                }
            }
            if found != by_pack.len() {
                return Err("missing selected pack".into());
            }
        }
    } else if mode == "range" {
        let mut handle: Option<rusqlite::blob::Blob<'_>> = None;
        for (id, indices) in by_pack {
            if let Some(handle) = handle.as_mut() {
                handle.reopen(id)?;
                c.blob_reopens += 1;
            } else {
                handle = Some(db.blob_open(MAIN_DB, "payload_packs", "body", id, true)?);
                c.blob_opens += 1;
            }
            let handle = handle.as_ref().unwrap();
            let length = handle.len();
            let mut header = [0; pack::HEADER];
            handle.read_at_exact(&mut header, 0)?;
            pack::check_header(id, length, &header)?;
            c.packs += 1;
            c.copied += pack::HEADER;
            for i in indices {
                let (b, a) = &mut blocks[i];
                let a = a.unwrap();
                if a.offset
                    .checked_add(b.body_bytes)
                    .is_none_or(|end| end > length)
                {
                    return Err("body outside incremental pack".into());
                }
                b.body = vec![0; b.body_bytes];
                handle.read_at_exact(&mut b.body, a.offset)?;
                pack::check_body(a, b.body_bytes, length, &b.body)?;
                c.copied += b.body.len();
            }
        }
        if let Some(handle) = handle {
            handle.close()?;
        }
    } else {
        return Err("unknown fetch mode".into());
    }
    if c.copied > 4 << 20 {
        return Err("copied payload budget".into());
    }
    Ok(())
}

type Selection = BTreeMap<(i64, i64), BTreeSet<usize>>;
fn selection(db: &Connection, case: &str) -> Result<Selection> {
    let groups = all_groups(db)?;
    let all: Vec<(i64, i64, usize)> = groups
        .iter()
        .flat_map(|s| {
            s.group
                .blocks
                .iter()
                .enumerate()
                .filter(|(i, _)| s.group.is_live(*i))
                .map(|(i, _)| (s.group.series_id, s.group.start, i))
        })
        .collect();
    if all.is_empty() {
        return Err("no blocks".into());
    }
    let external: Vec<_> = groups
        .iter()
        .flat_map(|s| {
            s.group
                .blocks
                .iter()
                .enumerate()
                .filter(|(i, _)| s.group.is_live(*i) && s.group.is_external(*i))
                .map(|(i, _)| (s.group.series_id, s.group.start, i))
        })
        .collect();
    let point = external
        .get(external.len() / 2)
        .or_else(|| all.get(all.len() / 2))
        .copied()
        .unwrap();
    let choices: Vec<_> = match case {
        "point" => vec![point],
        "sparse" => (0..16.min(all.len()))
            .map(|i| all[i * all.len() / 16.min(all.len())])
            .collect(),
        "range" => all
            .iter()
            .copied()
            .filter(|v| v.0 == point.0)
            .take(8)
            .collect(),
        "full" | "summary" => all
            .iter()
            .copied()
            .filter(|v| v.0 == point.0)
            .take(416)
            .collect(),
        _ => return Err("unknown query case".into()),
    };
    let mut out = Selection::new();
    for (series, start, slot) in choices {
        out.entry((series, start)).or_default().insert(slot);
    }
    Ok(out)
}

fn execute(
    db: &Connection,
    mode: &str,
    selected: &Selection,
    case: &str,
    diagnostics: bool,
) -> Result<(u64, Counters)> {
    let summary = case == "summary";
    if diagnostics {
        status(db, rusqlite::ffi::SQLITE_DBSTATUS_CACHE_HIT, true)?;
        status(db, rusqlite::ffi::SQLITE_DBSTATUS_CACHE_MISS, true)?;
    }
    let start = Instant::now();
    db.execute_batch("BEGIN")?;
    let mut blocks = Vec::new();
    let mut summaries = Vec::new();
    let mut c = Counters::default();
    let mut digest = 0u64;
    for ((series, start), slots) in selected {
        let s = fetch_group(db, *series, *start)?;
        c.metadata += s.metadata_bytes;
        let mut external = 0;
        for (slot, b) in s.group.blocks.into_iter().enumerate() {
            let a = if s.group.allocation & (1 << slot) != 0 {
                let a = s.addresses.get(external).copied();
                external += 1;
                a
            } else {
                None
            };
            if !slots.contains(&slot) || s.group.live & (1 << slot) == 0 {
                continue;
            }
            if summary {
                summaries.push((*series, b.head.count, b.summary));
                c.blocks += 1;
            } else {
                blocks.push((b, a));
            }
        }
    }
    if !summary {
        fetch_bodies(db, &mut blocks, mode, &mut c)?;
    }
    if start.elapsed().as_secs() > 5 {
        return Err("snapshot deadline".into());
    }
    // All Blob handles/statements are closed; decode only after ending snapshot.
    db.execute_batch("COMMIT")?;
    if diagnostics {
        c.cache_hits = status(db, rusqlite::ffi::SQLITE_DBSTATUS_CACHE_HIT, false)?;
        c.cache_misses = status(db, rusqlite::ffi::SQLITE_DBSTATUS_CACHE_MISS, false)?;
    }
    let mut sums: BTreeMap<i64, (usize, num_bigint::BigInt, bool)> = BTreeMap::new();
    for (series, count, summary) in summaries {
        let entry = sums
            .entry(series)
            .or_insert((0, num_bigint::BigInt::from(0), true));
        entry.0 += count;
        if summary.exact_sum.is_empty() {
            entry.2 = false;
        } else {
            entry.1 += codec::read_exact_value(&summary.exact_sum)?;
        }
    }
    for (series, (count, sum, valid)) in sums {
        digest = digest.wrapping_mul(31) ^ series as u64;
        digest = digest.wrapping_mul(31) ^ count as u64;
        if valid {
            digest = digest.wrapping_mul(31) ^ crate::exact::rounded_exact(&sum).0.to_bits();
        } else {
            digest ^= u64::MAX;
        }
    }
    for (b, _) in blocks {
        if c.decoded + b.head.count > 100_000 {
            return Err("decode query budget".into());
        }
        let points = codec::decode_block(&b)?;
        c.decoded += points.len();
        let midpoint = points.len() / 2;
        for (i, p) in points.into_iter().enumerate() {
            if case == "point" && i != midpoint {
                continue;
            }
            c.returned += 1;
            digest = digest.wrapping_mul(31) ^ p.at as u64;
            digest = digest.wrapping_mul(31) ^ p.value.to_bits();
        }
    }
    Ok((digest, c))
}

pub fn bench(path: &Path, mode: &str, case: &str, cache: &str, iterations: usize) -> Result<()> {
    if iterations == 0 {
        return Err("zero iterations".into());
    }
    let db = connect(path, true)?;
    let selected = selection(&db, case)?;
    for _ in 0..8 {
        std::hint::black_box(execute(&db, mode, &selected, case, false)?);
    }
    let mut total = Counters::default();
    let mut final_digest = 0;
    let mut nanos = 0u128;
    for _ in 0..iterations {
        if cache == "cold" {
            db.execute_batch("PRAGMA shrink_memory")?;
        } else if cache != "warm" {
            return Err("unknown cache mode".into());
        }
        let start = Instant::now();
        let (digest, c) = execute(&db, mode, &selected, case, false)?;
        nanos += start.elapsed().as_nanos();
        std::hint::black_box(digest);
        final_digest = digest;
        total.requested += c.requested;
        total.copied += c.copied;
        total.slice_copied += c.slice_copied;
        total.metadata += c.metadata;
        total.packs += c.packs;
        total.blob_opens += c.blob_opens;
        total.blob_reopens += c.blob_reopens;
        total.blocks += c.blocks;
        total.decoded += c.decoded;
        total.returned += c.returned;
        total.cache_hits += c.cache_hits;
        total.cache_misses += c.cache_misses;
    }
    println!(
        "{}",
        json!({"mode":mode,"case":case,"cache":cache,"iterations":iterations,"ns_per_op":nanos as f64/iterations as f64,"digest":format!("{final_digest:016x}"),"totals":total.value()})
    );
    Ok(())
}

/// Cache counters and strace are a separate, untimed diagnostic process.
pub fn diagnose(path: &Path, mode: &str, case: &str, cache: &str) -> Result<()> {
    let db = connect(path, true)?;
    let selected = selection(&db, case)?;
    for _ in 0..8 {
        std::hint::black_box(execute(&db, mode, &selected, case, false)?);
    }
    if cache == "cold" {
        db.execute_batch("PRAGMA shrink_memory")?;
    } else if cache != "warm" {
        return Err("unknown cache mode".into());
    }
    let (digest, c) = execute(&db, mode, &selected, case, true)?;
    println!(
        "{}",
        json!({"mode":mode,"case":case,"cache":cache,"digest":format!("{digest:016x}"),"counters":c.value()})
    );
    Ok(())
}

fn logical(
    db: &Connection,
    mode: &str,
    cutoff: i64,
) -> Result<(BTreeMap<i64, (usize, String)>, String)> {
    let mut series: BTreeMap<i64, Vec<Sample>> = BTreeMap::new();
    let mut summaries = Sha256::new();
    for s in all_groups(db)? {
        let mut blocks = Vec::new();
        let mut external = 0;
        for (slot, b) in s.group.blocks.into_iter().enumerate() {
            let a = if s.group.allocation & (1 << slot) != 0 {
                let a = s.addresses.get(external).copied();
                external += 1;
                a
            } else {
                None
            };
            if s.group.live & (1 << slot) != 0 {
                summaries.update(s.group.series_id.to_le_bytes());
                summaries.update(b.head.start.to_le_bytes());
                for f in [
                    b.summary.last,
                    b.summary.min,
                    b.summary.max,
                    b.summary.sum,
                    b.summary.increase,
                ] {
                    summaries.update(f.to_bits().to_le_bytes());
                }
                summaries.update(b.summary.resets.to_le_bytes());
                summaries.update([u8::from(b.summary.valid)]);
                summaries.update((b.summary.exact_sum.len() as u64).to_le_bytes());
                summaries.update(&b.summary.exact_sum);
                summaries.update((b.summary.exact_increase.len() as u64).to_le_bytes());
                summaries.update(&b.summary.exact_increase);
                blocks.push((b, a));
            }
        }
        fetch_bodies(db, &mut blocks, mode, &mut Counters::default())?;
        for (b, _) in blocks {
            series.entry(s.group.series_id).or_default().extend(
                codec::decode_block(&b)?
                    .into_iter()
                    .filter(|p| p.at >= cutoff),
            );
        }
    }
    let mut heads=db.prepare("SELECT series_id,head_count,head_start,head_end,tail FROM series_state WHERE tail IS NOT NULL")?;
    let rows = heads.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Vec<u8>>(4)?,
        ))
    })?;
    for row in rows {
        let (id, count, start, end, tail) = row?;
        series.entry(id).or_default().extend(
            codec::decode_head(
                id,
                usize::try_from(count)?,
                start,
                end,
                &tail,
                1 << 20,
                16 << 20,
            )?
            .into_iter()
            .filter(|p| p.at >= cutoff),
        );
    }
    let mut out = BTreeMap::new();
    for (id, mut points) in series {
        points.sort_by_key(|p| p.at);
        let mut h = Sha256::new();
        for p in &points {
            h.update(p.at.to_le_bytes());
            h.update(p.value.to_bits().to_le_bytes());
        }
        out.insert(id, (points.len(), format!("{:x}", h.finalize())));
    }
    Ok((out, format!("{:x}", summaries.finalize())))
}

pub fn verify(original: &Path, candidate: &Path) -> Result<Value> {
    let a = connect(original, true)?;
    let b = connect(candidate, true)?;
    let left = logical(&a, "whole", i64::MIN)?;
    let whole = logical(&b, "whole", i64::MIN)?;
    let range = logical(&b, "range", i64::MIN)?;
    if left != whole || whole != range {
        return Err("bitwise samples or exact summary bytes differ".into());
    }
    let manifest = original.parent().unwrap().join("manifest.json");
    let mut independent = 0;
    if manifest.exists() {
        let expected: Value = serde_json::from_slice(&fs::read(&manifest)?)?;
        for row in expected["series"].as_array().ok_or("manifest series")? {
            let id = row["id"].as_i64().ok_or("manifest id")?;
            let actual = left.0.get(&id).ok_or("missing manifest series")?;
            if actual.0 as u64 != row["count"].as_u64().ok_or("manifest count")?
                || actual.1 != row["sha256"].as_str().ok_or("manifest digest")?
            {
                return Err(
                    format!("independent Go public query hash differs for series {id}").into(),
                );
            }
            independent += 1;
        }
    }
    for table in [
        "store_state",
        "series",
        "label_values",
        "postings",
        "series_state",
        "clocks",
    ] {
        let sql = format!("SELECT * FROM {table}");
        let mut aa = a.prepare(&sql)?;
        let mut bb = b.prepare(&sql)?;
        let cols = aa.column_count();
        let mut ar = aa.query([])?;
        let mut br = bb.query([])?;
        loop {
            match (ar.next()?, br.next()?) {
                (None, None) => break,
                (Some(x), Some(y)) => {
                    for i in 0..cols {
                        if x.get_ref(i)? != y.get_ref(i)? {
                            return Err(format!("unchanged table {table} differs").into());
                        }
                    }
                }
                _ => return Err("unchanged row count differs".into()),
            }
        }
    }
    Ok(
        json!({"verified":true,"series":left.0.len(),"samples":left.0.values().map(|v|v.0).sum::<usize>(),"independent_go_series":independent,"exact_summary_sha256":left.1,"series_hashes":left.0}),
    )
}

/// Fast exhaustive density guard. Every original v4 metadata byte and every
/// encoded body's byte must be identical through both retrieval mechanisms.
/// Full decoded Go-manifest replay is additional, not inferred from this hash.
pub fn verify_bytes(original: &Path, candidate: &Path) -> Result<Value> {
    let a = connect(original, true)?;
    let b = connect(candidate, true)?;
    let left = all_groups(&a)?;
    let right = all_groups(&b)?;
    if left.len() != right.len() {
        return Err("group count differs".into());
    }
    let mut hash = Sha256::new();
    let mut blocks = 0;
    let mut bytes = 0;
    for (l, r) in left.into_iter().zip(right) {
        if (
            l.group.series_id,
            l.group.start,
            l.group.end,
            l.group.clock_id,
        ) != (
            r.group.series_id,
            r.group.start,
            r.group.end,
            r.group.clock_id,
        ) || l.original != r.original
        {
            return Err("original group metadata differs".into());
        }
        let mut original = l
            .group
            .blocks
            .into_iter()
            .map(|b| (b, None))
            .collect::<Vec<_>>();
        fetch_bodies(&a, &mut original, "whole", &mut Counters::default())?;
        let mut external = 0;
        let candidate = r
            .group
            .blocks
            .into_iter()
            .enumerate()
            .map(|(slot, b)| {
                let address = if r.group.allocation & (1 << slot) != 0 {
                    let a = r.addresses.get(external).copied();
                    external += 1;
                    a
                } else {
                    None
                };
                (b, address)
            })
            .collect::<Vec<_>>();
        let mut whole = candidate.clone();
        let mut range = candidate;
        fetch_bodies(&b, &mut whole, "whole", &mut Counters::default())?;
        fetch_bodies(&b, &mut range, "range", &mut Counters::default())?;
        for ((o, w), r) in original.into_iter().zip(whole).zip(range) {
            if o.0.clock != w.0.clock
                || w.0.clock != r.0.clock
                || o.0.body != w.0.body
                || w.0.body != r.0.body
            {
                return Err("encoded body or clock bytes differ".into());
            }
            hash.update(o.0.payload.to_le_bytes());
            hash.update((o.0.body.len() as u64).to_le_bytes());
            hash.update(&o.0.body);
            bytes += o.0.body.len();
            blocks += 1;
        }
    }
    Ok(
        json!({"verified_encoded_bytes":true,"blocks":blocks,"body_bytes":bytes,"encoded_sha256":format!("{:x}",hash.finalize())}),
    )
}

fn expire(db: &mut Connection, cutoff: i64, even_only: bool) -> Result<Value> {
    let before = Instant::now();
    let groups = all_groups(db)?;
    let packed = db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name='payload_packs'",
        [],
        |r| r.get::<_, i64>(0),
    )? == 1;
    let tx = db.transaction()?;
    let mut expired = 0;
    let mut deltas: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    for mut s in groups {
        if even_only && s.group.series_id % 2 != 0 {
            continue;
        }
        let mut external = 0;
        let old_live = s.group.live;
        for (slot, b) in s.group.blocks.iter().enumerate() {
            let a = if s.group.is_external(slot) {
                let a = s.addresses.get(external).copied();
                external += 1;
                a
            } else {
                None
            };
            if s.group.is_live(slot) && b.head.end < cutoff {
                s.group.live &= !(1 << slot);
                expired += 1;
                if let Some(a) = a {
                    let entry = deltas.entry(a.pack).or_default();
                    entry.0 += 1;
                    entry.1 += b.body_bytes as i64;
                } else if b.payload != 0 {
                    tx.execute("DELETE FROM payloads WHERE id=?", [b.payload])?;
                }
            }
        }
        if old_live == s.group.live {
            continue;
        }
        if s.group.live == 0 {
            tx.execute(
                "DELETE FROM groups WHERE series_id=? AND start_ts=?",
                params![s.group.series_id, s.group.start],
            )?;
            tx.execute(
                "UPDATE clocks SET refs=refs-1 WHERE id=?",
                [s.group.clock_id],
            )?;
            tx.execute(
                "DELETE FROM clocks WHERE id=? AND refs=0",
                [s.group.clock_id],
            )?;
        } else {
            pack::set_live(
                s.group.series_id,
                s.group.start,
                s.group.end,
                &mut s.original,
                s.group.live,
            )?;
            let data = if packed {
                pack::wrap(
                    s.group.series_id,
                    s.group.start,
                    s.group.end,
                    &s.original,
                    &s.addresses,
                )?
            } else {
                s.original
            };
            tx.execute(
                "UPDATE groups SET directory=? WHERE series_id=? AND start_ts=?",
                params![data, s.group.series_id, s.group.start],
            )?;
        }
    }
    // A zero-reference pack is deleted in the same transaction as publication.
    for (id, (refs, bytes)) in deltas {
        let old: i64 = tx.query_row("SELECT refs FROM payload_packs WHERE id=?", [id], |r| {
            r.get(0)
        })?;
        if old == refs {
            tx.execute("DELETE FROM payload_packs WHERE id=?", [id])?;
        } else if old > refs {
            tx.execute(
                "UPDATE payload_packs SET refs=refs-?,live_bytes=live_bytes-? WHERE id=?",
                params![refs, bytes, id],
            )?;
        } else {
            return Err("pack refcount underflow".into());
        }
    }
    tx.commit()?;
    let maintenance_ns = before.elapsed().as_nanos();
    check_refcounts(db)?;
    Ok(json!({"expired_blocks":expired,"maintenance_ns":maintenance_ns}))
}

fn check_refcounts(db: &Connection) -> Result<()> {
    if db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name='payload_packs'",
        [],
        |r| r.get::<_, i64>(0),
    )? == 0
    {
        return Ok(());
    }
    let mut expected: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    for s in all_groups(db)? {
        let mut external = 0;
        for (slot, b) in s.group.blocks.iter().enumerate() {
            if s.group.is_external(slot) {
                let a = s.addresses[external];
                external += 1;
                if s.group.is_live(slot) {
                    let entry = expected.entry(a.pack).or_default();
                    entry.0 += 1;
                    entry.1 += b.body_bytes as i64;
                }
            }
        }
    }
    let actual: Vec<(i64, i64, i64)> = db
        .prepare("SELECT id,refs,live_bytes FROM payload_packs ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if actual.len() != expected.len() {
        return Err("unreferenced pack cleanup failed".into());
    }
    for (id, refs, bytes) in actual {
        if expected.get(&id) != Some(&(refs, bytes)) {
            return Err("pack refcount/live-byte census differs".into());
        }
    }
    Ok(())
}

pub fn lifecycle(input: &Path, out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    let src = connect(input, true)?;
    let groups = all_groups(&src)?;
    let low = groups.iter().map(|s| s.group.start).min().ok_or("empty")?;
    let high = groups.iter().map(|s| s.group.end).max().unwrap();
    drop(src);
    for fraction in [25, 50, 75, 100] {
        let path = out.join(format!("expire{fraction}.db"));
        if path.exists() {
            return Err("lifecycle output exists".into());
        }
        fs::copy(input, &path)?;
        let cutoff = if fraction == 100 {
            high.saturating_add(1)
        } else {
            low + (high - low) * fraction / 100
        };
        let expected = logical(&connect(input, true)?, "range", cutoff)?.0;
        let mut db = connect(&path, false)?;
        db.execute_batch("PRAGMA wal_autocheckpoint=0")?;
        let maintenance = expire(&mut db, cutoff, false)?;
        let actual = logical(&db, "range", cutoff)?.0;
        if expected != actual {
            return Err("retention clipped logical samples differ".into());
        }
        let wal = fs::metadata(format!("{}-wal", path.display()))
            .map(|m| m.len())
            .unwrap_or(0);
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        drop(db);
        println!(
            "{}",
            json!({"fraction":fraction,"cutoff":cutoff,"maintenance":maintenance,"wal_before_checkpoint_bytes":wal,"retained_samples":actual.values().map(|v|v.0).sum::<usize>(),"inspect":inspect(&path)?})
        );
    }
    let path = out.join("alternate-series.db");
    if path.exists() {
        return Err("lifecycle output exists".into());
    }
    fs::copy(input, &path)?;
    let mut db = connect(&path, false)?;
    db.execute_batch("PRAGMA wal_autocheckpoint=0")?;
    let maintenance = expire(&mut db, i64::MAX, true)?;
    let wal = fs::metadata(format!("{}-wal", path.display()))
        .map(|m| m.len())
        .unwrap_or(0);
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    drop(db);
    println!(
        "{}",
        json!({"scenario":"alternate-series","maintenance":maintenance,"wal_before_checkpoint_bytes":wal,"inspect":inspect(&path)?})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merge_keeps_immutable_addresses_and_selected_crc() -> Result<()> {
        let points: Vec<_> = (0..960)
            .map(|i| Sample {
                at: i,
                value: (i * 17 % 997) as f64 / 10.0,
            })
            .collect();
        let mut group = codec::prepare_group(8, &points, 0, -2, 100000)?;
        group.clock_id = 1;
        for (i, b) in group.blocks.iter_mut().enumerate() {
            b.payload = i as i64 + 1;
        }
        let bodies: Vec<_> = group
            .blocks
            .iter()
            .map(|b| (b.payload, b.body.clone()))
            .collect();
        let (packs, map) = pack::pack_bodies(&bodies, "count4")?;
        let before = format!("{:x}", Sha256::digest(&packs[0].1));
        let refs: Vec<_> = group.blocks.iter().map(|b| map[&b.payload]).collect();
        let mut halves = Vec::new();
        for (i, part) in group.blocks.chunks(2).enumerate() {
            let mut half = group.clone();
            half.blocks = part.to_vec();
            half.start = part[0].head.start;
            half.end = part.last().unwrap().head.end;
            half.clock_id = i as i64 + 2;
            half.live = 3;
            half.allocation = 3;
            half.clock_body = codec::encode_clock_group(&half);
            let directory = codec::write_directory(&half)?;
            let addresses = part.iter().map(|b| map[&b.payload]).collect::<Vec<_>>();
            halves.push((
                half.start,
                pack::wrap(8, half.start, half.end, &directory, &addresses)?,
            ));
        }
        let directory = codec::write_directory(&group)?;
        let merged = pack::wrap(8, group.start, group.end, &directory, &refs)?;
        let mut publication = Connection::open_in_memory()?;
        publication.execute_batch("CREATE TABLE directories(start INTEGER PRIMARY KEY,body BLOB); CREATE TABLE immutable_pack(id INTEGER PRIMARY KEY,body BLOB)")?;
        publication.execute("INSERT INTO immutable_pack VALUES(1,?)", [&packs[0].1])?;
        for (start, body) in &halves {
            publication.execute("INSERT INTO directories VALUES(?,?)", params![start, body])?;
        }
        let tx = publication.transaction()?;
        tx.execute("DELETE FROM directories", [])?;
        tx.execute(
            "INSERT INTO directories VALUES(?,?)",
            params![group.start, &merged],
        )?;
        tx.commit()?;
        let untouched: Vec<u8> =
            publication.query_row("SELECT body FROM immutable_pack WHERE id=1", [], |r| {
                r.get(0)
            })?;
        assert_eq!(before, format!("{:x}", Sha256::digest(&untouched)));
        let (restored, addresses) = pack::unwrap(8, group.start, group.end, &merged)?;
        assert_eq!(addresses, refs);
        let clocks = codec::decode_clock_group(&group.clock_body)?;
        let mut decoded = codec::read_directory(8, group.start, group.end, 1, &restored, &clocks)?;
        for (b, a) in decoded.blocks.iter_mut().zip(&addresses) {
            b.body = packs[0].1[a.offset..a.offset + b.body_bytes].to_vec();
            pack::check_body(*a, b.body_bytes, packs[0].1.len(), &b.body)?;
            let decoded = codec::decode_block(b)?;
            assert_eq!(decoded.len(), 240);
        }
        assert_eq!(before, format!("{:x}", Sha256::digest(&packs[0].1)));
        // Proven selected-body mutant: corruption retaining the old trailer is
        // rejected by the actual production-compatible decoder checksum.
        let mut mutant = decoded.blocks[0].clone();
        mutant.body[3] ^= 1;
        assert!(codec::decode_block(&mutant).is_err());
        Ok(())
    }
    #[test]
    fn blob_snapshot_ends_before_owned_body_is_decoded() -> Result<()> {
        let path =
            std::env::temp_dir().join(format!("tinystore-pack-lifetime-{}.db", std::process::id()));
        if path.exists() {
            fs::remove_file(&path)?;
        }
        let writer = connect(&path, false)?;
        writer.execute_batch("CREATE TABLE payload_packs(id INTEGER PRIMARY KEY,body BLOB)")?;
        let points: Vec<_> = (0..240)
            .map(|i| Sample {
                at: i,
                value: (i % 19) as f64 / 10.0,
            })
            .collect();
        let mut group = codec::prepare_group(1, &points, 0, -2, 10000)?;
        let mut block = group.blocks.remove(0);
        let (packs, refs) = pack::pack_bodies(&[(1, block.body.clone())], "count2")?;
        writer.execute("INSERT INTO payload_packs VALUES(1,?)", [&packs[0].1])?;
        let reader = connect(&path, true)?;
        reader.execute_batch("BEGIN")?;
        let handle = reader.blob_open(MAIN_DB, "payload_packs", "body", 1, true)?;
        let a = refs[&1];
        let mut bytes = vec![0; block.body_bytes];
        handle.read_at_exact(&mut bytes, a.offset)?;
        writer.execute("DELETE FROM payload_packs WHERE id=1", [])?;
        let mut repeat = vec![0; block.body_bytes];
        handle.read_at_exact(&mut repeat, a.offset)?;
        assert_eq!(bytes, repeat);
        handle.close()?;
        reader.execute_batch("COMMIT")?;
        assert!(reader.is_autocommit());
        block.body = bytes;
        let decoded = codec::decode_block(&block)?;
        for (a, b) in decoded.iter().zip(&points) {
            assert_eq!((a.at, a.value.to_bits()), (b.at, b.value.to_bits()));
        }
        assert!(
            reader
                .blob_open(MAIN_DB, "payload_packs", "body", 1, true)
                .is_err()
        );
        drop(reader);
        drop(writer);
        fs::remove_file(&path)?;
        Ok(())
    }
}
