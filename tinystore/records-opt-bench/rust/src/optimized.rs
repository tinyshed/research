use crate::{
    Query, Result,
    codec::optimized::{self as decode, Chunk, Row, Rows, SharedSchema},
    engine,
};
use rusqlite::params;
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, VecDeque},
    path::Path,
    sync::Arc,
};

#[derive(Clone, Copy)]
pub struct Options {
    pub selective: bool,
    pub bounded: bool,
    pub share: bool,
    pub cache: u8,
    pub strict: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            selective: true,
            bounded: true,
            share: true,
            cache: 2,
            strict: false,
        }
    }
}
pub struct Engine {
    pub base: engine::Engine,
    options: Options,
    cache: RefCell<Cache>,
}
pub struct Page {
    pub records: Rows,
    pub more: bool,
    pub from: i64,
    pub to: i64,
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
    body: Arc<[u8]>,
    schema: Arc<[u8]>,
}
#[derive(Clone)]
struct Ranked {
    key: (i64, i64, i64, usize),
    newest: bool,
    row: Row,
}
impl PartialEq for Ranked {
    fn eq(&self, x: &Self) -> bool {
        self.key == x.key
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, x: &Self) -> Option<Ordering> {
        Some(self.cmp(x))
    }
}
impl Ord for Ranked {
    fn cmp(&self, x: &Self) -> Ordering {
        if self.newest {
            x.key.cmp(&self.key)
        } else {
            self.key.cmp(&x.key)
        }
    }
}
struct Selection {
    heap: BinaryHeap<Ranked>,
    all: Vec<Ranked>,
    limit: usize,
    newest: bool,
    bounded: bool,
    dropped: Option<i64>,
}
impl Selection {
    fn new(q: &Query, bounded: bool) -> Self {
        Self {
            heap: BinaryHeap::new(),
            all: vec![],
            limit: q.limit,
            newest: q.newest,
            bounded,
            dropped: None,
        }
    }
    fn drop_at(&mut self, at: i64) {
        self.dropped = Some(
            self.dropped
                .map_or(at, |x| if self.newest { x.max(at) } else { x.min(at) }),
        )
    }
    fn offer(&mut self, r: Ranked) {
        if !self.bounded {
            self.all.push(r);
            return;
        };
        if self.heap.len() < self.limit {
            self.heap.push(r);
            return;
        };
        if r < *self.heap.peek().unwrap() {
            let worst = self.heap.pop().unwrap();
            self.drop_at(worst.key.0);
            self.heap.push(r)
        } else {
            self.drop_at(r.key.0)
        }
    }
    fn finish(mut self, q: &Query, mut edge: Option<i64>, share: bool) -> Result<Page> {
        let mut chosen = if self.bounded {
            self.heap.into_vec()
        } else {
            self.all
        };
        chosen.sort_by_key(|r| r.key);
        if self.newest {
            chosen.reverse()
        };
        if !self.bounded && chosen.len() > self.limit {
            let at = chosen[self.limit].key.0;
            self.dropped = Some(at)
        };
        if let Some(at) = self.dropped {
            edge = Some(edge.map_or(at, |e| if q.newest { e.max(at) } else { e.min(at) }))
        };
        let mut records = Vec::with_capacity(self.limit.min(chosen.len()));
        for item in chosen.into_iter().take(self.limit) {
            if edge.is_none_or(|e| {
                if q.newest {
                    item.key.0 > e
                } else {
                    item.key.0 < e
                }
            }) {
                item.row.ready()?;
                records.push(item.row)
            }
        }
        if records.is_empty() && edge.is_some() {
            return Err("limit: timestamp does not fit page/budget".into());
        };
        Ok(Page {
            records: Rows::new(records, share)?,
            more: edge.is_some(),
            from: if !q.newest {
                edge.unwrap_or(q.from)
            } else {
                q.from
            },
            to: if q.newest {
                edge.map_or(q.to, |e| e.saturating_add(1))
            } else {
                q.to
            },
        })
    }
}
#[derive(Clone)]
enum Entry {
    Bytes(Arc<[u8]>),
    Decoded(Arc<Chunk>),
    Schema(Arc<SharedSchema>),
}
struct Cache {
    entries: HashMap<(u8, i64, i64), (Entry, usize)>,
    order: VecDeque<(u8, i64, i64)>,
    bytes: usize,
    hits: u64,
    misses: u64,
}
impl Cache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            hits: 0,
            misses: 0,
        }
    }
    fn get(&mut self, key: (u8, i64, i64)) -> Option<Entry> {
        if let Some((v, _)) = self.entries.get(&key) {
            self.hits += 1;
            Some(v.clone())
        } else {
            self.misses += 1;
            None
        }
    }
    fn put(&mut self, key: (u8, i64, i64), value: Entry, bytes: usize) {
        const BOUND: usize = 4 << 20;
        let bytes = bytes.saturating_add(256); // conservative cache map/order/entry charge
        if bytes > BOUND || self.entries.contains_key(&key) {
            return;
        };
        while self.bytes + bytes > BOUND {
            if let Some(k) = self.order.pop_front() {
                if let Some((_, size)) = self.entries.remove(&k) {
                    self.bytes -= size
                }
            } else {
                break;
            }
        }
        self.bytes += bytes;
        self.entries.insert(key, (value, bytes));
        self.order.push_back(key)
    }
    fn clear(&mut self) {
        self.entries = HashMap::new();
        self.order = VecDeque::new();
        self.bytes = 0
    }
}
impl Engine {
    pub fn open(path: &Path, options: Options) -> Result<Self> {
        Ok(Self {
            base: engine::Engine::open(path)?,
            options,
            cache: RefCell::new(Cache::new()),
        })
    }
    pub fn counters(&self) -> serde_json::Value {
        let mut v = self.base.counters();
        let cache = self.cache.borrow();
        v["follow_cache_bytes"] = cache.bytes.into();
        v["follow_cache_hits"] = cache.hits.into();
        v["follow_cache_misses"] = cache.misses.into();
        v
    }
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear()
    }
    fn sources(&self, q: &Query) -> Result<(Vec<Source>, Option<i64>)> {
        let db = &self.base.reader;
        let masks = q
            .level
            .map_or(0, |l| (256 - crate::codec::level_bit(l)) & 255);
        db.execute_batch("begin deferred")?;
        let result = (|| -> Result<_> {
            let mut candidates = Vec::new();
            for block in [true, false] {
                let sql = if block {
                    "select b.id,b.segment,s.name,b.first_at,b.last_at,b.count,b.size from blocks b join streams s on s.id=b.stream where b.last_at>=? and b.first_at<? and (?=0 or b.levels&?!=0) order by b.first_at,b.id"
                } else {
                    "select b.id,0,s.name,b.first_at,b.last_at,b.count,b.size from heads b join streams s on s.id=b.stream where b.last_at>=? and b.first_at<? and (?=0 or b.levels&?!=0) order by b.first_at,b.id"
                };
                let mut st = db.prepare_cached(sql)?;
                for r in st.query_map(params![q.from, q.to, masks, masks], |r| {
                    Ok(Source {
                        id: r.get(0)?,
                        segment: r.get(1)?,
                        stream: r.get(2)?,
                        first: r.get(3)?,
                        last: r.get(4)?,
                        count: r.get::<_, i64>(5)? as usize,
                        size: r.get::<_, i64>(6)? as usize,
                        body: Arc::from([]),
                        schema: Arc::from([]),
                    })
                })? {
                    let src = r?;
                    if src.count == 0
                        || src.count > 1024
                        || src.size == 0
                        || src.size > 4 << 20
                        || src.first > src.last
                    {
                        return Err("indexed count size time".into());
                    };
                    if block && q.trace.is_some() {
                        let bloom = db
                            .prepare_cached("select bloom from block_traces where block=?")?
                            .query_row([src.id], |r| r.get::<_, Vec<u8>>(0));
                        match bloom {
                            Ok(b) => {
                                if !may_hold(&b, &q.trace.unwrap()) {
                                    continue;
                                }
                            }
                            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                            Err(e) => return Err(e.into()),
                        }
                    };
                    candidates.push(src)
                }
            }
            candidates.sort_by_key(|s| {
                if q.newest {
                    (-i128::from(s.last), -i128::from(s.id))
                } else {
                    (i128::from(s.first), i128::from(s.id))
                }
            });
            let mut selected = Vec::new();
            let (mut bytes, mut decoded) = (0usize, 0usize);
            let mut edge = None;
            let mut schemas = HashMap::<i64, Arc<[u8]>>::new();
            for mut src in candidates {
                let boundary = if q.newest { src.last } else { src.first };
                if self.options.selective && enough(q, &selected, boundary) {
                    edge = Some(boundary);
                    break;
                };
                let extra = if src.segment != 0 && !schemas.contains_key(&src.segment) {
                    let n = db
                        .prepare_cached(
                            "select length(body) from segments where id=? and holder is null",
                        )?
                        .query_row([src.segment], |r| r.get::<_, i64>(0).map(|n| n as usize));
                    match n {
                        Ok(n) => n,
                        Err(e) => return Err(e.into()),
                    }
                } else {
                    0
                };
                if selected.len() + 1 > q.blocks
                    || decoded + src.count > q.decoded
                    || bytes + src.size + extra > q.bytes
                {
                    edge = Some(boundary);
                    break;
                };
                if extra > 0 {
                    let b = db
                        .prepare_cached("select body from segments where id=? and holder is null")?
                        .query_row([src.segment], |r| r.get::<_, Vec<u8>>(0))?;
                    if b.len() != extra {
                        return Err("schema size differs".into());
                    };
                    schemas.insert(src.segment, b.into());
                };
                if src.segment != 0 {
                    src.schema = Arc::clone(&schemas[&src.segment])
                };
                src.body = db
                    .prepare_cached(if src.segment == 0 {
                        "select body from heads where id=?"
                    } else {
                        "select body from blocks where id=?"
                    })?
                    .query_row([src.id], |r| r.get::<_, Vec<u8>>(0))?
                    .into();
                if src.body.len() != src.size {
                    return Err("body size differs from index".into());
                };
                bytes += src.size + extra;
                decoded += src.count;
                selected.push(src)
            }
            Ok((selected, edge))
        })();
        db.execute_batch("rollback")?;
        result
    }
    pub fn scan(&self, q: &Query) -> Result<Page> {
        check_query(q)?;
        let (sources, edge) = self.sources(q)?;
        let mut selection = Selection::new(q, self.options.bounded);
        let mut schemas = HashMap::new();
        for s in sources {
            let chunk = if s.segment == 0 {
                decode::head(&s.stream, &s.body)?
            } else {
                if !schemas.contains_key(&s.segment) {
                    schemas.insert(s.segment, decode::schema(&s.schema)?);
                };
                decode::block(Arc::clone(&schemas[&s.segment]), s.body)?
            };
            if chunk.count() != s.count
                || chunk.times.iter().min() != Some(&s.first)
                || chunk.times.iter().max() != Some(&s.last)
                || chunk.stream() != s.stream
            {
                return Err("body count/time/stream differs from index".into());
            };
            if !self.options.selective || self.options.strict {
                chunk.ensure_all()?
            };
            for i in 0..chunk.count() {
                if chunk.matches(i, q)? {
                    selection.offer(Ranked {
                        key: (
                            chunk.at(i),
                            if s.segment == 0 { i64::MAX } else { s.segment },
                            s.id,
                            i,
                        ),
                        newest: q.newest,
                        row: Row::new(Arc::clone(&chunk), i),
                    })
                }
            }
        }
        selection.finish(q, edge, self.options.share)
    }
    // Fetch metadata in one snapshot, then fetch only blocks intersecting the
    // requested rows. Cache keys include holder first_block; bytes are bounded.
    pub fn follow(&self, after: (i64, usize), limit: usize) -> Result<(Rows, (i64, usize))> {
        if after.0 < 0 || limit == 0 || limit > 10000 {
            return Err("invalid cursor/limit".into());
        };
        let db = &self.base.reader;
        db.execute_batch("begin deferred")?;
        let result = (|| -> Result<_> {
            let mut st=db.prepare_cached("select id,count,held,holder,start,first_block,last_block,length(body) from segments where id>=? order by id limit cast(? as integer)")?;
            let places = st
                .query_map(params![after.0.max(1), limit as i64], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)? as usize,
                        r.get::<_, i64>(2)? as usize,
                        r.get::<_, Option<i64>>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, i64>(7)? as usize,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut sequence = None;
            if places.is_empty() {
                sequence = Some(
                    db.prepare_cached(
                        "select coalesce(max(seq),0) from sqlite_sequence where name='segments'",
                    )?
                    .query_row([], |r| r.get::<_, i64>(0))?,
                )
            };
            let mut fetched = Vec::new();
            let (mut wanted, mut spent) = (limit, 0usize);
            let mut advanced = None;
            for (id, count, held, holder, start, first, last, schema_size) in places {
                if holder.is_some() || start != 0 || held != count || count == 0 || count > 16384 {
                    return Err("unsupported/invalid holder".into());
                };
                let skip = if after.0 == id { after.1 } else { 0 };
                if skip >= count {
                    advanced = Some((id + 1, 0));
                    continue;
                };
                if wanted == 0 {
                    break;
                };
                if schema_size > 4 << 20 {
                    return Err("schema bound".into());
                };
                let row_key = (0, id, first);
                let schema = self.cached_bytes(
                    row_key,
                    "select body from segments where id=?",
                    id,
                    schema_size,
                )?;
                spent += schema_size;
                let take = wanted.min(count - skip);
                let mut st=db.prepare_cached("select id,count,size,first_at,last_at from blocks where id between ? and ? order by id")?;
                let indexes = st
                    .query_map(params![first, last], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)? as usize,
                            r.get::<_, i64>(2)? as usize,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                        ))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let mut offset = 0;
                let mut blocks = Vec::new();
                for (block, n, size, begin, end) in indexes {
                    if n == 0 || n > 1024 || size == 0 || size > 4 << 20 || begin > end {
                        return Err("indexed count size time".into());
                    };
                    let to = offset + n;
                    if to > skip && offset < skip + take {
                        spent += size;
                        if spent > 16 << 20 {
                            return Err("follow byte budget".into());
                        };
                        let key = (2, block, 0);
                        let cached = if self.options.cache == 2 {
                            self.cache.borrow_mut().get(key)
                        } else {
                            None
                        };
                        let value = if let Some(Entry::Decoded(chunk)) = cached {
                            FollowBlock::Decoded(chunk)
                        } else {
                            FollowBlock::Body(self.cached_bytes(
                                (1, block, 0),
                                "select body from blocks where id=?",
                                block,
                                size,
                            )?)
                        };
                        blocks.push((block, offset, n, begin, end, value))
                    };
                    offset = to
                }
                if offset != count {
                    return Err("holder block count differs".into());
                };
                fetched.push((id, first, count, skip, take, schema, blocks));
                wanted -= take
            }
            Ok((fetched, sequence, advanced))
        })();
        db.execute_batch("rollback")?;
        let (fetched, sequence, advanced) = result?;
        let mut out = Vec::new();
        let mut next = after;
        if let Some(cursor) = advanced {
            next = cursor
        }
        if let Some(s) = sequence {
            if s >= after.0 {
                next = (s + 1, 0)
            }
        };
        for (id, first, count, skip, take, schema, blocks) in fetched {
            let key = (3, id, first);
            let parsed = if self.options.cache == 2 {
                match self.cache.borrow_mut().get(key) {
                    Some(Entry::Schema(s)) => Some(s),
                    _ => None,
                }
            } else {
                None
            };
            let parsed = if let Some(s) = parsed {
                s
            } else {
                let s = decode::schema(&schema)?;
                if self.options.cache == 2 {
                    self.cache.borrow_mut().put(
                        key,
                        Entry::Schema(Arc::clone(&s)),
                        s.retained_bytes(),
                    )
                };
                s
            };
            for (block, offset, n, begin, end, value) in blocks {
                let chunk = match value {
                    FollowBlock::Decoded(c) => c,
                    FollowBlock::Body(body) => {
                        let c = decode::block(Arc::clone(&parsed), body)?;
                        if c.count() != n
                            || c.times.first() != Some(&begin)
                            || c.times.last() != Some(&end)
                        {
                            return Err("body count/time differs from index".into());
                        };
                        c.ensure_all()?;
                        if self.options.cache == 2 {
                            self.cache.borrow_mut().put(
                                (2, block, 0),
                                Entry::Decoded(Arc::clone(&c)),
                                c.retained_bytes(),
                            )
                        };
                        c
                    }
                };
                if chunk.count() != n
                    || chunk.times.first() != Some(&begin)
                    || chunk.times.last() != Some(&end)
                {
                    return Err("cached body count/time differs from index".into());
                }
                for row in skip.saturating_sub(offset)..(skip + take - offset).min(n) {
                    let row = Row::new(Arc::clone(&chunk), row);
                    row.ready()?;
                    out.push(row)
                }
            }
            let end = skip + take;
            next = if end >= count { (id + 1, 0) } else { (id, end) }
        }
        Ok((Rows::new(out, self.options.share)?, next))
    }
    fn cached_bytes(
        &self,
        key: (u8, i64, i64),
        sql: &str,
        id: i64,
        size: usize,
    ) -> Result<Arc<[u8]>> {
        if self.options.cache > 0 {
            if let Some(Entry::Bytes(b)) = self.cache.borrow_mut().get(key) {
                return Ok(b);
            }
        };
        let body: Arc<[u8]> = self
            .base
            .reader
            .prepare_cached(sql)?
            .query_row([id], |r| r.get::<_, Vec<u8>>(0))?
            .into();
        if body.len() != size {
            return Err("body size differs from index".into());
        };
        if self.options.cache > 0 {
            self.cache
                .borrow_mut()
                .put(key, Entry::Bytes(Arc::clone(&body)), size)
        };
        Ok(body)
    }
}
enum FollowBlock {
    Decoded(Arc<Chunk>),
    Body(Arc<[u8]>),
}
fn check_query(q: &Query) -> Result<()> {
    if q.limit == 0
        || q.limit > 10000
        || q.blocks == 0
        || q.blocks > 1024
        || q.decoded > 1 << 20
        || q.bytes > 16 << 20
        || q.to < q.from
    {
        return Err("invalid query limit/budget".into());
    };
    Ok(())
}
fn enough(q: &Query, taken: &[Source], bound: i64) -> bool {
    if taken.is_empty()
        || q.level.is_some()
        || q.trace.is_some()
        || !q.attrs.is_empty()
        || !q.context.is_empty()
        || !q.search.is_empty()
    {
        return false;
    };
    if !q.newest && bound <= q.from || q.newest && bound >= q.to - 1 {
        return false;
    };
    let expected = taken
        .iter()
        .map(|s| {
            let span = (s.last as u64).wrapping_sub(s.first as u64).max(1) as f64;
            let included = if q.newest {
                (s.last.min(q.to - 1) as i128 - bound.max(s.first).max(q.from) as i128).max(0)
            } else {
                (bound.min(s.last) as i128 - s.first.max(q.from) as i128).max(0)
            };
            s.count as f64 * (included as f64 / span).clamp(0.0, 1.0)
        })
        .sum::<f64>();
    expected >= q.limit as f64
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
    (0u64..7).all(|i| {
        let bit = a.wrapping_add(i.wrapping_mul(b)) % (filter.len() as u64 * 8);
        filter[bit as usize / 8] & (1 << (bit % 8)) != 0
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_selector_preserves_equal_time_boundary() {
        let rs = crate::fixture(8);
        let encoded = crate::codec::encode_head(&rs).unwrap();
        let chunk = decode::head("stream-0", &encoded).unwrap();
        for newest in [false, true] {
            let mut q = Query::for_case("scan_page");
            q.limit = 3;
            q.newest = newest;
            let mut bounded = Selection::new(&q, true);
            let mut all = Selection::new(&q, false);
            for i in 0..chunk.count() {
                let item = Ranked {
                    key: (chunk.at(i), 0, 0, i),
                    newest,
                    row: Row::new(Arc::clone(&chunk), i),
                };
                bounded.offer(item.clone());
                all.offer(item)
            }
            let a = bounded.finish(&q, None, true).unwrap();
            let b = all.finish(&q, None, true).unwrap();
            assert_eq!(a.records.len(), b.records.len());
            assert_eq!(a.from, b.from);
            assert_eq!(a.to, b.to);
            assert_eq!(a.records.len(), 2)
        }
    }
    #[test]
    fn cache_eviction_preserves_owning_entries() {
        let mut c = Cache::new();
        let first: Arc<[u8]> = vec![42u8; 2 << 20].into();
        c.put((0, 1, 0), Entry::Bytes(first.clone()), first.len());
        c.put((0, 2, 0), Entry::Bytes(vec![0; 3 << 20].into()), 3 << 20);
        assert!(c.bytes <= 4 << 20);
        assert!(c.get((0, 1, 0)).is_none());
        assert_eq!(first[0], 42)
    }
}
