//! The compact blocks laid out again over ordinary SQLite tables, for the
//! search-fast round: what a layout made for reading buys over compact's.
//!
//! In `fast2.db`, built from compact.db and fast.db's seeds:
//!
//! - `lists`: one row a key with blocks, its 128-posting blocks one after
//!   another in one blob, each bit-packed: a byte of postings, a byte of the
//!   width of the gaps between documents less one, a byte of the width of the
//!   frequencies less one, then the gaps and the frequencies at those widths.
//!   A reader keeps one incremental-blob handle a list and reads a block at
//!   its offset; no statement runs a block.
//! - `dirs`: one row a key, 16 bytes a block, fixed width so that it is read
//!   without parsing: first and last document, the block's best saturation at
//!   the build's average length, rounded up to an f32, and the block's offset.
//!   At another average the bound is that times the new average over the
//!   build's when the new one is larger, which BM25's saturation never
//!   exceeds, and the bound stays above every score.
//! - `lengths`: one byte a document, 255 for a field of 255 tokens or more,
//!   whose length is in `long`.
//!
//! Switches, each off by default: `FAST2_WORDS=1` unpacks a block eight bytes
//! at a time; `FAST2_WALK=1` finds the bound of each passive list's block for
//! a window's candidates, which come in order, by stepping a cursor forward
//! instead of a binary search twice a candidate; `FAST2_DIRECT=1` adds an
//! essential list's postings from its decoded block without the cursor's
//! checks a posting; `FAST2_WINDOWS=1` finds each list's blocks for a window,
//! which come in order, by stepping a cursor forward instead of a binary
//! search a window; `FAST2_CACHE=1` keeps each key's directory, decoded, from
//! one query to the next in the reader, its bounds scaled again a query.
//! `FAST2_TABLE=1` (on by default) scores a list's postings of frequency below 16
//! and length below 255 from a table of its scores made when the list is
//! opened, the same arithmetic; `FAST_SEED`, `FAST_CACHE_KIB` and
//! `FAST_MMAP` as fast.rs reads them.

use std::cmp::Reverse;
use std::cell::RefCell;
use std::collections::{BinaryHeap, HashMap};
use std::rc::Rc;
use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::blocks::{END, FIELDS, get_varint, idf, key, saturated};
use crate::compact1::FIELD;
use crate::fts::open;

const NORMS: usize = 65_536;
const TF_TABLE: u32 = 16;
const LEN_TABLE: u32 = 255;

fn bits(value: u32) -> u8 {
    (32 - value.leading_zeros()).max(1) as u8
}

struct Packer {
    out: Vec<u8>,
    word: u64,
    filled: u32,
}

impl Packer {
    fn put(&mut self, value: u32, width: u8) {
        self.word |= u64::from(value) << self.filled;
        self.filled += u32::from(width);
        while self.filled >= 8 {
            self.out.push(self.word as u8);
            self.word >>= 8;
            self.filled -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.filled > 0 {
            self.out.push(self.word as u8);
        }
        self.out
    }
}

pub(crate) fn pack(docs: &[u32], tfs: &[u32]) -> Vec<u8> {
    let gaps: Vec<u32> = docs.windows(2).map(|pair| pair[1] - pair[0] - 1).collect();
    let gap_bits = bits(gaps.iter().copied().max().unwrap_or(0));
    let tf_bits = bits(tfs.iter().map(|tf| tf - 1).max().unwrap_or(0));
    let mut packer = Packer {
        out: vec![docs.len() as u8, gap_bits - 1, tf_bits - 1],
        word: 0,
        filled: 0,
    };
    for gap in &gaps {
        packer.put(*gap, gap_bits);
    }
    for tf in tfs {
        packer.put(tf - 1, tf_bits);
    }
    packer.finish()
}

/// A block's documents and frequencies, its first document from the directory.
fn unpack(bytes: &[u8], first: u32, docs: &mut Vec<u32>, tfs: &mut Vec<u32>) {
    let n = usize::from(bytes[0]);
    let gap_bits = u32::from(bytes[1]) + 1;
    let tf_bits = u32::from(bytes[2]) + 1;
    docs.clear();
    tfs.clear();
    let (mut word, mut filled, mut at) = (0u64, 0u32, 3usize);
    let mut take = |width: u32| -> u32 {
        while filled < width {
            word |= u64::from(bytes.get(at).copied().unwrap_or(0)) << filled;
            at += 1;
            filled += 8;
        }
        let value = (word & ((1u64 << width) - 1)) as u32;
        word >>= width;
        filled -= width;
        value
    };
    let mut doc = first;
    docs.push(doc);
    for _ in 1..n {
        doc += take(gap_bits) + 1;
        docs.push(doc);
    }
    for _ in 0..n {
        tfs.push(take(tf_bits) + 1);
    }
}

/// `unpack` reading eight bytes at a time: each value one unaligned load,
/// a shift and a mask. `bytes` ends with eight bytes of zeros.
pub(crate) fn unpack_words(bytes: &[u8], first: u32, docs: &mut Vec<u32>, tfs: &mut Vec<u32>) {
    let n = usize::from(bytes[0]);
    let gap_bits = u32::from(bytes[1]) + 1;
    let tf_bits = u32::from(bytes[2]) + 1;
    docs.clear();
    tfs.clear();
    let read = |bit: usize, width: u32| -> u32 {
        let at = 3 + bit / 8;
        let word = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        ((word >> (bit % 8)) & ((1u64 << width) - 1)) as u32
    };
    let (mut doc, mut bit) = (first, 0usize);
    docs.push(doc);
    for _ in 1..n {
        doc += read(bit, gap_bits) + 1;
        docs.push(doc);
        bit += gap_bits as usize;
    }
    for _ in 0..n {
        tfs.push(read(bit, tf_bits) + 1);
        bit += tf_bits as usize;
    }
}

pub(crate) fn unpack_varints(payload: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let (mut docs, mut tfs) = (Vec::new(), Vec::new());
    let (mut at, mut doc) = (0, 0);
    while at < payload.len() {
        doc += get_varint(payload, &mut at);
        docs.push(doc);
        tfs.push(get_varint(payload, &mut at));
    }
    (docs, tfs)
}

/// f32 no smaller than `value`.
pub(crate) fn up(value: f64) -> f32 {
    let narrow = value as f32;
    if f64::from(narrow) < value { narrow.next_up() } else { narrow }
}

/// Writes fast2.db from compact.db's blocks and lengths and fast.db's seeds.
pub fn build(compact: &Path, out_path: &Path) -> Value {
    let started = Instant::now();
    let source = open(compact);
    source
        .execute(&format!("attach database '{}' as f", compact.with_file_name("fast.db").display()), [])
        .unwrap();
    let mut lengths: Vec<u32> = Vec::new();
    {
        let mut rows = source.prepare("select chunk, lengths from norms order by chunk").unwrap();
        let mut read = rows.query([]).unwrap();
        while let Some(row) = read.next().unwrap() {
            let chunk: i64 = row.get(0).unwrap();
            let bytes: Vec<u8> = row.get(1).unwrap();
            let base = chunk as usize * NORMS;
            if lengths.len() < base + NORMS {
                lengths.resize(base + NORMS, 0);
            }
            for (i, doc) in bytes.chunks(8).enumerate() {
                lengths[base + i] = u32::from(u16::from_le_bytes([doc[FIELD * 2], doc[FIELD * 2 + 1]]));
            }
        }
    }
    let (tokens, documents): (i64, i64) = source
        .query_row("select tokens, documents from stats where field = ?1", [FIELD as i64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let average = (tokens as f64 / documents.max(1) as f64).max(1.0);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", out_path.display()));
    }
    let mut out = open(out_path);
    out.execute_batch(
        "create table lists (id integer primary key, payload blob not null);
         create table dirs (id integer primary key, directory blob not null);
         create table lengths (chunk integer primary key, bytes blob not null);
         create table long (doc integer primary key, len integer not null);
         create table meta (name text primary key, value real not null);",
    )
    .unwrap();
    let tx = out.transaction().unwrap();
    let (mut keys, mut blocks, mut list_bytes, mut long) = (0u64, 0u64, 0u64, 0u64);
    {
        let mut put_list = tx.prepare("insert into lists (id, payload) values (?1, ?2)").unwrap();
        let mut put_dir = tx.prepare("insert into dirs (id, directory) values (?1, ?2)").unwrap();
        let mut scan = source.prepare("select id, first, payload from cblob order by id, first").unwrap();
        let mut rows = scan.query([]).unwrap();
        let mut current = -1i64;
        let (mut payload, mut directory) = (Vec::new(), Vec::new());
        let mut flush = |id: i64, payload: &mut Vec<u8>, directory: &mut Vec<u8>| {
            put_list.execute(params![id, std::mem::take(payload)]).unwrap();
            put_dir.execute(params![id, std::mem::take(directory)]).unwrap();
        };
        while let Some(row) = rows.next().unwrap() {
            let id: i64 = row.get(0).unwrap();
            let raw: Vec<u8> = row.get(2).unwrap();
            if id != current {
                if current >= 0 {
                    flush(current, &mut payload, &mut directory);
                }
                current = id;
                keys += 1;
            }
            let (docs, tfs) = unpack_varints(&raw);
            let best = docs
                .iter()
                .zip(&tfs)
                .map(|(doc, tf)| saturated(*tf, lengths[*doc as usize], average))
                .fold(0.0, f64::max);
            let offset = payload.len() as u32;
            let packed = pack(&docs, &tfs);
            list_bytes += packed.len() as u64;
            payload.extend_from_slice(&packed);
            directory.extend_from_slice(&docs[0].to_le_bytes());
            directory.extend_from_slice(&docs[docs.len() - 1].to_le_bytes());
            directory.extend_from_slice(&up(best).to_le_bytes());
            directory.extend_from_slice(&offset.to_le_bytes());
            blocks += 1;
        }
        if current >= 0 {
            flush(current, &mut payload, &mut directory);
        }
        let mut put_len = tx.prepare("insert into lengths (chunk, bytes) values (?1, ?2)").unwrap();
        let mut put_long = tx.prepare("insert into long (doc, len) values (?1, ?2)").unwrap();
        for (chunk, slice) in lengths.chunks(NORMS).enumerate() {
            let bytes: Vec<u8> = slice.iter().map(|len| (*len).min(255) as u8).collect();
            for (i, len) in slice.iter().enumerate() {
                if *len >= 255 {
                    put_long.execute(params![(chunk * NORMS + i) as i64, *len as i64]).unwrap();
                    long += 1;
                }
            }
            put_len.execute(params![chunk as i64, bytes]).unwrap();
        }
        tx.execute("insert into meta (name, value) values ('average', ?1)", [average]).unwrap();
    }
    tx.commit().unwrap();
    out.pragma_update(None, "wal_checkpoint", "truncate").unwrap();
    let objects = crate::compact1::objects(&out);
    json!({"step": "build", "engine": "fast2", "seconds": started.elapsed().as_secs_f64(), "keys": keys,
        "blocks": blocks, "list_bytes": list_bytes, "long_lengths": long, "average": average,
        "file": std::fs::metadata(out_path).unwrap().len(), "objects": objects})
}

/// A list of a query: its directory, a blob handle on its blocks, the block
/// decoded, a cursor, and its scores' table.
struct Cursor<'a> {
    scale: f64,
    average: f64,
    firsts: Rc<Vec<u32>>,
    lasts: Rc<Vec<u32>>,
    bounds: Vec<f64>,
    offsets: Rc<Vec<u32>>,
    size: usize,
    wprobe: usize,
    blob: Option<rusqlite::blob::Blob<'a>>,
    whole: Option<(Vec<u32>, Vec<u32>)>,
    block: usize,
    decoded: usize,
    docs: Vec<u32>,
    tfs: Vec<u32>,
    at: usize,
    max: f64,
    table: Vec<f64>,
    words: bool,
}

#[derive(Default, Clone, Copy)]
pub struct Spent {
    pub lists: Duration,
    pub total: Duration,
    pub blocks: usize,
    pub scored: usize,
}

struct Lengths<'a> {
    short: &'a [u8],
    long: &'a HashMap<u32, u32>,
}

impl Lengths<'_> {
    #[inline]
    fn of(&self, doc: u32) -> u32 {
        let short = u32::from(self.short.get(doc as usize).copied().unwrap_or(0));
        if short == 255 { self.long[&doc] } else { short }
    }
}

impl Cursor<'_> {
    fn load(&mut self, spent: &mut Spent, buffer: &mut Vec<u8>) {
        if self.decoded == self.block {
            return;
        }
        if let Some((docs, tfs)) = &self.whole {
            self.docs.clone_from(docs);
            self.tfs.clone_from(tfs);
        } else {
            let start = self.offsets[self.block] as usize;
            let end = self.offsets.get(self.block + 1).map_or(self.size, |end| *end as usize);
            if self.words {
                buffer.clear();
                buffer.resize(end - start + 8, 0);
                self.blob.as_ref().unwrap().read_at_exact(&mut buffer[..end - start], start).unwrap();
                unpack_words(buffer, self.firsts[self.block], &mut self.docs, &mut self.tfs);
            } else {
                buffer.resize(end - start, 0);
                self.blob.as_ref().unwrap().read_at_exact(buffer, start).unwrap();
                unpack(buffer, self.firsts[self.block], &mut self.docs, &mut self.tfs);
            }
            spent.blocks += 1;
        }
        self.decoded = self.block;
    }

    fn doc(&mut self, spent: &mut Spent, buffer: &mut Vec<u8>) -> u32 {
        loop {
            if self.block >= self.firsts.len() {
                return END;
            }
            self.load(spent, buffer);
            if self.at < self.docs.len() {
                return self.docs[self.at];
            }
            self.block += 1;
            self.at = 0;
        }
    }

    fn seek(&mut self, doc: u32, spent: &mut Spent, buffer: &mut Vec<u8>) -> u32 {
        while self.block < self.lasts.len() && self.lasts[self.block] < doc {
            self.block += 1;
            self.at = 0;
        }
        if self.block >= self.lasts.len() {
            return END;
        }
        self.load(spent, buffer);
        while self.at < self.docs.len() && self.docs[self.at] < doc {
            self.at += 1;
        }
        self.doc(spent, buffer)
    }

    fn bound_at(&self, doc: u32) -> f64 {
        let from = self.block.min(self.lasts.len());
        let at = from + self.lasts[from..].partition_point(|last| *last < doc);
        if at < self.lasts.len() && self.firsts[at] <= doc { self.bounds[at] } else { 0.0 }
    }

    fn bound_over(&self, start: u32, end: u32) -> f64 {
        let from = self.block.min(self.lasts.len());
        let mut at = from + self.lasts[from..].partition_point(|last| *last < start);
        let mut best = 0.0f64;
        while at < self.firsts.len() && self.firsts[at] < end {
            best = best.max(self.bounds[at]);
            at += 1;
        }
        best
    }

    fn next_from(&self, end: u32) -> u32 {
        let from = self.block.min(self.lasts.len());
        let at = from + self.lasts[from..].partition_point(|last| *last < end);
        self.firsts.get(at).map_or(END, |first| (*first).max(end))
    }

    /// `bound_over` for windows that only move forward: the first block that
    /// could hold the window's documents found by stepping from the last.
    fn bound_over_walk(&mut self, start: u32, end: u32) -> f64 {
        let mut at = self.wprobe.max(self.block).min(self.lasts.len());
        while at < self.lasts.len() && self.lasts[at] < start {
            at += 1;
        }
        self.wprobe = at;
        let mut best = 0.0f64;
        while at < self.firsts.len() && self.firsts[at] < end {
            best = best.max(self.bounds[at]);
            at += 1;
        }
        best
    }

    fn next_from_walk(&mut self, end: u32) -> u32 {
        let mut at = self.wprobe.max(self.block).min(self.lasts.len());
        while at < self.lasts.len() && self.lasts[at] < end {
            at += 1;
        }
        self.wprobe = at;
        self.firsts.get(at).map_or(END, |first| (*first).max(end))
    }

    #[inline]
    fn score(&self, lengths: &Lengths) -> f64 {
        let doc = self.docs[self.at];
        let tf = self.tfs[self.at];
        let len = lengths.of(doc);
        if !self.table.is_empty() && tf < TF_TABLE && len < LEN_TABLE {
            return self.table[(tf * LEN_TABLE + len) as usize];
        }
        self.scale * saturated(tf, len, self.average)
    }
}

pub struct Reader {
    connection: Connection,
    documents: u64,
    average: f64,
    built_average: f64,
    short: Vec<u8>,
    long: HashMap<u32, u32>,
    seed: bool,
    table: bool,
    words: bool,
    walk: bool,
    direct: bool,
    windows: bool,
    cache: Option<RefCell<HashMap<i64, Rc<Dir>>>>,
    pub spent: Spent,
}

/// A key's directory decoded, its bounds as the build left them.
struct Dir {
    firsts: Rc<Vec<u32>>,
    lasts: Rc<Vec<u32>>,
    raw: Vec<f64>,
    offsets: Rc<Vec<u32>>,
}

fn decode_dir(directory: &[u8]) -> Dir {
    let n = directory.len() / 16;
    let (mut firsts, mut lasts, mut raw, mut offsets) =
        (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    for entry in directory.chunks_exact(16) {
        let word = |at: usize| u32::from_le_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]]);
        firsts.push(word(0));
        lasts.push(word(4));
        raw.push(f64::from(f32::from_bits(word(8))));
        offsets.push(word(12));
    }
    Dir {
        firsts: Rc::new(firsts),
        lasts: Rc::new(lasts),
        raw,
        offsets: Rc::new(offsets),
    }
}

impl Reader {
    pub fn open(compact: &Path) -> Reader {
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
        let connection = open(compact);
        for (schema, file) in [("f", "fast.db"), ("g", "fast2.db")] {
            connection
                .execute(&format!("attach database '{}' as {schema}", compact.with_file_name(file).display()), [])
                .unwrap();
        }
        for schema in ["main", "f", "g"] {
            connection
                .execute_batch(&format!(
                    "pragma {schema}.cache_size = -{}; pragma {schema}.mmap_size = {};",
                    var("FAST_CACHE_KIB", "1024"),
                    var("FAST_MMAP", "0")
                ))
                .unwrap();
        }
        connection.pragma_update(None, "query_only", true).unwrap();
        let (tokens, holding): (i64, i64) = connection
            .query_row("select tokens, documents from stats where field = ?1", [FIELD as i64], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        let built_average: f64 = connection
            .query_row("select value from g.meta where name = 'average'", [], |row| row.get(0))
            .unwrap();
        let mut short = Vec::new();
        {
            let mut rows = connection.prepare("select chunk, bytes from g.lengths order by chunk").unwrap();
            let mut read = rows.query([]).unwrap();
            while let Some(row) = read.next().unwrap() {
                let chunk: i64 = row.get(0).unwrap();
                let bytes: Vec<u8> = row.get(1).unwrap();
                short.resize(chunk as usize * NORMS, 0);
                short.extend_from_slice(&bytes);
            }
        }
        let long: HashMap<u32, u32> = connection
            .prepare("select doc, len from g.long")
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, i64>(0)? as u32, row.get::<_, i64>(1)? as u32)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        Reader {
            connection,
            documents: holding as u64,
            average: (tokens as f64 / holding.max(1) as f64).max(1.0),
            built_average,
            short,
            long,
            seed: var("FAST_SEED", "1") == "1",
            table: var("FAST2_TABLE", "1") == "1",
            words: var("FAST2_WORDS", "0") == "1",
            walk: var("FAST2_WALK", "0") == "1",
            direct: var("FAST2_DIRECT", "0") == "1",
            windows: var("FAST2_WINDOWS", "0") == "1",
            cache: (var("FAST2_CACHE", "0") == "1").then(|| RefCell::new(HashMap::new())),
            spent: Spent::default(),
        }
    }

    fn cursors(&self, terms: &[String], k: usize) -> (Vec<Cursor<'_>>, f64) {
        // with the cache, a directory is read only when it is not kept
        let mut find = self
            .connection
            .prepare_cached(if self.cache.is_some() {
                "select c.id, c.documents, c.inline, null, f.seeds, null \
                 from ckeys c left join f.fdir f on f.id = c.id where c.key = ?1"
            } else {
                "select c.id, c.documents, c.inline, d.directory, f.seeds, l.rowid \
                 from ckeys c left join g.dirs d on d.id = c.id left join f.fdir f on f.id = c.id \
                 left join g.lists l on l.id = c.id where c.key = ?1"
            })
            .unwrap();
        let lengths = Lengths {
            short: &self.short,
            long: &self.long,
        };
        let growth = (self.average / self.built_average).max(1.0) * (1.0 + 4.0 * f64::EPSILON);
        let mut cursors = Vec::new();
        let mut seed = 0.0f64;
        type Found = (i64, u64, Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>, Option<i64>);
        for term in terms {
            let found: Option<Found> = find
                .query_row([key(FIELD, term)], |row| {
                    Ok((row.get(0)?, row.get::<_, i64>(1)? as u64, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
                })
                .optional()
                .unwrap();
            let Some((id, holding, inline, directory, seeds, _)) = found else {
                continue;
            };
            let scale = FIELDS[FIELD].1 * idf(self.documents, holding.max(1));
            let average = self.average;
            let mut table = Vec::new();
            if self.table {
                table = Vec::with_capacity((TF_TABLE * LEN_TABLE) as usize);
                for tf in 0..TF_TABLE {
                    for len in 0..LEN_TABLE {
                        table.push(scale * saturated(tf, len, average));
                    }
                }
            }
            let tenth = |mut scores: Vec<f64>| -> f64 {
                if scores.len() < k {
                    return 0.0;
                }
                scores.sort_unstable_by(|a, b| b.total_cmp(a));
                scores[k - 1]
            };
            if let Some(inline) = inline {
                let (mut docs, mut tfs) = (Vec::new(), Vec::new());
                let (mut at, mut doc) = (0, 0);
                while at < inline.len() {
                    doc += get_varint(&inline, &mut at);
                    docs.push(doc);
                    tfs.push(get_varint(&inline, &mut at));
                    get_varint(&inline, &mut at);
                }
                let scores: Vec<f64> = docs
                    .iter()
                    .zip(&tfs)
                    .map(|(doc, tf)| scale * saturated(*tf, lengths.of(*doc), average))
                    .collect();
                let best = scores.iter().copied().fold(0.0, f64::max);
                if self.seed {
                    seed = seed.max(tenth(scores));
                }
                cursors.push(Cursor {
                    scale,
                    average,
                    firsts: Rc::new(vec![docs[0]]),
                    lasts: Rc::new(vec![docs[docs.len() - 1]]),
                    bounds: vec![best],
                    offsets: Rc::new(vec![0]),
                    size: 0,
                    wprobe: 0,
                    blob: None,
                    whole: Some((docs, tfs)),
                    block: 0,
                    decoded: usize::MAX,
                    docs: Vec::new(),
                    tfs: Vec::new(),
                    at: 0,
                    max: best,
                    table,
                    words: self.words,
                });
                continue;
            }
            let cached = self.cache.as_ref().and_then(|cache| cache.borrow().get(&id).cloned());
            let dir = match (cached, directory) {
                (Some(dir), _) => dir,
                (None, Some(directory)) => {
                    let dir = Rc::new(decode_dir(&directory));
                    if let Some(cache) = &self.cache {
                        cache.borrow_mut().insert(id, Rc::clone(&dir));
                    }
                    dir
                }
                (None, None) if self.cache.is_some() => {
                    let directory: Option<Vec<u8>> = self
                        .connection
                        .prepare_cached("select directory from g.dirs where id = ?1")
                        .unwrap()
                        .query_row([id], |row| row.get(0))
                        .optional()
                        .unwrap();
                    let Some(directory) = directory else { continue };
                    let dir = Rc::new(decode_dir(&directory));
                    self.cache.as_ref().unwrap().borrow_mut().insert(id, Rc::clone(&dir));
                    dir
                }
                (None, None) => continue,
            };
            let (firsts, lasts, offsets) = (Rc::clone(&dir.firsts), Rc::clone(&dir.lasts), Rc::clone(&dir.offsets));
            let bounds: Vec<f64> = dir.raw.iter().map(|raw| scale * raw * growth).collect();
            if self.seed
                && let Some(seeds) = seeds
            {
                let mut at = 0;
                let mut scores = Vec::new();
                while at < seeds.len() {
                    get_varint(&seeds, &mut at);
                    let tf = get_varint(&seeds, &mut at);
                    let len = get_varint(&seeds, &mut at);
                    scores.push(scale * saturated(tf, len, average));
                }
                seed = seed.max(tenth(scores));
            }
            let blob = self.connection.blob_open("g", "lists", "payload", id, true).unwrap();
            let size = blob.len();
            let max = bounds.iter().copied().fold(0.0, f64::max);
            cursors.push(Cursor {
                scale,
                average,
                firsts,
                lasts,
                bounds,
                offsets,
                size,
                wprobe: 0,
                blob: Some(blob),
                whole: None,
                block: 0,
                decoded: usize::MAX,
                docs: Vec::with_capacity(128),
                tfs: Vec::with_capacity(128),
                at: 0,
                max,
                table,
                words: self.words,
            });
        }
        (cursors, seed)
    }

    pub fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        let started = Instant::now();
        let (mut cursors, seed) = self.cursors(terms, k);
        let mut spent = Spent {
            lists: started.elapsed(),
            ..Spent::default()
        };
        let lengths = Lengths {
            short: &self.short,
            long: &self.long,
        };
        let found = windowed(&mut cursors, &lengths, k, seed, self.walk, self.direct, self.windows, &mut spent);
        drop(cursors);
        spent.total = started.elapsed();
        self.spent = spent;
        found
    }
}

#[derive(PartialEq, PartialOrd)]
struct Scored(f64, Reverse<u32>);

impl Eq for Scored {}

impl Ord for Scored {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

const WINDOW: u32 = 4096;

/// fast.rs's `windowed` over these cursors.
fn windowed(
    lists: &mut [Cursor],
    lengths: &Lengths,
    k: usize,
    seed: f64,
    walk: bool,
    direct: bool,
    windows: bool,
    spent: &mut Spent,
) -> Vec<(f64, u32)> {
    let floor = if seed > 0.0 { f64::from_bits(seed.to_bits() - 1) } else { 0.0 };
    let theta_of = |best: &BinaryHeap<Reverse<Scored>>| -> Option<f64> {
        let heap = (best.len() >= k).then(|| best.peek().unwrap().0.0);
        match (heap, floor > 0.0) {
            (Some(theta), _) => Some(theta.max(floor)),
            (None, true) => Some(floor),
            (None, false) => None,
        }
    };
    let mut buffer = Vec::new();
    let mut best: BinaryHeap<Reverse<Scored>> = BinaryHeap::new();
    let mut scores = vec![0.0f64; WINDOW as usize];
    let mut touched: Vec<u32> = Vec::new();
    let mut probes: Vec<usize> = Vec::new();
    let mut passive_bounds: Vec<f64> = Vec::new();
    let mut start = lists
        .iter_mut()
        .map(|list| if windows { list.next_from_walk(0) } else { list.next_from(0) })
        .min()
        .unwrap_or(END);
    while start != END {
        let end = start.saturating_add(WINDOW);
        let bounds: Vec<f64> = lists
            .iter_mut()
            .map(|list| if windows { list.bound_over_walk(start, end) } else { list.bound_over(start, end) })
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
                let mut doc = list.seek(start, spent, &mut buffer);
                while doc < end {
                    if direct {
                        while list.at < list.docs.len() {
                            let doc = list.docs[list.at];
                            if doc >= end {
                                break;
                            }
                            let slot = (doc - start) as usize;
                            if scores[slot] == 0.0 {
                                touched.push(doc);
                            }
                            scores[slot] += list.score(lengths);
                            spent.scored += 1;
                            list.at += 1;
                        }
                    } else {
                        let slot = (doc - start) as usize;
                        if scores[slot] == 0.0 {
                            touched.push(doc);
                        }
                        scores[slot] += list.score(lengths);
                        spent.scored += 1;
                        list.at += 1;
                    }
                    doc = list.doc(spent, &mut buffer);
                }
            }
            touched.sort_unstable();
            let window_rest: f64 = order[..passive].iter().map(|at| bounds[*at]).sum();
            if walk {
                probes.clear();
                for &at in &order[..passive] {
                    let list = &lists[at];
                    let from = list.block.min(list.lasts.len());
                    probes.push(from + list.lasts[from..].partition_point(|last| *last < start));
                }
            }
            for &doc in &touched {
                let mut score = std::mem::take(&mut scores[(doc - start) as usize]);
                let theta = theta_of(&best);
                let full = theta.is_some();
                let theta = theta.unwrap_or(0.0);
                if full && score + window_rest <= theta {
                    continue;
                }
                passive_bounds.clear();
                if walk {
                    for (j, &at) in order[..passive].iter().enumerate() {
                        let list = &lists[at];
                        let mut probe = probes[j];
                        while probe < list.lasts.len() && list.lasts[probe] < doc {
                            probe += 1;
                        }
                        probes[j] = probe;
                        passive_bounds.push(if probe < list.lasts.len() && list.firsts[probe] <= doc {
                            list.bounds[probe]
                        } else {
                            0.0
                        });
                    }
                } else {
                    passive_bounds.extend(order[..passive].iter().map(|at| lists[*at].bound_at(doc)));
                }
                let mut rest: f64 = passive_bounds.iter().sum();
                for (j, &at) in order[..passive].iter().enumerate().rev() {
                    if full && score + rest <= theta {
                        break;
                    }
                    // without the walk, the bound found again as compact1's executor finds it
                    let bound = if walk { passive_bounds[j] } else { lists[at].bound_at(doc) };
                    rest -= bound;
                    if bound > 0.0 && lists[at].seek(doc, spent, &mut buffer) == doc {
                        score += lists[at].score(lengths);
                        spent.scored += 1;
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
            .iter_mut()
            .map(|list| if windows { list.next_from_walk(end) } else { list.next_from(end) })
            .min()
            .unwrap_or(END);
    }
    let mut found: Vec<(f64, u32)> =
        best.into_iter().map(|Reverse(Scored(score, Reverse(doc)))| (score, doc)).collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    found
}
