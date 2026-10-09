//! Layout-only research. Value bodies, residual clocks and exact summaries are immutable.
#![allow(dead_code, unused_imports)]
mod codec;
mod engine;
mod exact;
mod profile;
mod tuning;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use codec::{Block, Group};
use engine::Sample;
use num_bigint::BigInt;
use num_traits::Zero;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone)]
struct Saved {
    group: Group,
    directory: Vec<u8>,
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hash_samples(points: &[Sample]) -> String {
    let mut h = Sha256::new();
    for p in points {
        h.update(p.at.to_le_bytes());
        h.update(p.value.to_bits().to_le_bytes())
    }
    hex(&h.finalize())
}
fn configure(c: &Connection, write: bool) -> Result<()> {
    c.execute_batch("PRAGMA foreign_keys=1;PRAGMA synchronous=FULL;PRAGMA fullfsync=1;PRAGMA checkpoint_fullfsync=1;PRAGMA cache_size=-1024;PRAGMA busy_timeout=5000;PRAGMA trusted_schema=0;PRAGMA mmap_size=0;")?;
    if write {
        c.execute_batch("PRAGMA journal_mode=WAL;")?;
    } else {
        c.execute_batch("PRAGMA query_only=1;")?;
    }
    c.set_prepared_statement_cache_capacity(32);
    let version: String = c.query_row("select sqlite_version()", [], |r| r.get(0))?;
    if version != "3.53.4" {
        return Err(format!("expected SQLite3.53.4, got {version}").into());
    }
    Ok(())
}
fn open(path: &Path, write: bool) -> Result<Connection> {
    let c = Connection::open_with_flags(
        path,
        if write {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        },
    )?;
    configure(&c, write)?;
    Ok(c)
}
fn inline_limit(c: &Connection) -> usize {
    let d: Vec<u8> = c
        .query_row("select directory from groups limit 1", [], |r| r.get(0))
        .unwrap_or_default();
    if d.first() == Some(&5) && d.len() >= 20 {
        u16::from_le_bytes(d[18..20].try_into().unwrap()) as usize
    } else {
        16
    }
}
fn load(
    c: &Connection,
    id: i64,
    start: i64,
    end: i64,
    clock_id: i64,
    directory: Vec<u8>,
    _inline: usize,
    bodies: bool,
) -> Result<Saved> {
    let inline = if directory.first() == Some(&5) && directory.len() >= 20 {
        u16::from_le_bytes(directory[18..20].try_into().unwrap()) as usize
    } else {
        16
    };
    let clock: Vec<u8> = c
        .prepare_cached("select body from clocks where id=?")?
        .query_row([clock_id], |r| r.get(0))?;
    let templates = codec::decode_clock_group(&clock)?;
    let mut group =
        codec::directory::read_with(id, start, end, clock_id, &directory, &templates, inline)?;
    group.clock_body = clock;
    if bodies {
        for slot in 0..group.blocks.len() {
            if group.is_external(slot) && group.is_live(slot) {
                group.blocks[slot].body = c
                    .prepare_cached("select body from payloads where id=?")?
                    .query_row([group.blocks[slot].payload], |r| r.get(0))?;
                if group.blocks[slot].body.len() != group.blocks[slot].body_bytes {
                    return Err("payload length".into());
                }
            }
        }
    }
    Ok(Saved { group, directory })
}
fn all(c: &Connection, bodies: bool) -> Result<Vec<Saved>> {
    let mut st=c.prepare("select series_id,start_ts,end_ts,clock_id,directory from groups order by series_id,start_ts")?;
    let rows = st.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, start, end, clock, directory) = row?;
        out.push(load(c, id, start, end, clock, directory, 16, bodies)?)
    }
    Ok(out)
}
fn selected(c: &Connection, id: i64, from: i64, to: i64, bodies: bool) -> Result<Vec<Saved>> {
    let mut st=c.prepare_cached("select start_ts,end_ts,clock_id,directory from groups where series_id=? and start_ts<? and end_ts>=? order by start_ts")?;
    let rows = st.query_map(params![id, to, from], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (start, end, clock, directory) = row?;
        out.push(load(c, id, start, end, clock, directory, 16, bodies)?)
    }
    Ok(out)
}
type HeadInput = (i64, Option<i64>, Option<i64>, Option<Vec<u8>>);
fn fetch_head(c: &Connection, id: i64, from: i64, to: i64) -> Result<HeadInput> {
    Ok(c
        .prepare_cached(
            "select head_count,head_start,head_end,tail from series_state where series_id=? and head_end>=? and head_start<?",
        )?
        .query_row(params![id,from,to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?.unwrap_or((0,None,None,None)))
}
fn decode_head_input(id: i64, row: HeadInput) -> Result<Vec<Sample>> {
    Ok(codec::decode_head(
        id,
        row.0 as usize,
        row.1.unwrap_or(0),
        row.2.unwrap_or(0),
        &row.3.unwrap_or_default(),
        1 << 20,
        16 << 20,
    )?)
}
fn read(c: &Connection, id: i64, from: i64, to: i64) -> Result<Vec<Sample>> {
    c.execute_batch("BEGIN DEFERRED")?;
    let mut groups = selected(c, id, from, to, false)?;
    for saved in &mut groups {
        for slot in 0..saved.group.blocks.len() {
            if saved.group.is_live(slot) && saved.group.is_external(slot) {
                let b = &mut saved.group.blocks[slot];
                if b.head.start < to && b.head.end >= from {
                    b.body = c
                        .prepare_cached("select body from payloads where id=?")?
                        .query_row([b.payload], |r| r.get(0))?;
                    if b.body.len() != b.body_bytes {
                        return Err("selected payload length".into());
                    }
                }
            }
        }
    }
    let h = fetch_head(c, id, from, to)?;
    c.execute_batch("COMMIT")?;
    let h = decode_head_input(id, h)?;
    let mut out = Vec::new();
    for saved in groups {
        for (slot, b) in saved.group.blocks.iter().enumerate() {
            if saved.group.is_live(slot) && b.head.start < to && b.head.end >= from {
                out.extend(
                    codec::decode_block(b)?
                        .into_iter()
                        .filter(|p| p.at >= from && p.at < to),
                )
            }
        }
    }
    out.extend(h.into_iter().filter(|p| p.at >= from && p.at < to));
    Ok(out)
}
fn verify(c: &Connection, m: &Value) -> Result<Value> {
    let mut total = 0usize;
    for s in m["series"].as_array().ok_or("manifest series")? {
        let p = read(
            c,
            s["id"].as_i64().unwrap(),
            s["from"].as_i64().unwrap(),
            s["to"].as_i64().unwrap(),
        )?;
        if p.len() != s["count"].as_u64().unwrap() as usize
            || hash_samples(&p) != s["sha256"].as_str().unwrap()
        {
            return Err(format!("sample hash mismatch series{}", s["id"]).into());
        }
        total += p.len();
    }
    Ok(json!({"verified_samples":total,"series":m["series"].as_array().unwrap().len()}))
}
fn block_hash(saved: &[Saved]) -> Result<String> {
    let mut h = Sha256::new();
    for s in saved {
        for (slot, b) in s.group.blocks.iter().enumerate() {
            if !s.group.is_live(slot) {
                return Err("layout rebuild requires a fresh all-live fixture".into());
            }
            h.update(s.group.series_id.to_le_bytes());
            h.update(b.head.start.to_le_bytes());
            h.update(b.head.end.to_le_bytes());
            h.update((b.head.count as u64).to_le_bytes());
            h.update(b.head.first.to_bits().to_le_bytes());
            for bytes in [
                &b.clock,
                &b.body,
                &b.summary.exact_sum,
                &b.summary.exact_increase,
            ] {
                h.update((bytes.len() as u64).to_le_bytes());
                h.update(bytes);
            }
            for value in [
                b.summary.last,
                b.summary.min,
                b.summary.max,
                b.summary.sum,
                b.summary.increase,
            ] {
                h.update(value.to_bits().to_le_bytes());
            }
            h.update(b.summary.resets.to_le_bytes());
            h.update([u8::from(b.summary.valid)]);
        }
    }
    Ok(hex(&h.finalize()))
}
fn retention_visible(c: &Connection, m: &Value, cutoff: i64) -> Result<Vec<(i64, String, usize)>> {
    let mut out = Vec::new();
    for s in m["series"].as_array().ok_or("manifest series")? {
        let id = s["id"].as_i64().unwrap();
        let p = read(
            c,
            id,
            s["from"].as_i64().unwrap().max(cutoff),
            s["to"].as_i64().unwrap(),
        )?;
        out.push((id, hash_samples(&p), p.len()));
    }
    Ok(out)
}
fn census(c: &Connection, path: &Path) -> Result<Value> {
    let groups = all(c, true)?;
    let mut distribution = BTreeMap::<usize, usize>::new();
    let mut directories = BTreeMap::<usize, usize>::new();
    let mut group_counts = BTreeMap::<usize, usize>::new();
    let mut parts = [0usize; 5];
    let (mut value, mut clock_residual, mut samples, mut inline_values, mut live_blocks) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for s in &groups {
        *directories.entry(s.directory.len()).or_default() += 1;
        *group_counts.entry(s.group.blocks.len()).or_default() += 1;
        let p = codec::directory::parts(&s.group);
        for i in 0..5 {
            parts[i] += p[i]
        }
        for (slot, b) in s.group.blocks.iter().enumerate() {
            if s.group.is_live(slot) {
                live_blocks += 1;
                samples += b.head.count;
                value += b.body_bytes;
                clock_residual += b.clock.len();
                *distribution.entry(b.body_bytes).or_default() += 1;
                if !s.group.is_external(slot) {
                    inline_values += b.body_bytes
                }
            }
        }
    }
    let objects: Vec<Value> = Vec::new();
    let mut settings = serde_json::Map::new();
    for name in [
        "page_size",
        "page_count",
        "freelist_count",
        "synchronous",
        "fullfsync",
        "checkpoint_fullfsync",
        "cache_size",
        "wal_autocheckpoint",
        "mmap_size",
        "trusted_schema",
    ] {
        let n: i64 = c.query_row(&format!("pragma {name}"), [], |r| r.get(0))?;
        settings.insert(name.into(), json!(n));
    }
    settings.insert(
        "sqlite_source_id".into(),
        json!(c.query_row::<String, _, _>("select sqlite_source_id()", [], |r| r.get(0))?),
    );
    let mut tables = serde_json::Map::new();
    for name in [
        "series",
        "series_state",
        "label_values",
        "postings",
        "groups",
        "clocks",
        "payloads",
    ] {
        tables.insert(
            name.into(),
            json!(
                c.query_row::<i64, _, _>(&format!("select count(*) from {name}"), [], |r| r
                    .get(0))?
            ),
        );
    }
    let mut blobs = serde_json::Map::new();
    for (name, sql) in [
        (
            "directory_stored",
            "select coalesce(sum(length(directory)),0) from groups",
        ),
        (
            "clock_stored",
            "select coalesce(sum(length(body)),0) from clocks",
        ),
        (
            "payload_stored",
            "select coalesce(sum(length(body)),0) from payloads",
        ),
        (
            "head_stored",
            "select coalesce(sum(length(tail)),0) from series_state",
        ),
        (
            "head_samples",
            "select coalesce(sum(head_count),0) from series_state",
        ),
    ] {
        blobs.insert(
            name.into(),
            json!(c.query_row::<i64, _, _>(sql, [], |r| r.get(0))?),
        );
    }
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    let shm = PathBuf::from(format!("{}-shm", path.display()));
    Ok(
        json!({"file_bytes":fs::metadata(path)?.len(),"wal_bytes":fs::metadata(wal).map(|x|x.len()).unwrap_or(0),"shm_bytes":fs::metadata(shm).map(|x|x.len()).unwrap_or(0),"settings":settings,"tables":tables,"blobs":blobs,"sealed_samples":samples,"live_blocks":live_blocks,"value_bytes_total":value,"inline_value_bytes":inline_values,"block_clock_residual_bytes":clock_residual,"expanded_directory_parts":{"first":parts[0],"summary_scalars_flags_resets":parts[1],"exact_summary":parts[2],"value_length":parts[3],"address_or_inline":parts[4]},"body_size_histogram":distribution,"directory_size_histogram":directories,"group_slots_histogram":group_counts,"objects":objects}),
    )
}
fn export(c: &Connection, dir: &Path) -> Result<Value> {
    fs::create_dir_all(dir)?;
    let mut blocks = BufWriter::new(fs::File::create(dir.join("blocks.jsonl"))?);
    let mut gs = BufWriter::new(fs::File::create(dir.join("groups.jsonl"))?);
    let groups = all(c, true)?;
    let (mut count, mut ordinal) = (0usize, 0usize);
    for saved in groups {
        let g = &saved.group;
        let kind: String =
            c.query_row("select kind from series where id=?", [g.series_id], |r| {
                r.get(0)
            })?;
        writeln!(
            gs,
            "{}",
            json!({"series_id":g.series_id,"start":g.start,"end":g.end,"clock_id":g.clock_id,"live":g.live,"allocation":g.allocation,"directory_version":saved.directory[0],"directory_b64":B64.encode(&saved.directory),"clock_body_b64":B64.encode(&g.clock_body)})
        )?;
        for (slot, b) in g.blocks.iter().enumerate() {
            if !g.is_live(slot) {
                continue;
            }
            let p = codec::decode_block(b)?;
            writeln!(
                blocks,
                "{}",
                json!({"block_id":format!("{}:{}:{}",g.series_id,b.head.start,ordinal),"ordinal":ordinal,"series_id":g.series_id,"kind":kind,"original_group_start":g.start,"original_group_end":g.end,"original_clock_id":g.clock_id,"original_group_slot":slot,"original_payload_id":b.payload,"original_payload_bytes":b.body_bytes,"directory_version":saved.directory[0],"live":true,"head":{"start":b.head.start,"end":b.head.end,"count":b.head.count,"first_bits":format!("{:016x}",b.head.first.to_bits())},"clock_b64":B64.encode(&b.clock),"value_body_b64":B64.encode(&b.body),"sample_sha256":hash_samples(&p),"crc32_ieee":if b.body.is_empty(){None}else{Some(format!("{:08x}",u32::from_le_bytes(b.body[b.body.len()-4..].try_into().unwrap())))},"summary":{"last_bits":format!("{:016x}",b.summary.last.to_bits()),"min_bits":format!("{:016x}",b.summary.min.to_bits()),"max_bits":format!("{:016x}",b.summary.max.to_bits()),"sum_bits":format!("{:016x}",b.summary.sum.to_bits()),"increase_bits":format!("{:016x}",b.summary.increase.to_bits()),"resets":b.summary.resets,"valid":b.summary.valid,"exact_sum_b64":B64.encode(&b.summary.exact_sum),"exact_increase_b64":B64.encode(&b.summary.exact_increase)}})
            )?;
            count += 1;
            ordinal += 1;
        }
    }
    blocks.flush()?;
    gs.flush()?;
    Ok(json!({"exported_blocks":count,"format":"tinystore-storage-block-v1"}))
}
fn fresh(
    source: &Path,
    dest: &Path,
    page: usize,
    rowid: bool,
    cap: usize,
    inline: usize,
    reencode: bool,
) -> Result<Value> {
    if dest.exists() {
        return Err("refuse existing variant".into());
    }
    if ![8, 16, 32].contains(&cap) || ![1024, 2048, 4096].contains(&page) || inline > 128 {
        return Err("unsupported experimental layout".into());
    }
    fs::create_dir_all(dest.parent().unwrap())?;
    let input = open(source, false)?;
    let saved = all(&input, true)?;
    let immutable = block_hash(&saved)?;
    let addresses: BTreeMap<(i64, i64), i64> = saved
        .iter()
        .flat_map(|s| {
            s.group
                .blocks
                .iter()
                .map(|b| ((s.group.series_id, b.head.start), b.payload))
        })
        .collect();
    let publication_start = Instant::now();
    let c = Connection::open(dest)?;
    c.execute_batch(&format!(
        "PRAGMA page_size={page};PRAGMA application_id=1414350164;"
    ))?;
    configure(&c, true)?;
    c.execute("attach database ? as src", [source.to_str().unwrap()])?;
    let schema: Vec<(String, String, String)> = input
        .prepare("select type,name,sql from sqlite_schema where sql is not null order by rowid")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?;
    c.execute_batch("BEGIN IMMEDIATE")?;
    for (kind, name, sql) in &schema {
        if kind == "table" {
            let ddl = if name == "groups" && rowid {
                sql.replace(", without rowid", "")
                    .replace(", WITHOUT ROWID", "")
            } else {
                sql.clone()
            };
            c.execute_batch(&ddl)?;
        }
    }
    for (kind, name, _) in &schema {
        if kind == "table" && !matches!(name.as_str(), "groups" | "clocks" | "payloads") {
            c.execute_batch(&format!("insert into main.{name} select * from src.{name}"))?;
        }
    }
    if !reencode && cap == 32 && inline == 16 {
        for name in ["groups", "clocks", "payloads"] {
            c.execute_batch(&format!("insert into main.{name} select * from src.{name}"))?;
        }
    } else {
        let mut next_payload =
            input.query_row::<i64, _, _>("select next_payload_id from store_state", [], |r| {
                r.get(0)
            })?;
        let mut next_clock = 1i64;
        let mut clocks = BTreeMap::<Vec<u8>, i64>::new();
        for original in saved {
            for part in original.group.blocks.chunks(cap) {
                let mut g = Group {
                    series_id: original.group.series_id,
                    start: part[0].head.start,
                    end: part.last().unwrap().head.end,
                    live: codec::slots_mask(part.len()),
                    blocks: part.to_vec(),
                    ..Default::default()
                };
                for (slot, b) in g.blocks.iter_mut().enumerate() {
                    if b.body_bytes > inline {
                        g.allocation |= 1 << slot;
                        if b.payload == 0 {
                            b.payload = next_payload;
                            next_payload += 1;
                        }
                        c.execute(
                            "insert into payloads values(?,?)",
                            params![b.payload, &b.body],
                        )?;
                    } else {
                        b.payload = 0;
                    }
                }
                g.clock_body = codec::encode_clock_group(&g);
                let digest = Sha256::digest(&g.clock_body).to_vec();
                g.clock_id = if let Some(id) = clocks.get(&digest) {
                    c.execute("update clocks set refs=refs+1 where id=?", [id])?;
                    *id
                } else {
                    let id = next_clock;
                    next_clock += 1;
                    c.execute(
                        "insert into clocks values(?,?,1,?)",
                        params![id, &digest, &g.clock_body],
                    )?;
                    clocks.insert(digest, id);
                    id
                };
                let directory = codec::directory::write_with(&g, inline)?;
                c.execute(
                    "insert into groups values(?,?,?,?,?)",
                    params![g.series_id, g.start, g.end, &directory, g.clock_id],
                )?;
            }
        }
        c.execute("update store_state set next_payload_id=?", [next_payload])?;
    }
    for (kind, _, sql) in &schema {
        if kind == "index" {
            c.execute_batch(sql)?;
        }
    }
    c.execute_batch("COMMIT;PRAGMA wal_checkpoint(TRUNCATE);")?;
    let publication_ns = publication_start.elapsed().as_nanos();
    for name in [
        "_tinystore_migrations",
        "series",
        "series_state",
        "label_values",
        "postings",
    ] {
        let unequal: i64 = c.query_row(
            &format!(
                "select count(*) from (select * from main.{name} except select * from src.{name})"
            ),
            [],
            |r| r.get(0),
        )?;
        let unequal_back: i64 = c.query_row(
            &format!(
                "select count(*) from (select * from src.{name} except select * from main.{name})"
            ),
            [],
            |r| r.get(0),
        )?;
        if unequal != 0 || unequal_back != 0 {
            return Err(format!("immutable registry/head/state rows differ: {name}").into());
        }
    }
    c.execute_batch("DETACH DATABASE src;")?;
    let after = all(&c, true)?;
    if block_hash(&after)? != immutable {
        return Err("immutable microblock body/clock/summary mismatch".into());
    }
    let mut stable = 0usize;
    for s in &after {
        for b in &s.group.blocks {
            let old = addresses[&(s.group.series_id, b.head.start)];
            if old > 0 && b.payload > 0 {
                if old != b.payload {
                    return Err("payload address relocated".into());
                }
                stable += 1;
            }
        }
    }
    let mut result = census(&c, dest)?;
    result["immutable_block_sha256"] = json!(immutable);
    result["stable_external_payload_addresses"] = json!(stable);
    result["publication_ns"] = json!(publication_ns);
    result["registry_head_state_rows_identical"] = json!(true);
    drop(c);
    Ok(result)
}
fn aggregate(c: &Connection, id: i64, from: i64, to: i64, force_raw: bool) -> Result<String> {
    c.execute_batch("BEGIN DEFERRED")?;
    let groups = selected(c, id, from, to, false)?;
    let h = fetch_head(c, id, from, to)?;
    let (mut sum, mut count) = (BigInt::zero(), 0usize);
    let mut raw = Vec::new();
    for saved in groups {
        for (slot, b) in saved.group.blocks.iter().enumerate() {
            if !saved.group.is_live(slot) || b.head.start >= to || b.head.end < from {
                continue;
            }
            if !force_raw
                && b.head.start >= from
                && b.head.end < to
                && !b.summary.exact_sum.is_empty()
            {
                sum += codec::read_exact_value(&b.summary.exact_sum)?;
                count += b.head.count;
            } else {
                let mut b = b.clone();
                if saved.group.is_external(slot) {
                    b.body = c
                        .prepare_cached("select body from payloads where id=?")?
                        .query_row([b.payload], |r| r.get(0))?;
                }
                raw.push(b);
            }
        }
    }
    c.execute_batch("COMMIT")?;
    let h = decode_head_input(id, h)?;
    for b in raw {
        for p in codec::decode_block(&b)? {
            if p.at >= from && p.at < to {
                sum += codec::finite_units(p.value)?;
                count += 1;
            }
        }
    }
    for p in h {
        if p.at >= from && p.at < to {
            sum += codec::finite_units(p.value)?;
            count += 1;
        }
    }
    Ok(format!("{count}:{}", hex(&sum.to_signed_bytes_le())))
}
fn bench(c: &Connection, m: &Value, case: &str, iterations: usize) -> Result<Value> {
    let series = m["series"].as_array().ok_or("manifest series")?;
    let operation = |i: usize| -> Result<String> {
        let s = &series[(i * 17 + 3) % series.len()];
        let (id, from, to) = (
            s["id"].as_i64().unwrap(),
            s["from"].as_i64().unwrap(),
            s["to"].as_i64().unwrap(),
        );
        let span = to - from;
        match case {
            "point" => {
                let at = from;
                Ok(hash_samples(&read(c, id, at, at + 1)?))
            }
            "range" => Ok(hash_samples(&read(
                c,
                id,
                from + span / 3,
                from + span / 3 + span / 16,
            )?)),
            "whole_summary" => aggregate(c, id, from, to, false),
            "cut_summary" => aggregate(c, id, from + span / 3, from + span * 2 / 3, false),
            _ => Err("unknown trace".into()),
        }
    };
    let start = Instant::now();
    let mut h = Sha256::new();
    for i in 0..iterations {
        h.update(operation(i)?);
    }
    Ok(
        json!({"case":case,"iterations":iterations,"elapsed_ns":start.elapsed().as_nanos(),"result_sha256":hex(&h.finalize())}),
    )
}
fn retain(c: &Connection, cutoff: i64) -> Result<Value> {
    let start = Instant::now();
    let inline = inline_limit(c);
    let groups = all(c, false)?;
    let mut expired = 0usize;
    let mut changed = 0;
    c.execute_batch("BEGIN IMMEDIATE")?;
    for saved in groups {
        let mut g = saved.group;
        let mut dirty = false;
        for slot in 0..g.blocks.len() {
            if g.is_live(slot) && g.blocks[slot].head.end < cutoff {
                if g.is_external(slot) {
                    c.execute("delete from payloads where id=?", [g.blocks[slot].payload])?;
                }
                g.live &= !(1 << slot);
                expired += g.blocks[slot].head.count;
                dirty = true;
            }
        }
        if !dirty {
            continue;
        }
        changed += 1;
        if g.live == 0 {
            c.execute(
                "delete from groups where series_id=? and start_ts=?",
                params![g.series_id, g.start],
            )?;
            c.execute("update clocks set refs=refs-1 where id=?", [g.clock_id])?;
        } else {
            let directory = codec::directory::write_with(&g, inline)?;
            c.execute(
                "update groups set directory=? where series_id=? and start_ts=?",
                params![directory, g.series_id, g.start],
            )?;
        }
    }
    c.execute_batch("delete from clocks where refs=0;COMMIT;")?;
    Ok(
        json!({"expired_samples":expired,"changed_groups":changed,"cutoff":cutoff,"elapsed_ns":start.elapsed().as_nanos()}),
    )
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |key: &str, default: &str| -> String {
        args.windows(2)
            .find(|x| x[0] == key)
            .map(|x| x[1].clone())
            .unwrap_or(default.into())
    };
    let mode = arg("--mode", "census");
    let path = PathBuf::from(arg("--db", ""));
    let manifest_path = PathBuf::from(arg(
        "--manifest",
        path.with_file_name("manifest.json").to_str().unwrap(),
    ));
    let start = Instant::now();
    let result = match mode.as_str() {
        "rewrite" => fresh(
            &PathBuf::from(arg("--source", "")),
            &path,
            arg("--page", "4096").parse()?,
            arg("--rowid", "0") == "1",
            arg("--cap", "32").parse()?,
            arg("--inline", "16").parse()?,
            arg("--reencode", "0") == "1",
        )?,
        "retain" => {
            let c = open(&path, true)?;
            let cutoff: i64 = arg("--cutoff", "0").parse()?;
            let m: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
            let before = retention_visible(&c, &m, cutoff)?;
            let r = retain(&c, cutoff)?;
            let after = retention_visible(&c, &m, cutoff)?;
            if before != after {
                return Err("retention boundary/liveness hash mismatch".into());
            }
            for s in all(&c, false)? {
                for (slot, b) in s.group.blocks.iter().enumerate() {
                    if s.group.is_live(slot) && b.head.end < cutoff {
                        return Err("expired block still live".into());
                    }
                }
            }
            let wal = fs::metadata(format!("{}-wal", path.display()))
                .map(|x| x.len())
                .unwrap_or(0);
            c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
            json!({"retention":r,"visible_hash_verified":true,"wal_before_checkpoint":wal,"physical":census(&c,&path)?})
        }
        "vacuum" => {
            let c = open(&path, true)?;
            c.execute_batch("VACUUM;PRAGMA wal_checkpoint(TRUNCATE)")?;
            census(&c, &path)?
        }
        _ => {
            let c = open(&path, false)?;
            match mode.as_str() {
                "census" => census(&c, &path)?,
                "export" => export(&c, &PathBuf::from(arg("--out", "")))?,
                "verify" => verify(&c, &serde_json::from_slice(&fs::read(manifest_path)?)?)?,
                "bench" => bench(
                    &c,
                    &serde_json::from_slice(&fs::read(manifest_path)?)?,
                    &arg("--case", "point"),
                    arg("--iterations", "100").parse()?,
                )?,
                "check_summary" => {
                    let m: Value = serde_json::from_slice(&fs::read(manifest_path)?)?;
                    for s in m["series"].as_array().unwrap().iter().take(16) {
                        let (id, from, to) = (
                            s["id"].as_i64().unwrap(),
                            s["from"].as_i64().unwrap(),
                            s["to"].as_i64().unwrap(),
                        );
                        if aggregate(&c, id, from, to, false)? != aggregate(&c, id, from, to, true)?
                            || aggregate(
                                &c,
                                id,
                                from + (to - from) / 3,
                                from + (to - from) * 2 / 3,
                                false,
                            )? != aggregate(
                                &c,
                                id,
                                from + (to - from) / 3,
                                from + (to - from) * 2 / 3,
                                true,
                            )?
                        {
                            return Err("summary/raw mismatch".into());
                        }
                    }
                    json!({"summary_matches_raw":true})
                }
                _ => return Err("unknown mode".into()),
            }
        }
    };
    println!(
        "{}",
        json!({"mode":mode,"elapsed_ns":start.elapsed().as_nanos(),"result":result})
    );
    Ok(())
}
