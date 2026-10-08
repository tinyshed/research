use crate::{Page, Query, Record, Result, codec};
use rusqlite::{Connection, params};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
pub struct Engine {
    pub writer: Connection,
    pub reader: Connection,
    waiting: HashMap<String, i64>,
    streams: HashMap<String, i64>,
}
#[derive(Clone)]
struct Source {
    id: i64,
    segment: i64,
    stream: String,
    first: i64,
    last: i64,
    count: usize,
    size: usize,
    body: Vec<u8>,
    schema: Vec<u8>,
}
impl Engine {
    pub fn open(path: &Path) -> Result<Self> {
        let writer = Connection::open(path)?;
        for pragma in [
            "foreign_keys=1",
            "busy_timeout=5000",
            "synchronous=FULL",
            "fullfsync=1",
            "checkpoint_fullfsync=1",
            "cache_size=-1024",
        ] {
            writer.execute_batch(&format!("pragma {pragma}"))?;
        }
        writer.execute_batch("pragma journal_mode=WAL")?;
        if writer.query_row("pragma page_size", [], |r| r.get::<_, i64>(0))? != 1024 {
            return Err("records pages must be 1024".into());
        };
        if writer.query_row("pragma application_id", [], |r| r.get::<_, i64>(0))? != 0x54524543 {
            return Err("records application id".into());
        };
        writer.set_prepared_statement_cache_capacity(128);
        let reader = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        reader.execute_batch("pragma foreign_keys=1; pragma busy_timeout=5000; pragma synchronous=FULL; pragma fullfsync=1; pragma checkpoint_fullfsync=1; pragma cache_size=-1024; pragma query_only=1")?;
        reader.set_prepared_statement_cache_capacity(128);
        let mut streams = HashMap::new();
        {
            let mut st = reader.prepare("select name,id from streams")?;
            for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
                let (s, i) = r?;
                streams.insert(s, i);
            }
        }
        let mut waiting = HashMap::new();
        {
            let mut st=reader.prepare("select s.name,max(h.last_at) from heads h join streams s on s.id=h.stream where h.late=0 group by h.stream")?;
            for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
                let (s, i) = r?;
                waiting.insert(s, i);
            }
        }
        Ok(Self {
            writer,
            reader,
            waiting,
            streams,
        })
    }
    pub fn append(&mut self, batch: &[Record]) -> Result<usize> {
        let mut input = 0;
        for r in batch {
            if r.stream.is_empty() || r.name.is_empty() {
                return Err("invalid name".into());
            };
            if r.at < crate::NOW - 14 * 24 * 3600 * 1000000000 {
                return Err("too old".into());
            };
            if r.at > crate::NOW + 600000000000 {
                return Err("too new".into());
            };
            if r.context.len() > 128 || r.attrs.len() > 128 || codec::input(r) > codec::MAX_BLOCK {
                return Err("record limit".into());
            };
            for f in r.context.iter().chain(&r.attrs) {
                let _: serde_json::Value =
                    serde_json::from_str(&f.value).map_err(|_| "invalid JSON")?;
            }
            input += codec::input(r)
        }
        if input > 4 << 20 {
            return Err("append limit".into());
        };
        let mut order = Vec::new();
        let mut groups: HashMap<String, Vec<Record>> = HashMap::new();
        for r in batch {
            if !groups.contains_key(&r.stream) {
                order.push(r.stream.clone())
            };
            groups.entry(r.stream.clone()).or_default().push(r.clone())
        }
        let mut rows = Vec::new();
        for name in order {
            let mut newest = self.waiting.get(&name).copied().unwrap_or(i64::MIN);
            let mut on = Vec::new();
            let mut late = Vec::new();
            for r in groups.remove(&name).unwrap() {
                let reference = newest.min(crate::NOW);
                if reference > i64::MIN + 10000000000 && r.at < reference - 10000000000 {
                    late.push(r)
                } else {
                    newest = newest.max(r.at);
                    on.push(r)
                }
            }
            for (late, rs) in [(false, on), (true, late)] {
                let mut start = 0;
                while start < rs.len() {
                    let mut end = start;
                    let mut input = 0;
                    while end < rs.len() && end - start < 1024 {
                        let n = codec::input(&rs[end]);
                        if end > start && input + n > codec::MAX_BLOCK {
                            break;
                        };
                        input += n;
                        end += 1
                    }
                    let rs = &rs[start..end];
                    let body = codec::encode_head(rs)?;
                    rows.push((
                        name.clone(),
                        late,
                        rs.iter().map(|r| r.at).min().unwrap(),
                        rs.iter().map(|r| r.at).max().unwrap(),
                        rs.iter()
                            .filter_map(|r| r.level)
                            .fold(0, |a, l| a | codec::level_bit(l)),
                        rs.len(),
                        input,
                        body,
                    ));
                    start = end
                }
            }
        }
        self.writer.execute_batch("begin immediate")?;
        let result = (|| -> Result<()> {
            for (name, late, first, last, levels, count, input, body) in &rows {
                let stream = if let Some(i) = self.streams.get(name) {
                    *i
                } else {
                    self.writer
                        .prepare_cached("insert into streams(name) values(?)")?
                        .execute([name])?;
                    let i = self.writer.last_insert_rowid();
                    self.streams.insert(name.clone(), i);
                    i
                };
                self.writer.prepare_cached("insert into heads(stream,late,first_at,last_at,levels,count,input,size,written_at,body) values(?,?,?,?,?,?,?,?,?,?)")?.execute(params![stream,late,first,last,levels,*count as i64,*input as i64,body.len() as i64,crate::NOW,body])?;
                self.writer.prepare_cached("insert into head_state(stream,late,count,input,since) values(?,?,?,?,?) on conflict(stream,late) do update set count=count+excluded.count,input=input+excluded.input")?.execute(params![stream,late,*count as i64,*input as i64,crate::NOW])?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.writer.execute_batch("rollback")?;
            return Err(e);
        };
        self.writer.execute_batch("commit")?;
        for (name, late, _, last, _, _, _, _) in rows {
            if !late {
                self.waiting
                    .entry(name)
                    .and_modify(|x| *x = (*x).max(last))
                    .or_insert(last);
            }
        }
        Ok(batch.len())
    }
    pub fn seal(&mut self) -> Result<usize> {
        let mut groups: Vec<(i64, i64, String)> = vec![];
        {
            let mut st=self.reader.prepare_cached("select h.stream,h.late,s.name from head_state h join streams s on s.id=h.stream order by h.stream,h.late")?;
            for r in st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))? {
                groups.push(r?)
            }
        };
        let mut sealed = 0;
        for (stream, late, name) in groups {
            let mut rs = Vec::new();
            let mut heads = Vec::new();
            {
                let mut st = self.reader.prepare_cached(
                    "select id,body from heads where stream=? and late=? order by id",
                )?;
                for r in st.query_map(params![stream, late], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
                })? {
                    let (id, b) = r?;
                    heads.push(id);
                    rs.extend(codec::decode_head(&name, &b)?)
                }
            };
            if rs.len() > 16384 || rs.iter().map(codec::input).sum::<usize>() > 4 << 20 {
                return Err("slice seal supports one bounded segment per head".into());
            };
            let segment = codec::encode_segment(rs)?;
            self.writer.execute_batch("begin immediate")?;
            let result = (|| -> Result<()> {
                let next: i64 = self.writer.query_row(
                    "select coalesce(max(seq),0)+1 from sqlite_sequence where name='blocks'",
                    [],
                    |r| r.get(0),
                )?;
                let input: usize = segment.records.iter().map(codec::input).sum();
                self.writer.prepare_cached("insert into segments(stream,first_at,last_at,count,start,held,input,first_block,last_block,body) values(?,?,?,?,0,?,?,?,?,?)")?.execute(params![stream,segment.records[0].at,segment.records.last().unwrap().at,segment.records.len() as i64,segment.records.len() as i64,input as i64,next,next+segment.blocks.len() as i64-1,segment.row])?;
                let id = self.writer.last_insert_rowid();
                let mut keys = BTreeSet::new();
                for r in &segment.records {
                    keys.insert((0, r.name.clone()));
                    for f in &r.attrs {
                        keys.insert((1, f.key.clone()));
                    }
                    for f in &r.context {
                        keys.insert((2, f.key.clone()));
                    }
                }
                for (kind, key) in keys {
                    self.writer
                        .prepare_cached("insert into segment_keys(segment,kind,key) values(?,?,?)")?
                        .execute(params![id, kind, key])?;
                }
                for (i, b) in segment.blocks.iter().enumerate() {
                    let span = 64 - (b.last as u64).wrapping_sub(b.first as u64).leading_zeros();
                    self.writer.prepare_cached("insert into blocks(id,segment,stream,first_at,last_at,span,levels,count,size,body) values(?,?,?,?,?,?,?,?,?,?)")?.execute(params![next+i as i64,id,stream,b.first,b.last,span,b.levels,b.count as i64,b.body.len() as i64,b.body])?;
                    if !b.traces.is_empty() {
                        self.writer
                            .prepare_cached("insert into block_traces(block,bloom) values(?,?)")?
                            .execute(params![next + i as i64, codec::bloom(&b.traces)])?;
                    }
                }
                for head in heads {
                    self.writer
                        .prepare_cached("delete from heads where id=?")?
                        .execute([head])?;
                }
                self.writer
                    .prepare_cached("delete from head_state where stream=? and late=?")?
                    .execute(params![stream, late])?;
                Ok(())
            })();
            if let Err(e) = result {
                self.writer.execute_batch("rollback")?;
                return Err(e);
            };
            self.writer.execute_batch("commit")?;
            if late == 0 {
                self.waiting.remove(&name);
            }
            sealed += segment.records.len()
        }
        Ok(sealed)
    }
    fn sources(&self, q: &Query) -> Result<(Vec<Source>, Option<i64>)> {
        let mut all = vec![];
        let masks = q.level.map_or(0, |l| (256 - codec::level_bit(l)) & 255);
        self.reader.execute_batch("begin deferred")?;
        let result = (|| -> Result<_> {
            for table in ["blocks", "heads"] {
                let sql = if table == "blocks" {
                    "select b.id,b.segment,s.name,b.first_at,b.last_at,b.count,b.size from blocks b join streams s on s.id=b.stream where b.last_at>=? and b.first_at<? and (?=0 or b.levels&?!=0) order by b.first_at,b.id"
                } else {
                    "select b.id,0,s.name,b.first_at,b.last_at,b.count,b.size from heads b join streams s on s.id=b.stream where b.last_at>=? and b.first_at<? and (?=0 or b.levels&?!=0) order by b.first_at,b.id"
                };
                let mut st = self.reader.prepare_cached(sql)?;
                for row in st.query_map(params![q.from, q.to, masks, masks], |r| {
                    Ok(Source {
                        id: r.get(0)?,
                        segment: r.get(1)?,
                        stream: r.get(2)?,
                        first: r.get(3)?,
                        last: r.get(4)?,
                        count: r.get::<_, i64>(5)? as usize,
                        size: r.get::<_, i64>(6)? as usize,
                        body: vec![],
                        schema: vec![],
                    })
                })? {
                    let src = row?;
                    if src.count == 0
                        || src.count > 1024
                        || src.size == 0
                        || src.size > 4 << 20
                        || src.first > src.last
                    {
                        return Err("indexed count size time".into());
                    };
                    if src.segment != 0 && q.trace.is_some() {
                        let bloom = self.reader.query_row(
                            "select bloom from block_traces where block=?",
                            [src.id],
                            |r| r.get::<_, Vec<u8>>(0),
                        );
                        if let Ok(bloom) = bloom {
                            if !may_hold(&bloom, &q.trace.unwrap()) {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    };
                    all.push(src)
                }
            }
            all.sort_by_key(|s| {
                if q.newest {
                    (-i128::from(s.last), -i128::from(s.id))
                } else {
                    (i128::from(s.first), i128::from(s.id))
                }
            });
            let mut count = 0;
            let mut size = 0;
            let mut decoded = 0;
            let mut selected = Vec::new();
            let mut edge = None;
            let mut schemas = HashMap::<i64, Vec<u8>>::new();
            for mut src in all {
                let extra = if src.segment != 0 && !schemas.contains_key(&src.segment) {
                    let b = self.reader.query_row(
                        "select body from segments where id=? and holder is null",
                        [src.segment],
                        |r| r.get::<_, Vec<u8>>(0),
                    )?;
                    let n = b.len();
                    schemas.insert(src.segment, b);
                    n
                } else {
                    0
                };
                if count + 1 > q.blocks
                    || decoded + src.count > q.decoded
                    || size + src.size + extra > q.bytes
                {
                    edge = Some(if q.newest { src.last } else { src.first });
                    break;
                };
                if src.segment != 0 {
                    src.schema = schemas[&src.segment].clone()
                };
                let sql = if src.segment == 0 {
                    "select body from heads where id=?"
                } else {
                    "select body from blocks where id=?"
                };
                src.body = self
                    .reader
                    .prepare_cached(sql)?
                    .query_row([src.id], |r| r.get(0))?;
                if src.body.len() != src.size {
                    return Err("body size differs from index".into());
                };
                size += src.size + extra;
                decoded += src.count;
                count += 1;
                selected.push(src)
            }
            Ok((selected, edge))
        })();
        self.reader.execute_batch("rollback")?;
        result
    }
    pub fn scan(&self, q: &Query) -> Result<Page> {
        if q.limit == 0
            || q.limit > 10000
            || q.blocks == 0
            || q.blocks > 1024
            || q.decoded > 1 << 20
            || q.bytes > 16 << 20
        {
            return Err("invalid query limit/budget".into());
        };
        let (sources, mut edge) = self.sources(q)?;
        let mut ranked = Vec::new();
        let mut schemas = HashMap::new();
        for s in sources {
            let rs = if s.segment == 0 {
                codec::decode_head(&s.stream, &s.body)?
            } else {
                if !schemas.contains_key(&s.segment) {
                    schemas.insert(s.segment, codec::decode_schema(&s.schema)?);
                };
                let schema = &schemas[&s.segment];
                if schema.stream != s.stream {
                    return Err("schema stream differs from index".into());
                };
                codec::decode_block(schema, &s.body)?
            };
            if rs.len() != s.count
                || rs.iter().map(|r| r.at).min() != Some(s.first)
                || rs.iter().map(|r| r.at).max() != Some(s.last)
            {
                return Err("body count/time differs from index".into());
            };
            for (i, r) in rs.into_iter().enumerate() {
                if q.matches(&r) {
                    let key = (
                        r.at,
                        if s.segment == 0 { i64::MAX } else { s.segment },
                        s.id,
                        i,
                    );
                    ranked.push((key, r))
                }
            }
        }
        ranked.sort_by_key(|(k, _)| *k);
        if q.newest {
            ranked.reverse()
        };
        if ranked.len() > q.limit {
            let t = ranked[q.limit].0.0;
            edge = Some(edge.map_or(t, |e| if q.newest { e.max(t) } else { e.min(t) }))
        };
        let records = ranked
            .into_iter()
            .take(q.limit)
            .filter(|(k, _)| edge.is_none_or(|e| if q.newest { k.0 > e } else { k.0 < e }))
            .map(|(_, r)| r)
            .collect::<Vec<_>>();
        if records.is_empty() && edge.is_some() {
            return Err("limit: timestamp does not fit page/budget".into());
        };
        let mut next = q.clone();
        if let Some(e) = edge {
            if q.newest {
                next.to = e + 1
            } else {
                next.from = e
            }
        };
        Ok(Page {
            records,
            more: edge.is_some(),
            from: next.from,
            to: next.to,
        })
    }
    pub fn follow(&self, after: (i64, usize), limit: usize) -> Result<(Vec<Record>, (i64, usize))> {
        if after.0 < 0 || limit == 0 || limit > 10000 {
            return Err("invalid cursor/limit".into());
        };
        let mut out = Vec::new();
        let mut next = after;
        self.reader.execute_batch("begin deferred")?;
        let result = (|| -> Result<_> {
            let mut st=self.reader.prepare_cached("select id,stream,count,body,held,holder,start from segments where id>=? order by id limit cast(? as integer)")?;
            let rows = st
                .query_map(params![after.0.max(1), limit as i64], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)? as usize,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, i64>(4)? as usize,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, i64>(6)? as usize,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if rows.is_empty() {
                let sequence: i64 = self.reader.query_row(
                    "select coalesce(max(seq),0) from sqlite_sequence where name='segments'",
                    [],
                    |r| r.get(0),
                )?;
                if sequence >= after.0 {
                    next = (sequence + 1, 0)
                }
            }
            let mut fetched = vec![];
            for (id, _, count, body, held, holder, start) in rows {
                if holder.is_some() || start != 0 || count != held {
                    return Err("merged holders unsupported".into());
                };
                let skip = if id == after.0 { after.1 } else { 0 };
                if skip >= count {
                    next = (id + 1, 0);
                    continue;
                };
                let mut blocks = self
                    .reader
                    .prepare_cached("select body from blocks where segment=? order by id")?;
                let bodies = blocks
                    .query_map([id], |r| r.get::<_, Vec<u8>>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                fetched.push((id, count, body, bodies, skip));
                if fetched.iter().map(|(_, n, _, _, s)| n - s).sum::<usize>() >= limit {
                    break;
                }
            }
            Ok(fetched)
        })();
        self.reader.execute_batch("rollback")?;
        for (id, count, body, blocks, skip) in result? {
            let schema = codec::decode_schema(&body)?;
            if schema.count != count {
                return Err("schema count differs".into());
            };
            let mut rs = Vec::new();
            for b in blocks {
                rs.extend(codec::decode_block(&schema, &b)?)
            }
            let before = out.len();
            for r in rs.into_iter().skip(skip).take(limit - out.len()) {
                out.push(r)
            }
            let end = skip + out.len() - before;
            next = if end >= count { (id + 1, 0) } else { (id, end) };
            if out.len() >= limit {
                break;
            }
        }
        Ok((out, next))
    }
    pub fn counters(&self) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        for (name, code) in [
            ("cache_hit", rusqlite::ffi::SQLITE_DBSTATUS_CACHE_HIT),
            ("cache_miss", rusqlite::ffi::SQLITE_DBSTATUS_CACHE_MISS),
            ("cache_write", rusqlite::ffi::SQLITE_DBSTATUS_CACHE_WRITE),
            ("cache_used", rusqlite::ffi::SQLITE_DBSTATUS_CACHE_USED),
            ("statement_bytes", rusqlite::ffi::SQLITE_DBSTATUS_STMT_USED),
        ] {
            let mut total = 0;
            for db in [&self.reader, &self.writer] {
                let (mut current, mut peak) = (0, 0);
                unsafe {
                    rusqlite::ffi::sqlite3_db_status(db.handle(), code, &mut current, &mut peak, 0)
                };
                total += current
            }
            out.insert(name.into(), total.into());
        }
        let (mut current, mut peak) = (0i64, 0i64);
        unsafe {
            rusqlite::ffi::sqlite3_status64(
                rusqlite::ffi::SQLITE_STATUS_MEMORY_USED,
                &mut current,
                &mut peak,
                0,
            );
        }
        out.insert("sqlite_memory_current".into(), current.into());
        out.insert("sqlite_memory_peak".into(), peak.into());
        serde_json::Value::Object(out)
    }
}
fn may_hold(filter: &[u8], value: &[u8]) -> bool {
    if filter.is_empty() {
        return false;
    };
    let mut a = 14695981039346656037u64;
    for b in value {
        a = (a ^ u64::from(*b)).wrapping_mul(1099511628211)
    }
    let b = (a ^ (a >> 31)).wrapping_mul(0x9e3779b97f4a7c15) | 1;
    (0..7).all(|i| {
        let bit = a.wrapping_add(i * b) % (filter.len() as u64 * 8);
        filter[bit as usize / 8] & (1 << (bit % 8)) != 0
    })
}
