//! The lab's compact blocks over MS MARCO's one field.
//!
//! The format is compact.rs's, table for table: the same schema, keys `b|term`
//! numbered in byte order, a key in no more than 16 documents inline in its
//! row, every other in blocks of 128 with a directory of first, last, largest
//! frequency and shortest field, lengths four u16 a document in rows of
//! 65,536. Two things differ from the lab and nothing else:
//!
//! - The build streams one term at a time from the prepared collection
//!   instead of holding every list in memory; `verify` checks that it writes
//!   the lab's build's tables row for row.
//! - The reader asks one key a term, `b|term`, where compact.rs asks four,
//!   since a passage has no title and its stems are its only tokens. Its
//!   lists, blocks and read-ahead are compact.rs's; the executors are the
//!   lab's own `windowed`, `max_score` and `exhaustive` from blocks.rs.
//!
//! Documents are numbered pid + 1, as the lab numbers them from one. `docs`
//! keeps each one's key, the pid, so that the lab's writer can set it, and no
//! text: no engine in this round stores the passages.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use rusqlite::{CachedStatement, Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::blocks::{
    BLOCK, FIELDS, Fetch, List, Posting, Skip, exhaustive, get_varint, idf, key, max_score, put_varint,
    saturated, windowed,
};
use crate::collection::{Collection, read_terms};
use crate::fts::open;

/// compact.rs's constants and schema, which it keeps private.
pub(crate) const INLINE: usize = 16;
pub(crate) const NORMS: u32 = 65_536;
pub(crate) const SCHEMA: &str = "
create table ckeys (key text primary key, id integer not null, documents integer not null,
  directory blob, inline blob) without rowid;
create table cblob (id integer not null, first integer not null, payload blob not null,
  primary key (id, first)) without rowid;
create table cpos (id integer not null, first integer not null, payload blob not null,
  primary key (id, first)) without rowid;
create table norms (chunk integer primary key, lengths blob not null);
create table stats (field integer primary key, tokens integer not null, documents integer not null);
create table docs (id integer primary key, key text not null unique, title text not null, body text not null);
create table head (doc integer not null, key text not null, tf integer not null, len integer not null,
  primary key (doc, key)) without rowid;
create index head_by_key on head (key, doc);
create table gone (doc integer primary key);
";

/// The one field: a body's words, which here are its stems.
pub const FIELD: usize = 2;

/// Keys a commit while building, so that the log stays a few hundred MiB.
pub(crate) const KEYS_A_COMMIT: usize = 100_000;

#[derive(Clone, Copy)]
pub(crate) struct Entry {
    pub(crate) first: u32,
    pub(crate) last: u32,
    pub(crate) most_tf: u32,
    pub(crate) least_len: u32,
}

pub(crate) fn encode_directory(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(entries.len() * 6);
    let mut previous = 0;
    for entry in entries {
        put_varint(&mut out, entry.first - previous);
        put_varint(&mut out, entry.last - entry.first);
        put_varint(&mut out, entry.most_tf);
        put_varint(&mut out, entry.least_len);
        previous = entry.last;
    }
    out
}

fn decode_directory(bytes: &[u8]) -> Vec<Entry> {
    let mut entries = Vec::new();
    let (mut at, mut previous) = (0, 0);
    while at < bytes.len() {
        let first = previous + get_varint(bytes, &mut at);
        let last = first + get_varint(bytes, &mut at);
        let most_tf = get_varint(bytes, &mut at);
        let least_len = get_varint(bytes, &mut at);
        entries.push(Entry {
            first,
            last,
            most_tf,
            least_len,
        });
        previous = last;
    }
    entries
}

pub(crate) fn pack(block: &[Posting]) -> Vec<u8> {
    let mut out = Vec::with_capacity(block.len() * 3);
    let mut previous = 0;
    for posting in block {
        put_varint(&mut out, posting.doc - previous);
        put_varint(&mut out, posting.tf);
        previous = posting.doc;
    }
    out
}

fn unpack_raw(payload: &[u8]) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(BLOCK);
    let (mut at, mut doc) = (0, 0);
    while at < payload.len() {
        doc += get_varint(payload, &mut at);
        out.push((doc, get_varint(payload, &mut at)));
    }
    out
}

pub(crate) fn pack_inline(postings: &[Posting]) -> Vec<u8> {
    let mut out = Vec::with_capacity(postings.len() * 4);
    let mut previous = 0;
    for posting in postings {
        put_varint(&mut out, posting.doc - previous);
        put_varint(&mut out, posting.tf);
        put_varint(&mut out, posting.len);
        previous = posting.doc;
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

/// Bytes by b-tree, the largest first.
pub fn objects(connection: &Connection) -> Value {
    let rows: Vec<(String, i64, i64, i64)> = connection
        .prepare(
            "select name, sum(pgsize), sum(payload), count(*) from dbstat group by name order by 2 desc",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    Value::Array(
        rows.into_iter()
            .map(|(name, bytes, payload, pages)| json!({"name": name, "bytes": bytes, "payload": payload, "pages": pages}))
            .collect(),
    )
}

/// Writes the compact tables from the prepared collection, a term at a time.
pub fn build(dir: &Path, path: &Path) -> Value {
    let started = Instant::now();
    let collection = Collection::open(dir);
    let names = read_terms(dir);
    let mut connection = open(path);
    connection.execute_batch(SCHEMA).unwrap();
    let documents = collection.documents as usize;
    let tx = connection.transaction().unwrap();
    {
        let mut keep = tx
            .prepare("insert into docs (id, key, title, body) values (?1, ?2, '', '')")
            .unwrap();
        for pid in 0..documents {
            keep.execute(params![pid as i64 + 1, pid.to_string()]).unwrap();
        }
    }
    tx.commit().unwrap();
    let docs_seconds = started.elapsed().as_secs_f64();
    let (mut postings, mut blocks, mut inline) = (0u64, 0u64, 0u64);
    let mut tx = connection.transaction().unwrap();
    for (number, term) in names.iter().enumerate() {
        if number > 0 && number % KEYS_A_COMMIT == 0 {
            tx.commit().unwrap();
            tx = connection.transaction().unwrap();
        }
        let (docs, freqs) = collection.list(number);
        let list: Vec<Posting> = docs
            .iter()
            .zip(freqs)
            .map(|(pid, tf)| Posting {
                doc: pid + 1,
                tf: *tf,
                len: collection.sizes[*pid as usize],
            })
            .collect();
        let id = number as i64 + 1;
        postings += list.len() as u64;
        let mut row = tx
            .prepare_cached("insert into ckeys (key, id, documents, directory, inline) values (?1, ?2, ?3, ?4, ?5)")
            .unwrap();
        if list.len() <= INLINE {
            inline += 1;
            row.execute(params![key(FIELD, term), id, list.len() as i64, None::<Vec<u8>>, pack_inline(&list)])
                .unwrap();
            continue;
        }
        let mut entries = Vec::new();
        let mut blob = tx
            .prepare_cached("insert into cblob (id, first, payload) values (?1, ?2, ?3)")
            .unwrap();
        for block in list.chunks(BLOCK) {
            blocks += 1;
            entries.push(Entry {
                first: block[0].doc,
                last: block[block.len() - 1].doc,
                most_tf: block.iter().map(|posting| posting.tf).max().unwrap(),
                least_len: block.iter().map(|posting| posting.len).min().unwrap(),
            });
            blob.execute(params![id, block[0].doc, pack(block)]).unwrap();
        }
        row.execute(params![key(FIELD, term), id, list.len() as i64, encode_directory(&entries), None::<Vec<u8>>])
            .unwrap();
    }
    {
        let mut norms = vec![0u16; (documents + 1) * 4];
        for (pid, size) in collection.sizes.iter().enumerate() {
            norms[(pid + 1) * 4 + FIELD] = (*size).min(u32::from(u16::MAX)) as u16;
        }
        let mut lengths = tx
            .prepare("insert into norms (chunk, lengths) values (?1, ?2)")
            .unwrap();
        for (chunk, slice) in norms.chunks(NORMS as usize * 4).enumerate() {
            let bytes: Vec<u8> = slice.iter().flat_map(|length| length.to_le_bytes()).collect();
            lengths.execute(params![chunk as i64, bytes]).unwrap();
        }
        for field in 0..4 {
            let tokens = if field == FIELD { collection.tokens } else { 0 };
            tx.execute(
                "insert into stats (field, tokens, documents) values (?1, ?2, ?3)",
                params![field as i64, tokens as i64, documents as i64],
            )
            .unwrap();
        }
    }
    tx.commit().unwrap();
    connection.pragma_update(None, "wal_checkpoint", "truncate").unwrap();
    let seconds = started.elapsed().as_secs_f64();
    let objects = objects(&connection);
    drop(connection);
    json!({"step": "build", "engine": "compact", "seconds": seconds, "docs_seconds": docs_seconds,
        "file": std::fs::metadata(path).unwrap().len(), "postings": postings, "blocks": blocks,
        "keys": names.len(), "inline": inline, "objects": objects, "memory": crate::memory()})
}

/// A row of norms as lengths, as long as a full row.
fn lengths_of(bytes: &[u8]) -> Vec<u16> {
    let mut lengths: Vec<u16> = bytes
        .chunks(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    lengths.resize(NORMS as usize * 4, 0);
    lengths
}

#[derive(Default)]
struct Norms {
    chunks: HashMap<u32, Rc<Vec<u16>>>,
}

impl Norms {
    fn chunk(&mut self, connection: &Connection, chunk: u32) -> Rc<Vec<u16>> {
        Rc::clone(self.chunks.entry(chunk).or_insert_with(|| {
            let bytes: Vec<u8> = connection
                .prepare_cached("select lengths from norms where chunk = ?1")
                .unwrap()
                .query_row([chunk], |row| row.get(0))
                .optional()
                .unwrap()
                .unwrap_or_default();
            Rc::new(lengths_of(&bytes))
        }))
    }
}

/// compact.rs's source of blocks: a statement a block, or a run of them once
/// a list is read in order, the lengths from the reader's norms.
struct Source<'a> {
    connection: &'a Connection,
    point: CachedStatement<'a>,
    range: CachedStatement<'a>,
    gone: &'a HashSet<u32>,
    norms: &'a mut Norms,
    ahead: usize,
    read: HashMap<i64, (usize, Vec<(usize, Vec<Posting>)>)>,
    opened: usize,
    statements: usize,
}

impl Source<'_> {
    fn postings(&mut self, payload: &[u8], field: usize) -> Vec<Posting> {
        let mut out = Vec::with_capacity(BLOCK);
        let mut current: Option<(u32, Rc<Vec<u16>>)> = None;
        for (doc, tf) in unpack_raw(payload) {
            if self.gone.contains(&doc) {
                continue;
            }
            let chunk = doc / NORMS;
            if current.as_ref().is_none_or(|(at, _)| *at != chunk) {
                current = Some((chunk, self.norms.chunk(self.connection, chunk)));
            }
            let lengths = &current.as_ref().unwrap().1;
            let len = u32::from(lengths[((doc % NORMS) as usize) * 4 + field]);
            out.push(Posting { doc, tf, len });
        }
        out
    }
}

impl Fetch for Source<'_> {
    fn block(&mut self, list: &List, index: usize) -> Vec<Posting> {
        let read = self.read.entry(list.id).or_insert((usize::MAX, Vec::new()));
        if let Some(at) = read.1.iter().position(|(block, _)| *block == index) {
            let (_, postings) = read.1.swap_remove(at);
            read.0 = index;
            return postings;
        }
        let in_order = read.0 != usize::MAX && index == read.0 + 1;
        read.0 = index;
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
        self.statements += 1;
        self.opened += payloads.len();
        let mut wanted = Vec::new();
        for (offset, (_, payload)) in payloads.iter().enumerate() {
            let postings = self.postings(payload, list.field);
            if offset == 0 {
                wanted = postings;
            } else {
                self.read.get_mut(&list.id).unwrap().1.push((index + offset, postings));
            }
        }
        wanted
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Executor {
    Window,
    Cursor,
    Exhaustive,
}

/// A reader of the one field, as compact.rs's `Compact` reads four.
pub struct Reader {
    connection: Connection,
    documents: u64,
    average: f64,
    gone: HashSet<u32>,
    norms: Norms,
    version: i64,
    settled_chunks: u32,
    pub executor: Executor,
    pub ahead: usize,
    pub opened: usize,
    pub statements: usize,
}

impl Reader {
    pub fn open(path: &Path, executor: Executor, ahead: usize) -> Reader {
        let connection = open(path);
        connection.pragma_update(None, "query_only", true).unwrap();
        let settled: i64 = connection
            .query_row("select coalesce(max(id), 0) from docs", [], |row| row.get(0))
            .unwrap();
        let mut reader = Reader {
            connection,
            documents: 0,
            average: 1.0,
            gone: HashSet::new(),
            norms: Norms::default(),
            version: -1,
            settled_chunks: settled as u32 / NORMS,
            executor,
            ahead,
            opened: 0,
            statements: 0,
        };
        reader.refresh();
        reader
    }

    /// Reads the statistics and the gone again when another connection wrote.
    fn refresh(&mut self) {
        let version: i64 = self
            .connection
            .query_row("pragma data_version", [], |row| row.get(0))
            .unwrap();
        if version == self.version {
            return;
        }
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
        self.norms.chunks.retain(|chunk, _| *chunk < settled);
    }

    fn lists(&self, terms: &[String]) -> Vec<List> {
        let mut find = self
            .connection
            .prepare_cached("select id, documents, directory, inline from ckeys where key = ?1")
            .unwrap();
        let mut heads = self
            .connection
            .prepare_cached("select doc, tf, len from head where key = ?1 order by doc")
            .unwrap();
        let mut lists = Vec::new();
        for term in terms {
            let key = key(FIELD, term);
            let found: Option<(i64, u64, Option<Vec<u8>>, Option<Vec<u8>>)> = find
                .query_row([&key], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u64, row.get(2)?, row.get(3)?)))
                .optional()
                .unwrap();
            let holding = found.as_ref().map_or(0, |found| found.1);
            let scale = FIELDS[FIELD].1 * idf(self.documents, holding.max(1));
            let average = self.average;
            match found {
                Some((_, _, _, Some(inline))) => {
                    let postings = unpack_inline(&inline, &self.gone);
                    if !postings.is_empty() {
                        lists.push(List::whole(key.clone(), FIELD, scale, average, postings));
                    }
                }
                Some((id, _, Some(directory), None)) => {
                    let skips: Vec<Skip> = decode_directory(&directory)
                        .into_iter()
                        .map(|entry| Skip {
                            first: entry.first,
                            last: entry.last,
                            bound: scale * saturated(entry.most_tf, entry.least_len, average),
                        })
                        .collect();
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
        lists
    }

    /// The best `k` documents for any of `terms`, with their scores.
    pub fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        self.refresh();
        let mut lists = self.lists(terms);
        let mut norms = std::mem::take(&mut self.norms);
        let (found, opened, statements) = {
            let mut source = Source {
                connection: &self.connection,
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
                norms: &mut norms,
                ahead: self.ahead,
                read: HashMap::new(),
                opened: 0,
                statements: 0,
            };
            let found = match self.executor {
                Executor::Window => windowed(&mut lists, &mut source, k),
                Executor::Cursor => max_score(&mut lists, &mut source, k),
                Executor::Exhaustive => exhaustive(&mut lists, &mut source, k),
            };
            (found, source.opened, source.statements)
        };
        self.norms = norms;
        self.opened = opened;
        self.statements = statements;
        found
    }
}

/// Builds the first `n` passages both ways, the lab's compact::build over
/// their tokens and `build` over the same prepared subset, and compares every
/// table but the documents' row for row.
pub fn verify(subset: &Path, n: usize) -> Value {
    use crate::analyze::{Analysis, Analyzer};
    use crate::corpus::Doc;
    let docs: Vec<Doc> = std::fs::read_to_string(subset.join("tokens.tsv"))
        .unwrap()
        .lines()
        .take(n)
        .map(|line| {
            let (pid, tokens) = line.split_once('\t').unwrap();
            Doc {
                id: pid.to_owned(),
                title: String::new(),
                text: tokens.to_owned(),
            }
        })
        .collect();
    let lab_path = subset.join("lab-compact.db");
    let ours_path = subset.join("ours-compact.db");
    for path in [&lab_path, &ours_path] {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
    let lab = crate::compact::build(&lab_path, &Analyzer::new(Analysis::named("surface")), &docs);
    let ours = build(subset, &ours_path);
    let a = open(&lab_path);
    a.execute(&format!("attach database '{}' as ours", ours_path.display()), [])
        .unwrap();
    let mut differ = serde_json::Map::new();
    for (table, columns) in [
        ("ckeys", "key, id, documents, directory, inline"),
        ("cblob", "id, first, payload"),
        ("norms", "chunk, lengths"),
        ("stats", "field, tokens, documents"),
    ] {
        let count = |sql: String| a.query_row(&sql, [], |row| row.get::<_, i64>(0)).unwrap();
        let only_lab = count(format!(
            "select count(*) from (select {columns} from main.{table} except select {columns} from ours.{table})"
        ));
        let only_ours = count(format!(
            "select count(*) from (select {columns} from ours.{table} except select {columns} from main.{table})"
        ));
        let rows = count(format!("select count(*) from main.{table}"));
        differ.insert(table.to_owned(), json!({"rows": rows, "only_lab": only_lab, "only_ours": only_ours}));
    }
    json!({"step": "verify-compact", "documents": n, "lab_postings": lab.postings, "lab_blocks": lab.blocks,
        "lab_keys": lab.keys, "tables": differ, "ours": ours})
}
