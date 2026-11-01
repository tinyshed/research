//! A faster reader of the compact blocks, every change a switch, for the
//! search-fast round: what each one buys is measured against compact1's
//! reader on the same file.
//!
//! The blocks, their rows and the lengths are compact.db's, read as compact1
//! reads them. What is added lives in a file of its own, `fast.db`, attached:
//! for every key with blocks, each block's Pareto front of (frequency,
//! length) — the postings no other in the block has both a higher or equal
//! frequency and a shorter or equal field than — and the ten postings of
//! the key that score best at the build's average length. Both stay exact
//! whatever the average and the idf become: BM25's saturation grows with the
//! frequency and falls with the length, so a block's best posting is on its
//! front, and the ten are real postings whose scores are computed when asked.
//!
//! Switches, read once when a reader opens:
//!
//! - `FAST_BOUNDS=front` takes a block's bound from its front, the block's
//!   exact best; `loose`, compact's own, from its largest frequency and its
//!   shortest field, which may belong to different postings.
//! - `FAST_SEED=1` starts θ at the best of each key's tenth-best score, a
//!   bound below the true tenth score of the query, so that pruning begins
//!   before ten documents have been scored.
//! - `FAST_NORMS=flat` keeps the field's lengths as one array of u16 a
//!   document, 2 bytes where compact1 keeps four u16 in chunks of 65,536.
//! - `FAST_AHEAD=n` reads up to n blocks of a list in one statement once the
//!   list is read in order (compact1's arm reads 8).
//! - `FAST_READ=blob` reads each block through one incremental-blob handle
//!   moved from row to row, as FTS5 reads its segments, from a rowid copy of
//!   the blocks in fast.db numbered key << 17 | block; `sql`, a statement.
//! - `FAST_CACHE_KIB=n` gives SQLite n KiB of page cache a database (1,024 as
//!   TinyStore opens one); `FAST_MMAP=n` maps up to n bytes of each file.
//! - `FAST_PROFILE=1` times the reader's parts, for the profile only.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::{CachedStatement, Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::blocks::{BLOCK, END, FIELDS, Fetch, List, Posting, Skip, get_varint, idf, key, put_varint, saturated};
use crate::compact1::FIELD;
use crate::fts::open;

const NORMS: u32 = 65_536;
const SEEDS: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bounds {
    Loose,
    Front,
}

#[derive(Clone, Copy, Debug)]
pub struct Switches {
    pub bounds: Bounds,
    pub seed: bool,
    pub flat: bool,
    pub ahead: usize,
    pub blob: bool,
    pub cache_kib: i64,
    pub mmap: i64,
    pub profile: bool,
}

impl Switches {
    pub fn from_env() -> Switches {
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
        Switches {
            bounds: match var("FAST_BOUNDS", "front").as_str() {
                "front" => Bounds::Front,
                "loose" => Bounds::Loose,
                other => panic!("FAST_BOUNDS={other}"),
            },
            seed: var("FAST_SEED", "1") == "1",
            flat: var("FAST_NORMS", "flat") == "flat",
            ahead: var("FAST_AHEAD", "8").parse().unwrap(),
            blob: match var("FAST_READ", "sql").as_str() {
                "blob" => true,
                "sql" => false,
                other => panic!("FAST_READ={other}"),
            },
            cache_kib: var("FAST_CACHE_KIB", "1024").parse().unwrap(),
            mmap: var("FAST_MMAP", "0").parse().unwrap(),
            profile: var("FAST_PROFILE", "0") == "1",
        }
    }

    pub fn to_json(self) -> Value {
        json!({"bounds": format!("{:?}", self.bounds).to_lowercase(), "seed": self.seed, "flat": self.flat,
            "ahead": self.ahead, "read": if self.blob { "blob" } else { "sql" }, "cache_kib": self.cache_kib,
            "mmap": self.mmap})
    }
}

fn unpack_raw(payload: &[u8], out: &mut Vec<(u32, u32)>) {
    out.clear();
    let (mut at, mut doc) = (0, 0);
    while at < payload.len() {
        doc += get_varint(payload, &mut at);
        out.push((doc, get_varint(payload, &mut at)));
    }
}

/// The postings of a block that no other dominates: sorted by length, each
/// with a frequency above every shorter or equal one's.
fn front(postings: &mut [(u32, u32)]) -> Vec<(u32, u32)> {
    postings.sort_unstable_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &(tf, len) in postings.iter() {
        if out.last().is_none_or(|&(best, _)| tf > best) {
            out.push((tf, len));
        }
    }
    out
}

/// One field's lengths, read from compact's norms.
fn flat_norms(connection: &Connection, from_chunk: u32, into: &mut Vec<u16>) {
    let mut rows = connection
        .prepare("select chunk, lengths from norms where chunk >= ?1 order by chunk")
        .unwrap();
    let mut read = rows.query([from_chunk]).unwrap();
    while let Some(row) = read.next().unwrap() {
        let chunk: i64 = row.get(0).unwrap();
        let bytes: Vec<u8> = row.get(1).unwrap();
        let base = chunk as usize * NORMS as usize;
        let need = base + NORMS as usize;
        if into.len() < need {
            into.resize(need, 0);
        }
        for (i, pair) in bytes.chunks(8).enumerate() {
            let at = FIELD * 2;
            into[base + i] = u16::from_le_bytes([pair[at], pair[at + 1]]);
        }
    }
}

/// Writes fast.db beside a compact.db: each block's front, each key's ten.
pub fn build(compact: &Path, fast: &Path) -> Value {
    let started = Instant::now();
    let source = open(compact);
    source.pragma_update(None, "query_only", true).unwrap();
    let mut lengths = Vec::new();
    flat_norms(&source, 0, &mut lengths);
    let (tokens, documents): (i64, i64) = source
        .query_row("select tokens, documents from stats where field = ?1", [FIELD as i64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let average = (tokens as f64 / documents.max(1) as f64).max(1.0);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", fast.display()));
    }
    let mut out = open(fast);
    out.execute_batch(
        "create table fdir (id integer primary key, front blob not null, seeds blob not null);
         create table fblob (id integer primary key, payload blob not null);",
    )
        .unwrap();
    let tx = out.transaction().unwrap();
    let (mut keys, mut blocks, mut points) = (0u64, 0u64, 0u64);
    {
        let mut write = tx
            .prepare("insert into fdir (id, front, seeds) values (?1, ?2, ?3)")
            .unwrap();
        let mut copy = tx
            .prepare("insert into fblob (id, payload) values (?1, ?2)")
            .unwrap();
        let mut index: i64 = 0;
        let mut scan = source
            .prepare("select id, payload from cblob order by id, first")
            .unwrap();
        let mut rows = scan.query([]).unwrap();
        let mut current: i64 = -1;
        let mut fronts: Vec<u8> = Vec::new();
        let mut best: Vec<(f64, u32, u32, u32)> = Vec::new();
        let mut raw = Vec::with_capacity(BLOCK);
        let mut flush = |id: i64, fronts: &mut Vec<u8>, best: &mut Vec<(f64, u32, u32, u32)>| {
            let order = |a: &(f64, u32, u32, u32), b: &(f64, u32, u32, u32)| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1));
            if best.len() > SEEDS {
                best.select_nth_unstable_by(SEEDS - 1, order);
                best.truncate(SEEDS);
            }
            best.sort_unstable_by(order);
            let mut seeds = Vec::new();
            for &(_, doc, tf, len) in best.iter() {
                put_varint(&mut seeds, doc);
                put_varint(&mut seeds, tf);
                put_varint(&mut seeds, len);
            }
            write.execute(params![id, std::mem::take(fronts), seeds]).unwrap();
            best.clear();
        };
        while let Some(row) = rows.next().unwrap() {
            let id: i64 = row.get(0).unwrap();
            let payload: Vec<u8> = row.get(1).unwrap();
            if id != current {
                if current >= 0 {
                    flush(current, &mut fronts, &mut best);
                }
                current = id;
                keys += 1;
                index = 0;
            }
            copy.execute(params![(id << 17) | index, &payload]).unwrap();
            index += 1;
            unpack_raw(&payload, &mut raw);
            let mut pairs: Vec<(u32, u32)> = raw
                .iter()
                .map(|&(doc, tf)| (tf, u32::from(lengths[doc as usize])))
                .collect();
            for (&(doc, tf), &(_, len)) in raw.iter().zip(&pairs) {
                best.push((saturated(tf, len, average), doc, tf, len));
            }
            let f = front(&mut pairs);
            put_varint(&mut fronts, f.len() as u32);
            for (tf, len) in &f {
                put_varint(&mut fronts, *tf);
                put_varint(&mut fronts, *len);
            }
            blocks += 1;
            points += f.len() as u64;
        }
        if current >= 0 {
            flush(current, &mut fronts, &mut best);
        }
    }
    tx.commit().unwrap();
    out.pragma_update(None, "wal_checkpoint", "truncate").unwrap();
    let objects = crate::compact1::objects(&out);
    json!({"step": "build", "engine": "fast", "seconds": started.elapsed().as_secs_f64(), "keys": keys,
        "blocks": blocks, "front_points": points, "average": average,
        "file": std::fs::metadata(fast).unwrap().len(), "objects": objects})
}

/// What the reader spent its time on, for the profile.
#[derive(Default, Clone, Copy)]
pub struct Spent {
    pub lists: Duration,
    pub statements: Duration,
    pub decode: Duration,
    pub total: Duration,
    pub postings: usize,
    pub scored: usize,
}

struct Source<'a> {
    point: CachedStatement<'a>,
    range: CachedStatement<'a>,
    gone: &'a HashSet<u32>,
    flat: Option<&'a [u16]>,
    chunks: &'a mut HashMap<u32, std::rc::Rc<Vec<u16>>>,
    connection: &'a Connection,
    ahead: usize,
    profile: bool,
    use_blob: bool,
    blob: Option<rusqlite::blob::Blob<'a>>,
    buffer: Vec<u8>,
    read: HashMap<i64, (usize, Vec<(usize, Vec<Posting>)>)>,
    raw: Vec<(u32, u32)>,
    opened: usize,
    statements: usize,
    spent: Spent,
}

impl Source<'_> {
    fn length(&mut self, doc: u32) -> u32 {
        if let Some(flat) = self.flat {
            return u32::from(flat.get(doc as usize).copied().unwrap_or(0));
        }
        let chunk = doc / NORMS;
        let connection = self.connection;
        let lengths = self.chunks.entry(chunk).or_insert_with(|| {
            let bytes: Vec<u8> = connection
                .prepare_cached("select lengths from norms where chunk = ?1")
                .unwrap()
                .query_row([chunk], |row| row.get(0))
                .optional()
                .unwrap()
                .unwrap_or_default();
            let mut lengths: Vec<u16> = bytes.chunks(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
            lengths.resize(NORMS as usize * 4, 0);
            std::rc::Rc::new(lengths)
        });
        u32::from(lengths[((doc % NORMS) as usize) * 4 + FIELD])
    }

    fn postings(&mut self, payload: &[u8]) -> Vec<Posting> {
        let mut raw = std::mem::take(&mut self.raw);
        unpack_raw(payload, &mut raw);
        let mut out = Vec::with_capacity(raw.len());
        for &(doc, tf) in &raw {
            if !self.gone.is_empty() && self.gone.contains(&doc) {
                continue;
            }
            let len = self.length(doc);
            out.push(Posting { doc, tf, len });
        }
        self.spent.postings += raw.len();
        self.raw = raw;
        out
    }
}

impl Fetch for Source<'_> {
    fn block(&mut self, list: &List, index: usize) -> Vec<Posting> {
        if self.use_blob {
            let took = self.profile.then(Instant::now);
            let row = (list.id << 17) | index as i64;
            match self.blob.as_mut() {
                Some(blob) => blob.reopen(row).unwrap(),
                None => self.blob = Some(self.connection.blob_open("f", "fblob", "payload", row, true).unwrap()),
            }
            let blob = self.blob.as_ref().unwrap();
            let mut buffer = std::mem::take(&mut self.buffer);
            buffer.resize(blob.len(), 0);
            blob.read_at_exact(&mut buffer, 0).unwrap();
            let decoding = self.profile.then(Instant::now);
            if let (Some(took), Some(decoding)) = (took, decoding) {
                self.spent.statements += decoding - took;
            }
            self.statements += 1;
            self.opened += 1;
            let postings = self.postings(&buffer);
            self.buffer = buffer;
            if let Some(decoding) = decoding {
                self.spent.decode += decoding.elapsed();
            }
            return postings;
        }
        let read = self.read.entry(list.id).or_insert((usize::MAX, Vec::new()));
        if let Some(at) = read.1.iter().position(|(block, _)| *block == index) {
            let (_, postings) = read.1.swap_remove(at);
            read.0 = index;
            return postings;
        }
        let in_order = read.0 != usize::MAX && index == read.0 + 1;
        read.0 = index;
        let took = self.profile.then(Instant::now);
        let payloads: Vec<(u32, Vec<u8>)> = if self.ahead > 1 && in_order {
            let last = (index + self.ahead - 1).min(list.skips.len() - 1);
            self.range
                .query_map(params![list.id, list.skips[index].first, list.skips[last].first], |row| {
                    Ok((row.get::<_, i64>(0)? as u32, row.get::<_, Vec<u8>>(1)?))
                })
                .unwrap()
                .map(Result::unwrap)
                .collect()
        } else {
            let first = list.skips[index].first;
            vec![(first, self.point.query_row(params![list.id, first], |row| row.get(0)).unwrap())]
        };
        let decoding = self.profile.then(Instant::now);
        if let (Some(took), Some(decoding)) = (took, decoding) {
            self.spent.statements += decoding - took;
        }
        self.statements += 1;
        self.opened += payloads.len();
        let mut wanted = Vec::new();
        for (offset, (_, payload)) in payloads.iter().enumerate() {
            let postings = self.postings(payload);
            if offset == 0 {
                wanted = postings;
            } else {
                self.read.get_mut(&list.id).unwrap().1.push((index + offset, postings));
            }
        }
        if let Some(decoding) = decoding {
            self.spent.decode += decoding.elapsed();
        }
        wanted
    }
}

pub struct Reader {
    connection: Connection,
    documents: u64,
    average: f64,
    gone: HashSet<u32>,
    flat: Vec<u16>,
    chunks: HashMap<u32, std::rc::Rc<Vec<u16>>>,
    version: i64,
    settled_chunks: u32,
    pub switches: Switches,
    pub opened: usize,
    pub statements: usize,
    pub spent: Spent,
}

impl Reader {
    pub fn open(compact: &Path, switches: Switches) -> Reader {
        let connection = open(compact);
        let fast = compact.with_file_name("fast.db");
        connection
            .execute(&format!("attach database '{}' as f", fast.display()), [])
            .unwrap();
        for schema in ["main", "f"] {
            connection
                .execute_batch(&format!("pragma {schema}.cache_size = -{}; pragma {schema}.mmap_size = {};",
                    switches.cache_kib, switches.mmap))
                .unwrap();
        }
        connection.pragma_update(None, "query_only", true).unwrap();
        let settled: i64 = connection
            .query_row("select coalesce(max(id), 0) from docs", [], |row| row.get(0))
            .unwrap();
        let mut reader = Reader {
            connection,
            documents: 0,
            average: 1.0,
            gone: HashSet::new(),
            flat: Vec::new(),
            chunks: HashMap::new(),
            version: -1,
            settled_chunks: settled as u32 / NORMS,
            switches,
            opened: 0,
            statements: 0,
            spent: Spent::default(),
        };
        if switches.flat {
            flat_norms(&reader.connection, 0, &mut reader.flat);
        }
        reader.refresh();
        reader
    }

    fn refresh(&mut self) {
        let version: i64 = self
            .connection
            .query_row("pragma data_version", [], |row| row.get(0))
            .unwrap();
        if version == self.version {
            return;
        }
        let first = self.version == -1;
        self.version = version;
        let (tokens, holding): (i64, i64) = self
            .connection
            .prepare_cached("select tokens, documents from stats where field = ?1")
            .unwrap()
            .query_row([FIELD as i64], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        self.average = (tokens as f64 / holding.max(1) as f64).max(1.0);
        self.documents = holding as u64;
        self.gone = self
            .connection
            .prepare_cached("select doc from gone")
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .map(|doc| doc.unwrap() as u32)
            .collect();
        let settled = self.settled_chunks;
        self.chunks.retain(|chunk, _| *chunk < settled);
        if self.switches.flat && !first {
            flat_norms(&self.connection, settled, &mut self.flat);
        }
    }

    fn length(&self, doc: u32) -> u32 {
        u32::from(self.flat.get(doc as usize).copied().unwrap_or(0))
    }

    /// The lists of a query, and θ's seed: the best of the keys' tenth scores.
    fn lists(&self, terms: &[String], k: usize) -> (Vec<List>, f64) {
        let mut find = self
            .connection
            .prepare_cached(
                "select c.id, c.documents, c.directory, c.inline, d.front, d.seeds \
                 from ckeys c left join f.fdir d on d.id = c.id where c.key = ?1",
            )
            .unwrap();
        let mut heads = self
            .connection
            .prepare_cached("select doc, tf, len from head where key = ?1 order by doc")
            .unwrap();
        let mut lists = Vec::new();
        let mut seed = 0.0f64;
        let tenth = |mut scores: Vec<f64>| -> f64 {
            if scores.len() < k {
                return 0.0;
            }
            scores.sort_unstable_by(|a, b| b.total_cmp(a));
            scores[k - 1]
        };
        type Found = (i64, u64, Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>);
        for term in terms {
            let key = key(FIELD, term);
            let found: Option<Found> = find
                .query_row([&key], |row| {
                    Ok((row.get(0)?, row.get::<_, i64>(1)? as u64, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
                })
                .optional()
                .unwrap();
            let holding = found.as_ref().map_or(0, |found| found.1);
            let scale = FIELDS[FIELD].1 * idf(self.documents, holding.max(1));
            let average = self.average;
            match found {
                Some((_, _, _, Some(inline), _, _)) => {
                    let postings = unpack_inline(&inline, &self.gone);
                    if !postings.is_empty() {
                        if self.switches.seed {
                            seed = seed.max(tenth(
                                postings.iter().map(|p| scale * saturated(p.tf, p.len, average)).collect(),
                            ));
                        }
                        lists.push(List::whole(key.clone(), FIELD, scale, average, postings));
                    }
                }
                Some((id, _, Some(directory), None, front, seeds)) => {
                    let entries = decode_directory(&directory);
                    let fronts = front.as_deref().map(decode_fronts);
                    let skips: Vec<Skip> = entries
                        .iter()
                        .enumerate()
                        .map(|(at, entry)| {
                            let bound = match (self.switches.bounds, &fronts) {
                                (Bounds::Front, Some(fronts)) => fronts[at]
                                    .iter()
                                    .map(|(tf, len)| scale * saturated(*tf, *len, average))
                                    .fold(0.0, f64::max),
                                _ => scale * saturated(entry.2, entry.3, average),
                            };
                            Skip {
                                first: entry.0,
                                last: entry.1,
                                bound,
                            }
                        })
                        .collect();
                    if self.switches.seed
                        && let Some(seeds) = seeds
                    {
                        let mut at = 0;
                        let mut scores = Vec::new();
                        while at < seeds.len() {
                            let doc = get_varint(&seeds, &mut at);
                            let tf = get_varint(&seeds, &mut at);
                            let len = get_varint(&seeds, &mut at);
                            if !self.gone.contains(&doc) && (self.flat.is_empty() || self.length(doc) == len) {
                                scores.push(scale * saturated(tf, len, average));
                            }
                        }
                        seed = seed.max(tenth(scores));
                    }
                    let max = skips.iter().map(|skip| skip.bound).fold(0.0, f64::max);
                    lists.push(List {
                        key: key.clone(),
                        id,
                        field: FIELD,
                        scale,
                        average,
                        skips,
                        max,
                        block: 0,
                        decoded: usize::MAX,
                        postings: Vec::new(),
                        at: 0,
                    });
                }
                _ => {}
            }
            let head: Vec<Posting> = heads
                .query_map([&key], |row| {
                    Ok(Posting {
                        doc: row.get::<_, i64>(0)? as u32,
                        tf: row.get::<_, i64>(1)? as u32,
                        len: row.get::<_, i64>(2)? as u32,
                    })
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            if !head.is_empty() {
                lists.push(List::whole(key, FIELD, scale, average, head));
            }
        }
        (lists, seed)
    }

    pub fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        let started = Instant::now();
        self.refresh();
        let (mut lists, seed) = self.lists(terms, k);
        let listed = started.elapsed();
        let mut chunks = std::mem::take(&mut self.chunks);
        let flat = std::mem::take(&mut self.flat);
        let (found, opened, statements, mut spent) = {
            let mut source = Source {
                point: self
                    .connection
                    .prepare_cached("select payload from cblob where id = ?1 and first = ?2")
                    .unwrap(),
                range: self
                    .connection
                    .prepare_cached(
                        "select first, payload from cblob where id = ?1 and first >= ?2 and first <= ?3 order by first",
                    )
                    .unwrap(),
                gone: &self.gone,
                flat: if self.switches.flat { Some(&flat) } else { None },
                chunks: &mut chunks,
                connection: &self.connection,
                ahead: self.switches.ahead,
                profile: self.switches.profile,
                use_blob: self.switches.blob,
                blob: None,
                buffer: Vec::new(),
                read: HashMap::new(),
                raw: Vec::with_capacity(BLOCK),
                opened: 0,
                statements: 0,
                spent: Spent::default(),
            };
            let mut scored = 0usize;
            let found = windowed(&mut lists, &mut source, k, seed, &mut scored);
            source.spent.scored = scored;
            (found, source.opened, source.statements, source.spent)
        };
        self.chunks = chunks;
        self.flat = flat;
        spent.lists = listed;
        spent.total = started.elapsed();
        self.opened = opened;
        self.statements = statements;
        self.spent = spent;
        found
    }
}

fn decode_directory(bytes: &[u8]) -> Vec<(u32, u32, u32, u32)> {
    let mut entries = Vec::new();
    let (mut at, mut previous) = (0, 0);
    while at < bytes.len() {
        let first = previous + get_varint(bytes, &mut at);
        let last = first + get_varint(bytes, &mut at);
        let most_tf = get_varint(bytes, &mut at);
        let least_len = get_varint(bytes, &mut at);
        entries.push((first, last, most_tf, least_len));
        previous = last;
    }
    entries
}

fn decode_fronts(bytes: &[u8]) -> Vec<Vec<(u32, u32)>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let n = get_varint(bytes, &mut at) as usize;
        let mut points = Vec::with_capacity(n);
        for _ in 0..n {
            let tf = get_varint(bytes, &mut at);
            let len = get_varint(bytes, &mut at);
            points.push((tf, len));
        }
        out.push(points);
    }
    out
}

fn unpack_inline(bytes: &[u8], gone: &HashSet<u32>) -> Vec<Posting> {
    let mut out = Vec::new();
    let (mut at, mut doc) = (0, 0);
    while at < bytes.len() {
        doc += get_varint(bytes, &mut at);
        let tf = get_varint(bytes, &mut at);
        let len = get_varint(bytes, &mut at);
        if !gone.contains(&doc) {
            out.push(Posting { doc, tf, len });
        }
    }
    out
}

/// blocks.rs's cursor over a list, which it keeps private, under names of
/// its own: the same steps, line for line.
trait Walk {
    fn at_doc(&mut self, source: &mut dyn Fetch) -> u32;
    fn load(&mut self, source: &mut dyn Fetch);
    fn step(&mut self);
    fn go_to(&mut self, doc: u32, source: &mut dyn Fetch) -> u32;
    fn block_bound(&self, doc: u32) -> f64;
    fn bound_in(&self, start: u32, end: u32) -> f64;
    fn first_from(&self, end: u32) -> u32;
    fn term_score(&self) -> f64;
}

impl Walk for List {
    fn at_doc(&mut self, source: &mut dyn Fetch) -> u32 {
        loop {
            if self.block >= self.skips.len() {
                return END;
            }
            self.load(source);
            if self.at < self.postings.len() {
                return self.postings[self.at].doc;
            }
            self.block += 1;
            self.at = 0;
        }
    }

    fn load(&mut self, source: &mut dyn Fetch) {
        if self.decoded == self.block {
            return;
        }
        let postings = source.block(self, self.block);
        self.postings = postings;
        self.decoded = self.block;
    }

    fn step(&mut self) {
        self.at += 1;
    }

    fn go_to(&mut self, doc: u32, source: &mut dyn Fetch) -> u32 {
        while self.block < self.skips.len() && self.skips[self.block].last < doc {
            self.block += 1;
            self.at = 0;
        }
        if self.block >= self.skips.len() {
            return END;
        }
        self.load(source);
        while self.at < self.postings.len() && self.postings[self.at].doc < doc {
            self.at += 1;
        }
        self.at_doc(source)
    }

    fn block_bound(&self, doc: u32) -> f64 {
        let rest = &self.skips[self.block.min(self.skips.len())..];
        let at = rest.partition_point(|skip| skip.last < doc);
        rest.get(at)
            .filter(|skip| skip.first <= doc)
            .map_or(0.0, |skip| skip.bound)
    }

    fn bound_in(&self, start: u32, end: u32) -> f64 {
        let rest = &self.skips[self.block.min(self.skips.len())..];
        let from = rest.partition_point(|skip| skip.last < start);
        rest[from..]
            .iter()
            .take_while(|skip| skip.first < end)
            .map(|skip| skip.bound)
            .fold(0.0, f64::max)
    }

    fn first_from(&self, end: u32) -> u32 {
        let rest = &self.skips[self.block.min(self.skips.len())..];
        let from = rest.partition_point(|skip| skip.last < end);
        rest.get(from).map_or(END, |skip| skip.first.max(end))
    }

    fn term_score(&self) -> f64 {
        let posting = self.postings[self.at];
        self.scale * saturated(posting.tf, posting.len, self.average)
    }
}

/// A score and its document, ordered by score, then the lower document first.
#[derive(PartialEq, PartialOrd)]
struct Scored(f64, Reverse<u32>);

impl Eq for Scored {}

impl Ord for Scored {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

const WINDOW: u32 = 4096;

/// blocks.rs's `windowed`, with θ never below `seed`'s floor: a document
/// whose bound is below the seed cannot be among the best `k`, since `k`
/// documents score at least the seed; one at the seed exactly is kept, the
/// floor being the largest float below it.
fn windowed(lists: &mut [List], source: &mut dyn Fetch, k: usize, seed: f64, scored: &mut usize) -> Vec<(f64, u32)> {
    let floor = if seed > 0.0 { f64::from_bits(seed.to_bits() - 1) } else { 0.0 };
    let theta_of = |best: &BinaryHeap<Reverse<Scored>>| -> Option<f64> {
        let heap = (best.len() >= k).then(|| best.peek().unwrap().0.0);
        match (heap, floor > 0.0) {
            (Some(theta), _) => Some(theta.max(floor)),
            (None, true) => Some(floor),
            (None, false) => None,
        }
    };
    let mut best: BinaryHeap<Reverse<Scored>> = BinaryHeap::new();
    let mut scores = vec![0.0f64; WINDOW as usize];
    let mut touched: Vec<u32> = Vec::new();
    let mut start = lists
        .iter()
        .map(|list| list.first_from(0))
        .min()
        .unwrap_or(END);
    while start != END {
        let end = start.saturating_add(WINDOW);
        let bounds: Vec<f64> = lists
            .iter()
            .map(|list| list.bound_in(start, end))
            .collect();
        let mut order: Vec<usize> = (0..lists.len()).collect();
        order.sort_by(|a, b| bounds[*a].total_cmp(&bounds[*b]));
        let passive = match theta_of(&best) {
            None => 0,
            Some(theta) => {
                let mut sum = 0.0;
                order
                    .iter()
                    .take_while(|list| {
                        sum += bounds[**list];
                        sum <= theta
                    })
                    .count()
            }
        };
        if passive < order.len() {
            for &at in &order[passive..] {
                let list = &mut lists[at];
                let mut doc = list.go_to(start, source);
                while doc < end {
                    let slot = (doc - start) as usize;
                    if scores[slot] == 0.0 {
                        touched.push(doc);
                    }
                    scores[slot] += list.term_score();
                    *scored += 1;
                    list.step();
                    doc = list.at_doc(source);
                }
            }
            touched.sort_unstable();
            let window_rest: f64 = order[..passive].iter().map(|at| bounds[*at]).sum();
            for &doc in &touched {
                let mut score = std::mem::take(&mut scores[(doc - start) as usize]);
                let theta = theta_of(&best);
                let full = theta.is_some();
                let theta = theta.unwrap_or(0.0);
                if full && score + window_rest <= theta {
                    continue;
                }
                let mut rest: f64 = order[..passive]
                    .iter()
                    .map(|at| lists[*at].block_bound(doc))
                    .sum();
                for &at in order[..passive].iter().rev() {
                    if full && score + rest <= theta {
                        break;
                    }
                    let bound = lists[at].block_bound(doc);
                    rest -= bound;
                    if bound > 0.0 && lists[at].go_to(doc, source) == doc {
                        score += lists[at].term_score();
                        *scored += 1;
                    }
                }
                if best.len() < k {
                    if !full || score > theta {
                        best.push(Reverse(Scored(score, Reverse(doc))));
                    }
                } else if score > best.peek().unwrap().0.0 {
                    best.pop();
                    best.push(Reverse(Scored(score, Reverse(doc))));
                }
            }
            touched.clear();
        }
        start = lists
            .iter()
            .map(|list| list.first_from(end))
            .min()
            .unwrap_or(END);
    }
    let mut found: Vec<(f64, u32)> = best
        .into_iter()
        .map(|Reverse(Scored(score, Reverse(doc)))| (score, doc))
        .collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    found
}
