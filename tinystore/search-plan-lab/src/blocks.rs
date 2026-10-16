//! Postings of our own over SQLite's pages: each field's term a list of
//! blocks of up to 128 documents, packed, and a directory beside them that
//! a query reads first. A block's largest frequency and shortest field bound
//! what its term can add to any of its documents' scores, whatever the
//! collection's statistics have become since, so a query answers its top ten
//! by block-max MaxScore and never opens a block that cannot reach it.
//!
//! ```text
//! skip  (key, first) → last, most_tf, least_len   the directory, read first
//! post  (key, first) → payload                     (doc delta, tf, len) a document, varints
//! terms (key)        → documents                   for idf and the planner
//! head  (doc, key)   → tf, len                     what sets wrote since the blocks were built
//! gone  (doc)                                       documents in blocks a set replaced
//! ```
//!
//! LAB_FORMAT=packed builds the same blocks with fewer rows to read: a key
//! is a number, and its directory one row a chunk of the build rather than
//! a row a block.
//!
//! ```text
//! keys (key)        → id, documents
//! dirs (id, chunk)  → directory     (first delta, last − first, most_tf, least_len) a block, varints
//! blob (id, first)  → payload
//! ```
//!
//! A key is a field and a term, `b|война`. The fields and their weights are
//! FTS5's columns and Tantivy's boosts in this lab, and a document's score
//! is the sum over its keys of weight × idf × BM25's saturated frequency.
//! Positions are not kept: no phrase can be asked of these blocks.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use rusqlite::{CachedStatement, Connection, OptionalExtension, params};

use crate::analyze::Analyzer;
use crate::corpus::Doc;
use crate::fts::open;
use crate::plan::{Backend, Word};

const BLOCK: usize = 128;
/// Documents built into blocks at a time: a key's documents in one chunk
/// make its blocks, and its last block of each chunk may be short.
const CHUNK: usize = 250_000;
const K1: f64 = 1.2;
const B: f64 = 0.75;
const END: u32 = u32::MAX;

/// The fields and their weights: a title's words, its stems, a body's, its stems.
const FIELDS: [(&str, f64); 4] = [("t", 2.0), ("ts", 1.5), ("b", 1.0), ("bs", 0.75)];

const SCHEMA: &str = "
create table skip (key text not null, first integer not null, last integer not null,
  most_tf integer not null, least_len integer not null, primary key (key, first)) without rowid;
create table post (key text not null, first integer not null, payload blob not null,
  primary key (key, first)) without rowid;
create table terms (key text primary key, documents integer not null) without rowid;
create table stats (field integer primary key, tokens integer not null, documents integer not null);
create table docs (id integer primary key, key text not null unique, title text not null, body text not null);
create table head (doc integer not null, key text not null, tf integer not null, len integer not null,
  primary key (doc, key)) without rowid;
create index head_by_key on head (key, doc);
create table gone (doc integer primary key);
create table keys (key text primary key, id integer not null, documents integer not null) without rowid;
create table dirs (id integer not null, chunk integer not null, directory blob not null,
  primary key (id, chunk)) without rowid;
create table blob (id integer not null, first integer not null, payload blob not null,
  primary key (id, first)) without rowid;
";

fn key(field: usize, term: &str) -> String {
    format!("{}|{term}", FIELDS[field].0)
}

/// BM25's saturated frequency: what a term's `tf` in a field of `len`
/// tokens adds, before its weight and idf.
fn saturated(tf: u32, len: u32, average: f64) -> f64 {
    let tf = f64::from(tf);
    tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * f64::from(len) / average))
}

fn idf(documents: u64, holding: u64) -> f64 {
    let (n, df) = (documents as f64, holding as f64);
    (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
}

/// A document's four fields as tokens, as FTS5's four columns hold them.
fn fields(analyzer: &Analyzer, doc: &Doc) -> [Vec<String>; 4] {
    let title = analyzer.tokens(&doc.title);
    let body = analyzer.tokens(&doc.text);
    let surface = |tokens: &[crate::analyze::Token]| {
        tokens.iter().map(|token| token.surface.clone()).collect()
    };
    let stems = |tokens: &[crate::analyze::Token]| {
        tokens
            .iter()
            .filter_map(|token| token.stem.clone())
            .collect()
    };
    [surface(&title), stems(&title), surface(&body), stems(&body)]
}

/// Each term of a field once, with how often it occurs.
fn counted(terms: &[String]) -> Vec<(&str, u32)> {
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for term in terms {
        *counts.entry(term.as_str()).or_default() += 1;
    }
    counts.into_iter().collect()
}

fn put_varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn get_varint(bytes: &[u8], at: &mut usize) -> u32 {
    let mut value = 0u32;
    let mut shift = 0;
    loop {
        let byte = bytes[*at];
        *at += 1;
        value |= u32::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return value;
        }
        shift += 7;
    }
}

#[derive(Clone, Copy)]
struct Posting {
    doc: u32,
    tf: u32,
    len: u32,
}

fn pack(block: &[Posting]) -> Vec<u8> {
    let mut out = Vec::with_capacity(block.len() * 4);
    let mut previous = 0;
    for posting in block {
        put_varint(&mut out, posting.doc - previous);
        put_varint(&mut out, posting.tf);
        put_varint(&mut out, posting.len);
        previous = posting.doc;
    }
    out
}

/// A block's postings, those of a document a set replaced left out.
fn unpack(payload: &[u8], gone: &HashSet<u32>) -> Vec<Posting> {
    let mut postings = Vec::with_capacity(BLOCK);
    let (mut at, mut doc) = (0, 0);
    while at < payload.len() {
        doc += get_varint(payload, &mut at);
        let tf = get_varint(payload, &mut at);
        let len = get_varint(payload, &mut at);
        if !gone.contains(&doc) {
            postings.push(Posting { doc, tf, len });
        }
    }
    postings
}

pub struct Built {
    pub seconds: f64,
    pub objects: Vec<(String, u64)>,
    pub file: u64,
    pub postings: u64,
    pub blocks: u64,
}

pub fn build(path: &Path, analyzer: &Analyzer, docs: &[Doc]) -> Built {
    let started = Instant::now();
    let mut connection = open(path);
    connection.execute_batch(SCHEMA).unwrap();
    let packed = std::env::var("LAB_FORMAT").as_deref() == Ok("packed");
    let mut tokens = [0u64; 4];
    let (mut postings, mut blocks) = (0u64, 0u64);
    // A packed build's keys: each one's number and the documents it is in.
    let mut ids: HashMap<String, (i64, u64)> = HashMap::new();
    for (chunk, docs) in docs.chunks(CHUNK).enumerate() {
        let tx = connection.transaction().unwrap();
        let mut lists: HashMap<String, Vec<Posting>> = HashMap::new();
        {
            let mut keep = tx
                .prepare("insert into docs (id, key, title, body) values (?1, ?2, ?3, ?4)")
                .unwrap();
            for (at, doc) in docs.iter().enumerate() {
                let id = (chunk * CHUNK + at + 1) as u32;
                for (field, terms) in fields(analyzer, doc).iter().enumerate() {
                    tokens[field] += terms.len() as u64;
                    let len = terms.len() as u32;
                    for (term, tf) in counted(terms) {
                        lists.entry(key(field, term)).or_default().push(Posting {
                            doc: id,
                            tf,
                            len,
                        });
                    }
                }
                keep.execute(params![id, doc.id, doc.title, doc.text])
                    .unwrap();
            }
            if packed {
                let (added, blocked) = write_packed(&tx, chunk, &lists, &mut ids);
                postings += added;
                blocks += blocked;
                lists.clear();
            }
            let mut keys: Vec<&String> = lists.keys().collect();
            keys.sort();
            let mut skip = tx
                .prepare("insert into skip (key, first, last, most_tf, least_len) values (?1, ?2, ?3, ?4, ?5)")
                .unwrap();
            let mut post = tx
                .prepare("insert into post (key, first, payload) values (?1, ?2, ?3)")
                .unwrap();
            let mut count = tx
                .prepare(
                    "insert into terms (key, documents) values (?1, ?2)
                     on conflict (key) do update set documents = documents + excluded.documents",
                )
                .unwrap();
            for key in keys {
                let list = &lists[key];
                postings += list.len() as u64;
                for block in list.chunks(BLOCK) {
                    blocks += 1;
                    let most_tf = block.iter().map(|posting| posting.tf).max().unwrap();
                    let least_len = block.iter().map(|posting| posting.len).min().unwrap();
                    skip.execute(params![
                        key,
                        block[0].doc,
                        block[block.len() - 1].doc,
                        most_tf,
                        least_len
                    ])
                    .unwrap();
                    post.execute(params![key, block[0].doc, pack(block)])
                        .unwrap();
                }
                count.execute(params![key, list.len() as i64]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    if packed {
        let tx = connection.transaction().unwrap();
        {
            let mut insert = tx
                .prepare("insert into keys (key, id, documents) values (?1, ?2, ?3)")
                .unwrap();
            let mut all: Vec<(&String, &(i64, u64))> = ids.iter().collect();
            all.sort();
            for (key, (id, documents)) in all {
                insert.execute(params![key, id, *documents as i64]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    for (field, tokens) in tokens.iter().enumerate() {
        connection
            .execute(
                "insert into stats (field, tokens, documents) values (?1, ?2, ?3)",
                params![field as i64, *tokens as i64, docs.len() as i64],
            )
            .unwrap();
    }
    connection
        .pragma_update(None, "wal_checkpoint", "truncate")
        .unwrap();
    let seconds = started.elapsed().as_secs_f64();
    let objects = connection
        .prepare("select name, sum(pgsize) from dbstat group by name order by 2 desc")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    drop(connection);
    Built {
        seconds,
        objects,
        file: std::fs::metadata(path).unwrap().len(),
        postings,
        blocks,
    }
}

/// Writes a chunk's lists as packed rows: a key's number, given the first
/// time it is met, one directory row and its blocks; says the postings and
/// blocks it wrote.
fn write_packed(
    tx: &rusqlite::Transaction,
    chunk: usize,
    lists: &HashMap<String, Vec<Posting>>,
    ids: &mut HashMap<String, (i64, u64)>,
) -> (u64, u64) {
    let mut numbered: Vec<(i64, &String)> = lists
        .keys()
        .map(|key| {
            let next = ids.len() as i64 + 1;
            let entry = ids.entry(key.clone()).or_insert((next, 0));
            entry.1 += lists[key].len() as u64;
            (entry.0, key)
        })
        .collect();
    numbered.sort();
    let mut dir = tx
        .prepare("insert into dirs (id, chunk, directory) values (?1, ?2, ?3)")
        .unwrap();
    let mut blob = tx
        .prepare("insert into blob (id, first, payload) values (?1, ?2, ?3)")
        .unwrap();
    let (mut postings, mut blocks) = (0, 0);
    for (id, key) in numbered {
        let list = &lists[key];
        postings += list.len() as u64;
        let mut directory = Vec::new();
        let mut previous = 0;
        for block in list.chunks(BLOCK) {
            blocks += 1;
            let (first, last) = (block[0].doc, block[block.len() - 1].doc);
            put_varint(&mut directory, first - previous);
            put_varint(&mut directory, last - first);
            put_varint(
                &mut directory,
                block.iter().map(|posting| posting.tf).max().unwrap(),
            );
            put_varint(
                &mut directory,
                block.iter().map(|posting| posting.len).min().unwrap(),
            );
            previous = last;
            blob.execute(params![id, first, pack(block)]).unwrap();
        }
        dir.execute(params![id, chunk as i64, directory]).unwrap();
    }
    (postings, blocks)
}

/// A block as the directory says it: its documents' range and the most its
/// key adds to any of them.
struct Skip {
    first: u32,
    last: u32,
    bound: f64,
}

/// A key's postings in a query: its directory, the block its cursor is in,
/// decoded, and the cursor.
struct List {
    key: String,
    /// The key's number in a packed index, -1 in the other.
    id: i64,
    /// The key's weight × idf.
    scale: f64,
    average: f64,
    skips: Vec<Skip>,
    max: f64,
    block: usize,
    decoded: usize,
    postings: Vec<Posting>,
    at: usize,
}

/// Where a block's payload comes from.
struct Source<'a> {
    payload: CachedStatement<'a>,
    gone: &'a HashSet<u32>,
    opened: usize,
    /// Time spent finding and unpacking blocks.
    opening: std::time::Duration,
}

impl List {
    /// The cursor's document, the next block opened when this one is done.
    fn doc(&mut self, source: &mut Source) -> u32 {
        loop {
            if self.block >= self.skips.len() {
                return END;
            }
            self.open(source);
            if self.at < self.postings.len() {
                return self.postings[self.at].doc;
            }
            self.block += 1;
            self.at = 0;
        }
    }

    fn open(&mut self, source: &mut Source) {
        if self.decoded == self.block {
            return;
        }
        let started = Instant::now();
        let first = self.skips[self.block].first;
        let payload: Vec<u8> = if self.id >= 0 {
            source
                .payload
                .query_row(params![self.id, first], |row| row.get(0))
        } else {
            source
                .payload
                .query_row(params![self.key, first], |row| row.get(0))
        }
        .unwrap();
        self.postings = unpack(&payload, source.gone);
        self.decoded = self.block;
        source.opened += 1;
        source.opening += started.elapsed();
    }

    fn next(&mut self) {
        self.at += 1;
    }

    /// Moves to the first posting at `doc` or after it, passing whole blocks
    /// before it unopened.
    fn seek(&mut self, doc: u32, source: &mut Source) -> u32 {
        while self.block < self.skips.len() && self.skips[self.block].last < doc {
            self.block += 1;
            self.at = 0;
        }
        if self.block >= self.skips.len() {
            return END;
        }
        self.open(source);
        while self.at < self.postings.len() && self.postings[self.at].doc < doc {
            self.at += 1;
        }
        self.doc(source)
    }

    /// The bound of the block that could hold `doc`, nothing when none could.
    fn bound_at(&self, doc: u32) -> f64 {
        let rest = &self.skips[self.block.min(self.skips.len())..];
        let at = rest.partition_point(|skip| skip.last < doc);
        rest.get(at)
            .filter(|skip| skip.first <= doc)
            .map_or(0.0, |skip| skip.bound)
    }

    fn current_bound(&self) -> f64 {
        self.skips[self.block].bound
    }

    /// The most the key adds to any document in `[start, end)`.
    fn bound_over(&self, start: u32, end: u32) -> f64 {
        let rest = &self.skips[self.block.min(self.skips.len())..];
        let from = rest.partition_point(|skip| skip.last < start);
        rest[from..]
            .iter()
            .take_while(|skip| skip.first < end)
            .map(|skip| skip.bound)
            .fold(0.0, f64::max)
    }

    /// A document no later than the key's first at `end` or after it, read
    /// from the directory without opening a block.
    fn next_from(&self, end: u32) -> u32 {
        let rest = &self.skips[self.block.min(self.skips.len())..];
        let from = rest.partition_point(|skip| skip.last < end);
        rest.get(from).map_or(END, |skip| skip.first.max(end))
    }

    fn score(&self) -> f64 {
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

/// The best `k` of every document the lists hold, by block-max MaxScore.
///
/// The lists are ordered by the most each can add. While the `k`th best
/// score is θ, the lists whose maxima together reach no more than θ are
/// non-essential: a document only they hold cannot enter, so candidates come
/// from the others, and a non-essential list is asked about a candidate only
/// while the bounds of the blocks that could hold it might still lift it
/// past θ.
fn max_score(lists: &mut [List], source: &mut Source, k: usize) -> Vec<(f64, u32)> {
    lists.sort_by(|a, b| a.max.total_cmp(&b.max));
    let prefix: Vec<f64> = lists
        .iter()
        .scan(0.0, |sum, list| {
            *sum += list.max;
            Some(*sum)
        })
        .collect();
    let mut best: BinaryHeap<Reverse<Scored>> = BinaryHeap::new();
    loop {
        let theta = if best.len() < k {
            0.0
        } else {
            best.peek().unwrap().0.0
        };
        let essential = if best.len() < k {
            0
        } else {
            prefix.partition_point(|sum| *sum <= theta)
        };
        if essential == lists.len() {
            break;
        }
        let mut doc = END;
        for list in &mut lists[essential..] {
            doc = doc.min(list.doc(source));
        }
        if doc == END {
            break;
        }
        let mut bound = 0.0;
        for list in &mut lists[essential..] {
            if list.doc(source) == doc {
                bound += list.current_bound();
            }
        }
        let mut rest: f64 = lists[..essential]
            .iter()
            .map(|list| list.bound_at(doc))
            .sum();
        let full = best.len() >= k;
        if full && bound + rest <= theta {
            for list in &mut lists[essential..] {
                if list.doc(source) == doc {
                    list.next();
                }
            }
            continue;
        }
        let mut score = 0.0;
        for list in &mut lists[essential..] {
            if list.doc(source) == doc {
                score += list.score();
                list.next();
            }
        }
        for list in lists[..essential].iter_mut().rev() {
            if full && score + rest <= theta {
                break;
            }
            let bound = list.bound_at(doc);
            rest -= bound;
            if bound > 0.0 && list.seek(doc, source) == doc {
                score += list.score();
            }
        }
        if !full {
            best.push(Reverse(Scored(score, Reverse(doc))));
        } else if score > theta {
            best.pop();
            best.push(Reverse(Scored(score, Reverse(doc))));
        }
    }
    sorted(
        best.into_iter()
            .map(|Reverse(Scored(score, Reverse(doc)))| (score, doc))
            .collect(),
    )
}

/// Documents a window of `windowed` holds.
const WINDOW: u32 = 4096;

/// The best `k` by block-max MaxScore a window of documents at a time, as
/// Lucene's MaxScoreBulkScorer goes: in each window the lists are split into
/// essential and not by the most each adds inside it, and a window no list
/// can lift past θ is passed unread; the essential lists add their postings
/// into an array one list after another, and the documents it holds are
/// completed from the other lists in order.
fn windowed(lists: &mut [List], source: &mut Source, k: usize) -> Vec<(f64, u32)> {
    let mut best: BinaryHeap<Reverse<Scored>> = BinaryHeap::new();
    let mut scores = vec![0.0f64; WINDOW as usize];
    let mut touched: Vec<u32> = Vec::new();
    let mut start = lists
        .iter()
        .map(|list| list.next_from(0))
        .min()
        .unwrap_or(END);
    while start != END {
        let end = start.saturating_add(WINDOW);
        let bounds: Vec<f64> = lists
            .iter()
            .map(|list| list.bound_over(start, end))
            .collect();
        let mut order: Vec<usize> = (0..lists.len()).collect();
        order.sort_by(|a, b| bounds[*a].total_cmp(&bounds[*b]));
        // The lists whose window maxima together cannot lift a document past θ.
        let passive = if best.len() < k {
            0
        } else {
            let theta = best.peek().unwrap().0.0;
            let mut sum = 0.0;
            order
                .iter()
                .take_while(|list| {
                    sum += bounds[**list];
                    sum <= theta
                })
                .count()
        };
        if passive < order.len() {
            for &at in &order[passive..] {
                let list = &mut lists[at];
                let mut doc = list.seek(start, source);
                while doc < end {
                    let slot = (doc - start) as usize;
                    if scores[slot] == 0.0 {
                        touched.push(doc);
                    }
                    scores[slot] += list.score();
                    list.next();
                    doc = list.doc(source);
                }
            }
            touched.sort_unstable();
            let window_rest: f64 = order[..passive].iter().map(|at| bounds[*at]).sum();
            for &doc in &touched {
                let mut score = std::mem::take(&mut scores[(doc - start) as usize]);
                let full = best.len() >= k;
                let theta = if full { best.peek().unwrap().0.0 } else { 0.0 };
                if full && score + window_rest <= theta {
                    continue;
                }
                let mut rest: f64 = order[..passive]
                    .iter()
                    .map(|at| lists[*at].bound_at(doc))
                    .sum();
                for &at in order[..passive].iter().rev() {
                    if full && score + rest <= theta {
                        break;
                    }
                    let bound = lists[at].bound_at(doc);
                    rest -= bound;
                    if bound > 0.0 && lists[at].seek(doc, source) == doc {
                        score += lists[at].score();
                    }
                }
                if !full {
                    best.push(Reverse(Scored(score, Reverse(doc))));
                } else if score > theta {
                    best.pop();
                    best.push(Reverse(Scored(score, Reverse(doc))));
                }
            }
            touched.clear();
        }
        start = lists
            .iter()
            .map(|list| list.next_from(end))
            .min()
            .unwrap_or(END);
    }
    sorted(
        best.into_iter()
            .map(|Reverse(Scored(score, Reverse(doc)))| (score, doc))
            .collect(),
    )
}

/// The best `k` by every posting of every list, for the check.
fn exhaustive(lists: &mut [List], source: &mut Source, k: usize) -> Vec<(f64, u32)> {
    let mut scores: HashMap<u32, f64> = HashMap::new();
    for list in lists.iter_mut() {
        while list.doc(source) != END {
            *scores.entry(list.postings[list.at].doc).or_default() += list.score();
            list.next();
        }
    }
    let mut all = sorted(
        scores
            .into_iter()
            .map(|(doc, score)| (score, doc))
            .collect(),
    );
    all.truncate(k);
    all
}

fn sorted(mut found: Vec<(f64, u32)>) -> Vec<(f64, u32)> {
    found.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    found
}

pub struct Blocks {
    connection: Connection,
    documents: u64,
    average: [f64; 4],
    gone: HashSet<u32>,
    /// Scores every posting instead of skipping, for the check.
    pub exhaustive: bool,
    /// LAB_EXECUTOR=window answers by `windowed`, anything else by `max_score`.
    windows: bool,
    /// Built with LAB_FORMAT=packed: keys by number, a directory row a chunk.
    packed: bool,
    /// The scores of the last query's documents, for the check.
    pub scores: Vec<f64>,
    /// Blocks the last query opened.
    pub opened: usize,
    /// The last query's time reading directories, and finding and unpacking blocks.
    pub directory_ms: f64,
    pub opening_ms: f64,
}

impl Blocks {
    pub fn open(path: &Path) -> Blocks {
        let connection = open(path);
        connection.pragma_update(None, "query_only", true).unwrap();
        let mut average = [0.0; 4];
        let mut documents = 0;
        for (field, tokens, holding) in connection
            .prepare("select field, tokens, documents from stats")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
        {
            average[field as usize] = (tokens as f64 / holding as f64).max(1.0);
            documents = holding as u64;
        }
        // An index built before the packed tables existed has no `keys`.
        let packed = connection
            .query_row(
                "select exists (select 1 from sqlite_schema where name = 'keys')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
            && connection
                .query_row("select exists (select 1 from keys)", [], |row| {
                    row.get::<_, bool>(0)
                })
                .unwrap();
        let gone = connection
            .prepare("select doc from gone")
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .map(|doc| doc.unwrap() as u32)
            .collect();
        Blocks {
            connection,
            documents,
            average,
            gone,
            exhaustive: false,
            windows: std::env::var("LAB_EXECUTOR").as_deref() == Ok("window"),
            packed,
            scores: Vec::new(),
            opened: 0,
            directory_ms: 0.0,
            opening_ms: 0.0,
        }
    }

    fn holding(&self, key: &str) -> u64 {
        self.find(key).map_or(0, |(_, documents)| documents)
    }

    /// A key's number, -1 when the index numbers none, and the documents it is in.
    fn find(&self, key: &str) -> Option<(i64, u64)> {
        let sql = if self.packed {
            "select id, documents from keys where key = ?1"
        } else {
            "select -1, documents from terms where key = ?1"
        };
        let mut statement = self.connection.prepare_cached(sql).unwrap();
        statement
            .query_row([key], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .optional()
            .unwrap()
    }

    /// A key's directory, from its rows a block or its packed rows a chunk.
    fn directory(&self, key: &str, id: i64, scale: f64, average: f64) -> Vec<Skip> {
        let bound = |most_tf: u32, least_len: u32| scale * saturated(most_tf, least_len, average);
        if !self.packed {
            let mut skips = self
                .connection
                .prepare_cached(
                    "select first, last, most_tf, least_len from skip where key = ?1 order by first",
                )
                .unwrap();
            return skips
                .query_map([key], |row| {
                    let (most_tf, least_len): (i64, i64) = (row.get(2)?, row.get(3)?);
                    Ok(Skip {
                        first: row.get::<_, i64>(0)? as u32,
                        last: row.get::<_, i64>(1)? as u32,
                        bound: bound(most_tf as u32, least_len as u32),
                    })
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
        }
        let mut dirs = self
            .connection
            .prepare_cached("select directory from dirs where id = ?1 order by chunk")
            .unwrap();
        let mut skips = Vec::new();
        for directory in dirs
            .query_map([id], |row| row.get::<_, Vec<u8>>(0))
            .unwrap()
        {
            let directory = directory.unwrap();
            let (mut at, mut previous) = (0, 0);
            while at < directory.len() {
                let first = previous + get_varint(&directory, &mut at);
                let last = first + get_varint(&directory, &mut at);
                let most_tf = get_varint(&directory, &mut at);
                let least_len = get_varint(&directory, &mut at);
                skips.push(Skip {
                    first,
                    last,
                    bound: bound(most_tf, least_len),
                });
                previous = last;
            }
        }
        skips
    }

    /// A word's keys and weights, as the other engines ask its forms.
    fn forms(word: &Word) -> Vec<(usize, String)> {
        let stem = word.stem.as_deref().unwrap_or(&word.surface);
        let mut forms = vec![
            (0, word.surface.clone()),
            (2, word.surface.clone()),
            (1, stem.to_owned()),
            (3, stem.to_owned()),
        ];
        if word.stem.is_some() {
            forms.push((0, stem.to_owned()));
            forms.push((2, stem.to_owned()));
        }
        for typo in &word.typos {
            forms.extend((0..4).map(|field| (field, typo.clone())));
        }
        forms
    }

    /// The lists of a query: each key's blocks, and what sets put in the head.
    fn lists(&self, words: &[Word]) -> Vec<List> {
        let mut heads = self
            .connection
            .prepare_cached("select doc, tf, len from head where key = ?1 order by doc")
            .unwrap();
        let mut lists = Vec::new();
        for word in words {
            for (field, term) in Self::forms(word) {
                let key = key(field, &term);
                let (id, holding) = self.find(&key).unwrap_or((-1, 0));
                let scale = FIELDS[field].1 * idf(self.documents, holding.max(1));
                let average = self.average[field];
                let directory = if holding > 0 {
                    self.directory(&key, id, scale, average)
                } else {
                    Vec::new()
                };
                if !directory.is_empty() {
                    let max = directory.iter().map(|skip| skip.bound).fold(0.0, f64::max);
                    lists.push(List {
                        key: key.clone(),
                        id,
                        scale,
                        average,
                        skips: directory,
                        max,
                        block: 0,
                        decoded: usize::MAX,
                        postings: Vec::new(),
                        at: 0,
                    });
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
                    let bound = head
                        .iter()
                        .map(|posting| scale * saturated(posting.tf, posting.len, average))
                        .fold(0.0, f64::max);
                    let skip = Skip {
                        first: head[0].doc,
                        last: head[head.len() - 1].doc,
                        bound,
                    };
                    lists.push(List {
                        key,
                        id: -1,
                        scale,
                        average,
                        skips: vec![skip],
                        max: bound,
                        block: 0,
                        decoded: 0,
                        postings: head,
                        at: 0,
                    });
                }
            }
        }
        lists
    }
}

impl Backend for Blocks {
    fn documents(&self) -> u64 {
        self.documents
    }

    /// The most documents one field holds the term in, as Tantivy's counts it.
    fn frequency(&mut self, term: &str) -> u64 {
        (0..4)
            .map(|field| self.holding(&key(field, term)))
            .max()
            .unwrap_or(0)
    }

    fn top(&mut self, words: &[Word], every: bool, k: usize) -> Vec<u32> {
        assert!(!every, "these blocks answer plans of any word");
        let reading = Instant::now();
        let mut lists = self.lists(words);
        self.directory_ms = reading.elapsed().as_secs_f64() * 1e3;
        let mut source = Source {
            payload: self
                .connection
                .prepare_cached(if self.packed {
                    "select payload from blob where id = ?1 and first = ?2"
                } else {
                    "select payload from post where key = ?1 and first = ?2"
                })
                .unwrap(),
            gone: &self.gone,
            opened: 0,
            opening: std::time::Duration::ZERO,
        };
        let found = if self.exhaustive {
            exhaustive(&mut lists, &mut source, k)
        } else if self.windows {
            windowed(&mut lists, &mut source, k)
        } else {
            max_score(&mut lists, &mut source, k)
        };
        self.opened = source.opened;
        self.opening_ms = source.opening.as_secs_f64() * 1e3;
        self.scores = found.iter().map(|(score, _)| *score).collect();
        found.into_iter().map(|(_, doc)| doc).collect()
    }

    fn hydrate(&mut self, ids: &[u32]) -> usize {
        let mut statement = self
            .connection
            .prepare_cached("select key, title, body from docs where id = ?1")
            .unwrap();
        ids.iter()
            .map(|id| {
                statement
                    .query_row([*id], |row| {
                        let (key, title, body): (String, String, String) =
                            (row.get(0)?, row.get(1)?, row.get(2)?);
                        Ok(key.len() + title.len() + body.len())
                    })
                    .unwrap()
            })
            .sum()
    }
}

/// Sets replace a document by writing its postings to the head and marking
/// its rowid gone from the blocks, in one transaction; packing the head into
/// blocks would be the store's maintenance, not measured here.
pub struct Writer {
    connection: Connection,
}

impl Writer {
    pub fn open(path: &Path) -> Writer {
        Writer {
            connection: open(path),
        }
    }

    pub fn set(&mut self, analyzer: &Analyzer, docs: &[(u32, &Doc)]) {
        let tx = self.connection.transaction().unwrap();
        {
            let mut gone = tx
                .prepare_cached("insert or ignore into gone (doc) values (?1)")
                .unwrap();
            let mut forget = tx
                .prepare_cached("delete from head where doc = ?1")
                .unwrap();
            let mut head = tx
                .prepare_cached("insert into head (doc, key, tf, len) values (?1, ?2, ?3, ?4)")
                .unwrap();
            let mut keep = tx
                .prepare_cached(
                    "insert or replace into docs (id, key, title, body) values (?1, ?2, ?3, ?4)",
                )
                .unwrap();
            for (id, doc) in docs {
                gone.execute([*id]).unwrap();
                forget.execute([*id]).unwrap();
                for (field, terms) in fields(analyzer, doc).iter().enumerate() {
                    for (term, tf) in counted(terms) {
                        head.execute(params![*id, key(field, term), tf, terms.len() as i64])
                            .unwrap();
                    }
                }
                keep.execute(params![*id, doc.id, doc.title, doc.text])
                    .unwrap();
            }
        }
        tx.commit().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_packs_and_unpacks_leaving_out_what_is_gone() {
        let block = [
            Posting {
                doc: 3,
                tf: 1,
                len: 9,
            },
            Posting {
                doc: 300,
                tf: 4,
                len: 120,
            },
            Posting {
                doc: 70_000,
                tf: 2,
                len: 5,
            },
        ];
        let gone = HashSet::from([300]);
        let back = unpack(&pack(&block), &gone);
        assert_eq!(
            back.iter()
                .map(|posting| (posting.doc, posting.tf, posting.len))
                .collect::<Vec<_>>(),
            [(3, 1, 9), (70_000, 2, 5)]
        );
    }
}
