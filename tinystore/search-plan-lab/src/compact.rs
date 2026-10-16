//! The blocks of `blocks` with less to store and less to read, and a head
//! that is packed into them.
//!
//! - A field's length lives once a document, in `norms`, rather than in each
//!   of its postings: a block holds documents and frequencies alone, and a
//!   reader keeps the lengths it met, 8 bytes a document.
//! - A key in no more than 16 documents keeps its postings, lengths and all,
//!   in its own row: no block and no directory row for most keys.
//! - The documents are built in one pass, so no key has a short block but its
//!   last.
//! - A set gives a document a new number and marks the old one gone, as
//!   Lucene does, so the head's documents always follow every block's, and
//!   packing the head appends to each key's list.
//! - LAB_POSITIONS=1 keeps each posting's positions in `cpos` beside its
//!   block, to weigh them; no query reads them.
//! - LAB_AHEAD=n reads the next n blocks of a list in one statement once the
//!   list is being read in order.
//!
//! ```text
//! ckeys (key)       → id, documents, directory | inline   (doc delta, tf, len) inline
//! cblob (id, first) → payload                            (doc delta, tf) a posting
//! cpos  (id, first) → positions                          count, then deltas, a posting
//! norms (chunk)     → lengths                            u16 a field, 4 a document, 65,536 documents a row
//! ```

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use rusqlite::{CachedStatement, Connection, OptionalExtension, params};

use crate::analyze::Analyzer;
use crate::blocks::{
    BLOCK, FIELDS, Fetch, List, Posting, Skip, exhaustive, fields, get_varint, idf, key, max_score,
    put_varint, saturated, windowed,
};
use crate::corpus::Doc;
use crate::fts::open;
use crate::plan::{Backend, Word};

const INLINE: usize = 16;
const NORMS: u32 = 65_536;

const SCHEMA: &str = "
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

/// A directory's entry: a block's range and what bounds it.
#[derive(Clone, Copy)]
struct Entry {
    first: u32,
    last: u32,
    most_tf: u32,
    least_len: u32,
}

fn encode_directory(entries: &[Entry]) -> Vec<u8> {
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

fn entry_of(block: &[Posting]) -> Entry {
    Entry {
        first: block[0].doc,
        last: block[block.len() - 1].doc,
        most_tf: block.iter().map(|posting| posting.tf).max().unwrap(),
        least_len: block.iter().map(|posting| posting.len).min().unwrap(),
    }
}

/// A block's payload: documents and frequencies, no lengths.
fn pack(block: &[Posting]) -> Vec<u8> {
    let mut out = Vec::with_capacity(block.len() * 3);
    let mut previous = 0;
    for posting in block {
        put_varint(&mut out, posting.doc - previous);
        put_varint(&mut out, posting.tf);
        previous = posting.doc;
    }
    out
}

/// A block's documents and frequencies as they were written, nothing left out.
fn unpack_raw(payload: &[u8]) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(BLOCK);
    let (mut at, mut doc) = (0, 0);
    while at < payload.len() {
        doc += get_varint(payload, &mut at);
        out.push((doc, get_varint(payload, &mut at)));
    }
    out
}

/// A short list as its key's row keeps it: lengths and all.
fn pack_inline(postings: &[Posting]) -> Vec<u8> {
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

pub struct Built {
    pub seconds: f64,
    pub objects: Vec<(String, u64)>,
    pub file: u64,
    pub postings: u64,
    pub blocks: u64,
    pub keys: u64,
    pub inline: u64,
}

/// A key's postings while the build gathers them, and their positions when kept.
#[derive(Default)]
struct Gathered {
    postings: Vec<Posting>,
    positions: Vec<u8>,
}

pub fn build(path: &Path, analyzer: &Analyzer, docs: &[Doc]) -> Built {
    let started = Instant::now();
    let keep_positions = std::env::var("LAB_POSITIONS").as_deref() == Ok("1");
    let mut connection = open(path);
    connection.execute_batch(SCHEMA).unwrap();
    let mut lists: HashMap<String, Gathered> = HashMap::new();
    let mut norms = vec![0u16; (docs.len() + 1) * 4];
    let mut tokens = [0u64; 4];
    let tx = connection.transaction().unwrap();
    {
        let mut keep = tx
            .prepare("insert into docs (id, key, title, body) values (?1, ?2, ?3, ?4)")
            .unwrap();
        for (at, doc) in docs.iter().enumerate() {
            let id = at as u32 + 1;
            for (field, terms) in fields(analyzer, doc).iter().enumerate() {
                let len = terms.len() as u32;
                tokens[field] += u64::from(len);
                norms[id as usize * 4 + field] = len.min(u32::from(u16::MAX)) as u16;
                let mut at_positions: HashMap<&str, Vec<u32>> = HashMap::new();
                for (position, term) in terms.iter().enumerate() {
                    at_positions
                        .entry(term.as_str())
                        .or_default()
                        .push(position as u32);
                }
                for (term, positions) in at_positions {
                    let gathered = lists.entry(key(field, term)).or_default();
                    gathered.postings.push(Posting {
                        doc: id,
                        tf: positions.len() as u32,
                        len,
                    });
                    if keep_positions {
                        put_varint(&mut gathered.positions, positions.len() as u32);
                        let mut previous = 0;
                        for position in positions {
                            put_varint(&mut gathered.positions, position - previous);
                            previous = position;
                        }
                    }
                }
            }
            keep.execute(params![id, doc.id, doc.title, doc.text])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    let (mut postings, mut blocks, mut inline) = (0u64, 0u64, 0u64);
    let mut keys: Vec<&String> = lists.keys().collect();
    keys.sort();
    let tx = connection.transaction().unwrap();
    {
        let mut row = tx
            .prepare("insert into ckeys (key, id, documents, directory, inline) values (?1, ?2, ?3, ?4, ?5)")
            .unwrap();
        let mut blob = tx
            .prepare("insert into cblob (id, first, payload) values (?1, ?2, ?3)")
            .unwrap();
        let mut pos = tx
            .prepare("insert into cpos (id, first, payload) values (?1, ?2, ?3)")
            .unwrap();
        for (number, key) in keys.iter().enumerate() {
            let id = number as i64 + 1;
            let gathered = &lists[*key];
            let list = &gathered.postings;
            postings += list.len() as u64;
            if list.len() <= INLINE {
                inline += 1;
                row.execute(params![
                    key,
                    id,
                    list.len() as i64,
                    None::<Vec<u8>>,
                    pack_inline(list)
                ])
                .unwrap();
                if keep_positions {
                    pos.execute(params![id, list[0].doc, gathered.positions])
                        .unwrap();
                }
                continue;
            }
            let mut entries = Vec::new();
            let mut at = 0;
            for block in list.chunks(BLOCK) {
                blocks += 1;
                entries.push(entry_of(block));
                blob.execute(params![id, block[0].doc, pack(block)])
                    .unwrap();
                if keep_positions {
                    let from = at;
                    for _ in block {
                        let count = get_varint(&gathered.positions, &mut at);
                        for _ in 0..count {
                            get_varint(&gathered.positions, &mut at);
                        }
                    }
                    pos.execute(params![id, block[0].doc, &gathered.positions[from..at]])
                        .unwrap();
                }
            }
            row.execute(params![
                key,
                id,
                list.len() as i64,
                encode_directory(&entries),
                None::<Vec<u8>>
            ])
            .unwrap();
        }
        let mut lengths = tx
            .prepare("insert into norms (chunk, lengths) values (?1, ?2)")
            .unwrap();
        for (chunk, slice) in norms.chunks(NORMS as usize * 4).enumerate() {
            let bytes: Vec<u8> = slice
                .iter()
                .flat_map(|length| length.to_le_bytes())
                .collect();
            lengths.execute(params![chunk as i64, bytes]).unwrap();
        }
        for (field, tokens) in tokens.iter().enumerate() {
            tx.execute(
                "insert into stats (field, tokens, documents) values (?1, ?2, ?3)",
                params![field as i64, *tokens as i64, docs.len() as i64],
            )
            .unwrap();
        }
    }
    tx.commit().unwrap();
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
        keys: keys.len() as u64,
        inline,
    }
}

/// A row of norms as lengths, as long as a full row: the build's last row
/// holds only the documents it had, and later documents fall in it too.
fn lengths_of(bytes: &[u8]) -> Vec<u16> {
    let mut lengths: Vec<u16> = bytes
        .chunks(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    lengths.resize(NORMS as usize * 4, 0);
    lengths
}

/// The field lengths a reader has met: a row of `norms` each, kept.
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

    fn bytes(&self) -> usize {
        self.chunks.values().map(|chunk| chunk.len() * 2).sum()
    }
}

/// Where a compact list's blocks come from: SQLite, a statement a block or
/// a run of them, the lengths from the reader's norms.
struct Source<'a> {
    connection: &'a Connection,
    point: CachedStatement<'a>,
    range: CachedStatement<'a>,
    gone: &'a HashSet<u32>,
    norms: &'a mut Norms,
    ahead: usize,
    /// Each list's last block read, and the blocks read ahead of it.
    read: HashMap<i64, (usize, Vec<(usize, Vec<Posting>)>)>,
    opened: usize,
    statements: usize,
    opening: std::time::Duration,
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
        let started = Instant::now();
        let read = self.read.entry(list.id).or_insert((usize::MAX, Vec::new()));
        if let Some(at) = read.1.iter().position(|(block, _)| *block == index) {
            let (_, postings) = read.1.swap_remove(at);
            read.0 = index;
            self.opening += started.elapsed();
            return postings;
        }
        let in_order = read.0 != usize::MAX && index == read.0 + 1;
        read.0 = index;
        let payloads: Vec<(u32, Vec<u8>)> = if self.ahead > 1 && in_order {
            let last = (index + self.ahead - 1).min(list.skips.len() - 1);
            self.range
                .query_map(
                    params![list.id, list.skips[index].first, list.skips[last].first],
                    |row| Ok((row.get::<_, i64>(0)? as u32, row.get::<_, Vec<u8>>(1)?)),
                )
                .unwrap()
                .map(Result::unwrap)
                .collect()
        } else {
            let first = list.skips[index].first;
            vec![(
                first,
                self.point
                    .query_row(params![list.id, first], |row| row.get(0))
                    .unwrap(),
            )]
        };
        self.statements += 1;
        self.opened += payloads.len();
        let mut wanted = Vec::new();
        for (offset, (_, payload)) in payloads.iter().enumerate() {
            let postings = self.postings(payload, list.field);
            if offset == 0 {
                wanted = postings;
            } else {
                self.read
                    .get_mut(&list.id)
                    .unwrap()
                    .1
                    .push((index + offset, postings));
            }
        }
        self.opening += started.elapsed();
        wanted
    }
}

pub struct Compact {
    connection: Connection,
    documents: u64,
    average: [f64; 4],
    gone: HashSet<u32>,
    norms: Norms,
    /// SQLite's data_version when the statistics and the gone were read.
    version: i64,
    /// The gone are told by the writer beside this reader, as a store's
    /// readers share its writer's memory, rather than read again.
    pub shared_gone: bool,
    /// Rows of norms that sets may still change: those past the build's documents.
    settled_chunks: u32,
    pub exhaustive: bool,
    windows: bool,
    ahead: usize,
    pub scores: Vec<f64>,
    pub opened: usize,
    pub statements: usize,
    pub directory_ms: f64,
    pub opening_ms: f64,
}

impl Compact {
    pub fn open(path: &Path) -> Compact {
        let connection = open(path);
        connection.pragma_update(None, "query_only", true).unwrap();
        let settled: i64 = connection
            .query_row("select coalesce(max(id), 0) from docs", [], |row| {
                row.get(0)
            })
            .unwrap();
        let mut compact = Compact {
            connection,
            documents: 0,
            average: [1.0; 4],
            gone: HashSet::new(),
            norms: Norms::default(),
            version: -1,
            shared_gone: false,
            settled_chunks: settled as u32 / NORMS,
            exhaustive: false,
            windows: std::env::var("LAB_EXECUTOR").as_deref() == Ok("window"),
            ahead: std::env::var("LAB_AHEAD")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(1),
            scores: Vec::new(),
            opened: 0,
            statements: 0,
            directory_ms: 0.0,
            opening_ms: 0.0,
        };
        compact.refresh();
        compact
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
        for (field, tokens, holding) in self
            .connection
            .prepare_cached("select field, tokens, documents from stats")
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
            self.average[field as usize] = (tokens as f64 / holding.max(1) as f64).max(1.0);
            self.documents = holding as u64;
        }
        if !self.shared_gone {
            self.gone = self
                .connection
                .prepare_cached("select doc from gone")
                .unwrap()
                .query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .map(|doc| doc.unwrap() as u32)
                .collect();
        }
        let settled = self.settled_chunks;
        self.norms.chunks.retain(|chunk, _| *chunk < settled);
    }

    /// Documents the writer beside this reader marked gone.
    pub fn forget(&mut self, gone: &[u32]) {
        self.gone.extend(gone);
    }

    pub fn gone_count(&self) -> usize {
        self.gone.len()
    }

    /// Forgets documents a purge took out of every block.
    pub fn purged(&mut self, docs: &[u32]) {
        for doc in docs {
            self.gone.remove(doc);
        }
    }

    /// The bytes of field lengths the reader holds.
    pub fn norms_bytes(&self) -> usize {
        self.norms.bytes()
    }

    fn find(&self, key: &str) -> Option<(i64, u64, Option<Vec<u8>>, Option<Vec<u8>>)> {
        self.connection
            .prepare_cached("select id, documents, directory, inline from ckeys where key = ?1")
            .unwrap()
            .query_row([key], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get(2)?,
                    row.get(3)?,
                ))
            })
            .optional()
            .unwrap()
    }

    fn lists(&self, words: &[Word]) -> Vec<List> {
        let mut heads = self
            .connection
            .prepare_cached("select doc, tf, len from head where key = ?1 order by doc")
            .unwrap();
        let mut lists = Vec::new();
        for word in words {
            for (field, term) in forms(word) {
                let key = key(field, &term);
                let found = self.find(&key);
                let holding = found.as_ref().map_or(0, |found| found.1);
                let scale = FIELDS[field].1 * idf(self.documents, holding.max(1));
                let average = self.average[field];
                match found {
                    Some((_, _, _, Some(inline))) => {
                        let postings = unpack_inline(&inline, &self.gone);
                        if !postings.is_empty() {
                            lists.push(List::whole(key.clone(), field, scale, average, postings));
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
                            field,
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
                    lists.push(List::whole(key, field, scale, average, head));
                }
            }
        }
        lists
    }
}

/// A word's keys, as the other engines ask its forms.
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

impl Backend for Compact {
    fn documents(&self) -> u64 {
        self.documents
    }

    fn frequency(&mut self, term: &str) -> u64 {
        (0..4)
            .map(|field| self.find(&key(field, term)).map_or(0, |found| found.1))
            .max()
            .unwrap_or(0)
    }

    fn top(&mut self, words: &[Word], every: bool, k: usize) -> Vec<u32> {
        assert!(!every, "these blocks answer plans of any word");
        self.refresh();
        let reading = Instant::now();
        let mut lists = self.lists(words);
        self.directory_ms = reading.elapsed().as_secs_f64() * 1e3;
        let mut norms = std::mem::take(&mut self.norms);
        let (found, opened, statements, opening) = {
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
                opening: std::time::Duration::ZERO,
            };
            let found = if self.exhaustive {
                exhaustive(&mut lists, &mut source, k)
            } else if self.windows {
                windowed(&mut lists, &mut source, k)
            } else {
                max_score(&mut lists, &mut source, k)
            };
            (found, source.opened, source.statements, source.opening)
        };
        self.norms = norms;
        self.opened = opened;
        self.statements = statements;
        self.opening_ms = opening.as_secs_f64() * 1e3;
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

/// What packing the head did.
pub struct Packed {
    pub documents: usize,
    pub keys: usize,
    pub postings: usize,
    pub millis: f64,
}

/// Sets give a document a new number and write its postings to the head;
/// `pack` appends the head to the keys' lists, all of it in one commit, and
/// `pack_step` a few keys of it a commit. `purge_begin` and `purge_step`
/// take the documents gone so far out of the blocks that hold them.
pub struct Writer {
    connection: Connection,
    next_doc: u32,
    next_key: i64,
    purge: Option<Purge>,
}

/// A purge under way: the documents it takes out, and each key that holds
/// any of them with the ones it holds, a step of keys at a time.
struct Purge {
    docs: Vec<u32>,
    keys: Vec<(String, Vec<u32>)>,
    at: usize,
}

/// What a step of purging did; `purged` the documents a finished purge took
/// out of every block, which readers may forget.
#[derive(Default)]
pub struct Purged {
    pub keys: usize,
    pub postings: usize,
    pub millis: f64,
    pub purged: Vec<u32>,
}

impl Writer {
    pub fn open(path: &Path) -> Writer {
        let connection = open(path);
        let next_doc: i64 = connection
            .query_row(
                "select max((select coalesce(max(id), 0) from docs), (select coalesce(max(doc), 0) from gone)) + 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let next_key: i64 = connection
            .query_row("select coalesce(max(id), 0) + 1 from ckeys", [], |row| {
                row.get(0)
            })
            .unwrap();
        // The head read a key at a time, and a gone document's text kept
        // until a purge has read which keys hold it.
        connection
            .execute_batch(
                "create index if not exists head_key on head (key);
                 create table if not exists gone_text (doc integer primary key, title text not null, body text not null);",
            )
            .unwrap();
        Writer {
            connection,
            next_doc: next_doc as u32,
            next_key,
            purge: None,
        }
    }

    /// Appends at most `most_keys` of the head's keys to their lists, in one
    /// commit, so that a writer is held for a step and never for the head.
    /// A document's lengths go to its norms the first time a step packs any
    /// of its keys.
    pub fn pack_step(&mut self, most_rows: usize) -> Packed {
        let started = Instant::now();
        let tx = self.connection.transaction().unwrap();
        // The head's first rows in key order, one statement, and the last key
        // left whole for the next step unless it is the only one.
        let mut rows: Vec<(String, Posting)> = tx
            .prepare_cached("select key, doc, tf, len from head order by key, doc limit ?1")
            .unwrap()
            .query_map([most_rows as i64 + 1], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Posting {
                        doc: row.get::<_, i64>(1)? as u32,
                        tf: row.get::<_, i64>(2)? as u32,
                        len: row.get::<_, i64>(3)? as u32,
                    },
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        if rows.len() > most_rows {
            let last = rows[rows.len() - 1].0.clone();
            if rows[0].0 == last {
                rows = tx
                    .prepare_cached(
                        "select key, doc, tf, len from head where key = ?1 order by doc",
                    )
                    .unwrap()
                    .query_map([&last], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            Posting {
                                doc: row.get::<_, i64>(1)? as u32,
                                tf: row.get::<_, i64>(2)? as u32,
                                len: row.get::<_, i64>(3)? as u32,
                            },
                        ))
                    })
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
            } else {
                rows.retain(|(key, _)| *key != last);
            }
        }
        let mut lengths: HashMap<u32, [u16; 4]> = HashMap::new();
        let postings = rows.len();
        let mut keys = Vec::new();
        let mut at = 0;
        while at < rows.len() {
            let key = rows[at].0.clone();
            let field = field_of(&key);
            let mut fresh = Vec::new();
            while at < rows.len() && rows[at].0 == key {
                let posting = rows[at].1;
                lengths.entry(posting.doc).or_default()[field] =
                    posting.len.min(u32::from(u16::MAX)) as u16;
                fresh.push(posting);
                at += 1;
            }
            append(&tx, &mut self.next_key, &key, &fresh);
            keys.push(key);
        }
        if let (Some(first), Some(last)) = (keys.first(), keys.last()) {
            tx.prepare_cached("delete from head where key >= ?1 and key <= ?2")
                .unwrap()
                .execute([first, last])
                .unwrap();
        }
        let mut chunks: HashMap<u32, Vec<u16>> = HashMap::new();
        // Only the rows of norms a step gave a document its lengths in are
        // written again: a row holds 65,536 documents, 512 KiB.
        let mut changed: HashSet<u32> = HashSet::new();
        let mut added = [0u64; 4];
        let mut documents = 0;
        for (doc, fields) in &mut lengths {
            let chunk = chunks
                .entry(*doc / NORMS)
                .or_insert_with(|| norms_of(&tx, *doc / NORMS));
            let base = ((*doc % NORMS) as usize) * 4;
            if chunk[base..base + 4].iter().any(|length| *length > 0) {
                continue;
            }
            changed.insert(*doc / NORMS);
            // Its other fields' lengths, from the keys of its still in the head.
            let mut rest = tx
                .prepare_cached("select key, len from head where doc = ?1")
                .unwrap();
            for (key, len) in rest
                .query_map([*doc], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap()
                .map(Result::unwrap)
            {
                fields[field_of(&key)] = len.min(i64::from(u16::MAX)) as u16;
            }
            chunk[base..base + 4].copy_from_slice(fields);
            for (sum, length) in added.iter_mut().zip(fields.iter()) {
                *sum += u64::from(*length);
            }
            documents += 1;
        }
        chunks.retain(|chunk, _| changed.contains(chunk));
        write_norms(&tx, chunks);
        for (field, tokens) in added.iter().enumerate() {
            tx.execute(
                "update stats set tokens = tokens + ?1, documents = (select count(*) from docs) where field = ?2",
                params![*tokens as i64, field as i64],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        Packed {
            documents,
            keys: keys.len(),
            postings,
            millis: started.elapsed().as_secs_f64() * 1e3,
        }
    }

    /// Reads which keys hold the documents gone so far, from their texts;
    /// says how many keys a purge will step through.
    pub fn purge_begin(&mut self, analyzer: &Analyzer) -> usize {
        let rows: Vec<(u32, String, String)> = self
            .connection
            .prepare("select doc, title, body from gone_text order by doc")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)? as u32, row.get(1)?, row.get(2)?))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut keys: std::collections::BTreeMap<String, Vec<u32>> =
            std::collections::BTreeMap::new();
        for (doc, title, body) in &rows {
            let text = Doc {
                id: String::new(),
                title: title.clone(),
                text: body.clone(),
            };
            for (field, terms) in fields(analyzer, &text).iter().enumerate() {
                for (term, _) in crate::blocks::counted(terms) {
                    keys.entry(key(field, term)).or_default().push(*doc);
                }
            }
        }
        let count = keys.len();
        self.purge = Some(Purge {
            docs: rows.into_iter().map(|(doc, _, _)| doc).collect(),
            keys: keys.into_iter().collect(),
            at: 0,
        });
        count
    }

    pub fn purging(&self) -> bool {
        self.purge.is_some()
    }

    /// Takes the purge's documents out of at most `most_keys` keys' blocks,
    /// in one commit: only the blocks whose range holds one are written
    /// again. The last step forgets them, their texts and their lengths.
    pub fn purge_step(&mut self, most_keys: usize) -> Purged {
        let started = Instant::now();
        let Writer {
            connection, purge, ..
        } = self;
        let Some(under_way) = purge.as_mut() else {
            return Purged::default();
        };
        let tx = connection.transaction().unwrap();
        let end = (under_way.at + most_keys).min(under_way.keys.len());
        let mut norms: HashMap<u32, Vec<u16>> = HashMap::new();
        let mut postings = 0;
        for (key, docs) in &under_way.keys[under_way.at..end] {
            postings += take_out(&tx, key, docs, &mut norms);
        }
        let keys = end - under_way.at;
        under_way.at = end;
        let mut purged = Vec::new();
        if end == under_way.keys.len() {
            let mut chunks: HashMap<u32, Vec<u16>> = HashMap::new();
            let mut removed = [0u64; 4];
            for doc in &under_way.docs {
                let chunk = chunks
                    .entry(*doc / NORMS)
                    .or_insert_with(|| norms_of(&tx, *doc / NORMS));
                let base = ((*doc % NORMS) as usize) * 4;
                for (sum, length) in removed.iter_mut().zip(&mut chunk[base..base + 4]) {
                    *sum += u64::from(*length);
                    *length = 0;
                }
                tx.prepare_cached("delete from gone where doc = ?1")
                    .unwrap()
                    .execute([*doc])
                    .unwrap();
                tx.prepare_cached("delete from gone_text where doc = ?1")
                    .unwrap()
                    .execute([*doc])
                    .unwrap();
            }
            write_norms(&tx, chunks);
            for (field, tokens) in removed.iter().enumerate() {
                tx.execute(
                    "update stats set tokens = tokens - ?1 where field = ?2",
                    params![*tokens as i64, field as i64],
                )
                .unwrap();
            }
            purged = std::mem::take(&mut under_way.docs);
            *purge = None;
        }
        tx.commit().unwrap();
        Purged {
            keys,
            postings,
            millis: started.elapsed().as_secs_f64() * 1e3,
            purged,
        }
    }

    /// Sets each document by its key, all in one commit; the number is
    /// ignored. Says the numbers it marked gone.
    pub fn set(&mut self, analyzer: &Analyzer, docs: &[(u32, &Doc)]) -> Vec<u32> {
        let tx = self.connection.transaction().unwrap();
        let mut gone = Vec::new();
        for (_, doc) in docs {
            gone.extend(forget(&tx, &doc.id));
            let id = self.next_doc;
            self.next_doc += 1;
            let mut head = tx
                .prepare_cached("insert into head (doc, key, tf, len) values (?1, ?2, ?3, ?4)")
                .unwrap();
            for (field, terms) in fields(analyzer, doc).iter().enumerate() {
                for (term, tf) in crate::blocks::counted(terms) {
                    head.execute(params![id, key(field, term), tf, terms.len() as i64])
                        .unwrap();
                }
            }
            tx.prepare_cached("insert into docs (id, key, title, body) values (?1, ?2, ?3, ?4)")
                .unwrap()
                .execute(params![id, doc.id, doc.title, doc.text])
                .unwrap();
        }
        tx.commit().unwrap();
        gone
    }

    pub fn head_rows(&self) -> i64 {
        self.connection
            .query_row("select count(*) from head", [], |row| row.get(0))
            .unwrap()
    }

    pub fn delete(&mut self, key: &str) -> Vec<u32> {
        let tx = self.connection.transaction().unwrap();
        let gone = forget(&tx, key);
        tx.commit().unwrap();
        gone.into_iter().collect()
    }

    /// Appends every head posting to its key's list: a key short enough stays
    /// in its row, a list's short last block is filled before new blocks
    /// follow it, and the documents' lengths go to their row of norms.
    pub fn pack(&mut self) -> Packed {
        let started = Instant::now();
        let tx = self.connection.transaction().unwrap();
        let rows: Vec<(String, Posting)> = tx
            .prepare("select key, doc, tf, len from head order by key, doc")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Posting {
                        doc: row.get::<_, i64>(1)? as u32,
                        tf: row.get::<_, i64>(2)? as u32,
                        len: row.get::<_, i64>(3)? as u32,
                    },
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut lengths: HashMap<u32, [u16; 4]> = HashMap::new();
        let mut keys = 0;
        let mut at = 0;
        while at < rows.len() {
            let key = rows[at].0.clone();
            let field = FIELDS
                .iter()
                .position(|(name, _)| key.starts_with(&format!("{name}|")))
                .unwrap();
            let mut fresh = Vec::new();
            while at < rows.len() && rows[at].0 == key {
                let posting = rows[at].1;
                lengths.entry(posting.doc).or_default()[field] =
                    posting.len.min(u32::from(u16::MAX)) as u16;
                fresh.push(posting);
                at += 1;
            }
            append(&tx, &mut self.next_key, &key, &fresh);
            keys += 1;
        }
        let mut chunks: HashMap<u32, Vec<u16>> = HashMap::new();
        for (doc, fields) in &lengths {
            let chunk = chunks.entry(doc / NORMS).or_insert_with(|| {
                let bytes: Vec<u8> = tx
                    .query_row(
                        "select lengths from norms where chunk = ?1",
                        [doc / NORMS],
                        |row| row.get(0),
                    )
                    .optional()
                    .unwrap()
                    .unwrap_or_default();
                lengths_of(&bytes)
            });
            let base = ((doc % NORMS) as usize) * 4;
            chunk[base..base + 4].copy_from_slice(fields);
        }
        for (chunk, values) in chunks {
            let bytes: Vec<u8> = values
                .iter()
                .flat_map(|length| length.to_le_bytes())
                .collect();
            tx.execute(
                "insert or replace into norms (chunk, lengths) values (?1, ?2)",
                params![chunk, bytes],
            )
            .unwrap();
        }
        for field in 0..4 {
            let added: u64 = lengths
                .values()
                .map(|fields| u64::from(fields[field]))
                .sum();
            tx.execute(
                "update stats set tokens = tokens + ?1, documents = (select count(*) from docs) where field = ?2",
                params![added as i64, field as i64],
            )
            .unwrap();
        }
        tx.execute("delete from head", []).unwrap();
        tx.commit().unwrap();
        Packed {
            documents: lengths.len(),
            keys,
            postings: rows.len(),
            millis: started.elapsed().as_secs_f64() * 1e3,
        }
    }
}

/// Appends a key's head postings to its list.
fn append(tx: &rusqlite::Transaction, next_key: &mut i64, key: &str, fresh: &[Posting]) {
    let found: Option<(i64, i64, Option<Vec<u8>>, Option<Vec<u8>>)> = tx
        .prepare_cached("select id, documents, directory, inline from ckeys where key = ?1")
        .unwrap()
        .query_row([key], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()
        .unwrap();
    let (id, documents, directory, inline) = found.unwrap_or_else(|| {
        let id = *next_key;
        *next_key += 1;
        (id, 0, None, Some(Vec::new()))
    });
    let documents = documents + fresh.len() as i64;
    let mut blob = tx
        .prepare_cached("insert or replace into cblob (id, first, payload) values (?1, ?2, ?3)")
        .unwrap();
    let mut row = tx
            .prepare_cached("insert or replace into ckeys (key, id, documents, directory, inline) values (?1, ?2, ?3, ?4, ?5)")
            .unwrap();
    if let Some(inline) = inline {
        let mut all = unpack_inline(&inline, &HashSet::new());
        all.extend_from_slice(fresh);
        if all.len() <= INLINE {
            row.execute(params![
                key,
                id,
                documents,
                None::<Vec<u8>>,
                pack_inline(&all)
            ])
            .unwrap();
            return;
        }
        let mut entries = Vec::new();
        for block in all.chunks(BLOCK) {
            entries.push(entry_of(block));
            blob.execute(params![id, block[0].doc, pack(block)])
                .unwrap();
        }
        row.execute(params![
            key,
            id,
            documents,
            encode_directory(&entries),
            None::<Vec<u8>>
        ])
        .unwrap();
        return;
    }
    let mut entries = decode_directory(&directory.unwrap());
    let last = *entries.last().unwrap();
    let held: Vec<(u32, u32)> = unpack_raw(
        &tx.query_row(
            "select payload from cblob where id = ?1 and first = ?2",
            params![id, last.first],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .unwrap(),
    );
    let room = BLOCK.saturating_sub(held.len()).min(fresh.len());
    let (filling, rest) = fresh.split_at(room);
    if !filling.is_empty() {
        let mut block: Vec<Posting> = held
            .iter()
            .map(|(doc, tf)| Posting {
                doc: *doc,
                tf: *tf,
                len: 0,
            })
            .collect();
        block.extend_from_slice(filling);
        let entry = entries.last_mut().unwrap();
        entry.last = filling[filling.len() - 1].doc;
        entry.most_tf = entry
            .most_tf
            .max(filling.iter().map(|posting| posting.tf).max().unwrap());
        entry.least_len = entry
            .least_len
            .min(filling.iter().map(|posting| posting.len).min().unwrap());
        blob.execute(params![id, last.first, pack(&block)]).unwrap();
    }
    for block in rest.chunks(BLOCK) {
        entries.push(entry_of(block));
        blob.execute(params![id, block[0].doc, pack(block)])
            .unwrap();
    }
    row.execute(params![
        key,
        id,
        documents,
        encode_directory(&entries),
        None::<Vec<u8>>
    ])
    .unwrap();
}

/// Marks a key's document gone from the blocks and drops its head postings;
/// says its number.
fn forget(tx: &rusqlite::Transaction, key: &str) -> Option<u32> {
    let old: Option<i64> = tx
        .prepare_cached("select id from docs where key = ?1")
        .unwrap()
        .query_row([key], |row| row.get(0))
        .optional()
        .unwrap();
    if let Some(old) = old {
        tx.prepare_cached("insert or ignore into gone (doc) values (?1)")
            .unwrap()
            .execute([old])
            .unwrap();
        tx.prepare_cached("delete from head where doc = ?1")
            .unwrap()
            .execute([old])
            .unwrap();
        tx.prepare_cached("insert or replace into gone_text (doc, title, body) select id, title, body from docs where id = ?1")
            .unwrap()
            .execute([old])
            .unwrap();
        tx.prepare_cached("delete from docs where id = ?1")
            .unwrap()
            .execute([old])
            .unwrap();
    }
    old.map(|old| old as u32)
}

/// A key's field, from the name it starts with.
fn field_of(key: &str) -> usize {
    FIELDS
        .iter()
        .position(|(name, _)| key.starts_with(&format!("{name}|")))
        .unwrap()
}

fn norms_of(tx: &rusqlite::Transaction, chunk: u32) -> Vec<u16> {
    let bytes: Vec<u8> = tx
        .query_row(
            "select lengths from norms where chunk = ?1",
            [chunk],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
        .unwrap_or_default();
    lengths_of(&bytes)
}

fn write_norms(tx: &rusqlite::Transaction, chunks: HashMap<u32, Vec<u16>>) {
    for (chunk, values) in chunks {
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|length| length.to_le_bytes())
            .collect();
        tx.execute(
            "insert or replace into norms (chunk, lengths) values (?1, ?2)",
            params![chunk, bytes],
        )
        .unwrap();
    }
}

/// Takes `docs`, sorted, out of a key's list: from its own row if short,
/// else from the blocks whose range holds one, each written again or dropped
/// when emptied, its directory with them. Says how many postings it took.
fn take_out(
    tx: &rusqlite::Transaction,
    key: &str,
    docs: &[u32],
    norms: &mut HashMap<u32, Vec<u16>>,
) -> usize {
    let found: Option<(i64, i64, Option<Vec<u8>>, Option<Vec<u8>>)> = tx
        .prepare_cached("select id, documents, directory, inline from ckeys where key = ?1")
        .unwrap()
        .query_row([key], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()
        .unwrap();
    // A key no pack reached held its documents in the head, which a set empties.
    let Some((id, documents, directory, inline)) = found else {
        return 0;
    };
    let gone: HashSet<u32> = docs.iter().copied().collect();
    let field = field_of(key);
    if let Some(inline) = inline {
        let before = unpack_inline(&inline, &HashSet::new()).len();
        let kept = unpack_inline(&inline, &gone);
        let taken = before - kept.len();
        if taken > 0 {
            write_key(
                tx,
                key,
                id,
                kept.len() as i64,
                None,
                (!kept.is_empty()).then(|| pack_inline(&kept)),
            );
        }
        return taken;
    }
    let mut entries = decode_directory(&directory.unwrap());
    let mut touched: Vec<usize> = docs
        .iter()
        .filter_map(|doc| {
            let at = entries
                .partition_point(|entry| entry.first <= *doc)
                .checked_sub(1)?;
            (*doc <= entries[at].last).then_some(at)
        })
        .collect();
    touched.dedup();
    let mut taken = 0;
    for at in touched.into_iter().rev() {
        let first = entries[at].first;
        let held = unpack_raw(
            &tx.query_row(
                "select payload from cblob where id = ?1 and first = ?2",
                params![id, first],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .unwrap(),
        );
        let kept: Vec<Posting> = held
            .iter()
            .filter(|(doc, _)| !gone.contains(doc))
            .map(|(doc, tf)| {
                let chunk = norms
                    .entry(doc / NORMS)
                    .or_insert_with(|| norms_of(tx, doc / NORMS));
                Posting {
                    doc: *doc,
                    tf: *tf,
                    len: u32::from(chunk[(doc % NORMS) as usize * 4 + field]),
                }
            })
            .collect();
        taken += held.len() - kept.len();
        if kept.len() == held.len() {
            continue;
        }
        tx.prepare_cached("delete from cblob where id = ?1 and first = ?2")
            .unwrap()
            .execute(params![id, first])
            .unwrap();
        if kept.is_empty() {
            entries.remove(at);
        } else {
            entries[at] = entry_of(&kept);
            tx.prepare_cached("insert into cblob (id, first, payload) values (?1, ?2, ?3)")
                .unwrap()
                .execute(params![id, kept[0].doc, pack(&kept)])
                .unwrap();
        }
    }
    let left = documents - taken as i64;
    let directory = (!entries.is_empty()).then(|| encode_directory(&entries));
    write_key(tx, key, id, left, directory, None);
    taken
}

/// A key's row as it now stands, or none once it holds nothing.
fn write_key(
    tx: &rusqlite::Transaction,
    key: &str,
    id: i64,
    documents: i64,
    directory: Option<Vec<u8>>,
    inline: Option<Vec<u8>>,
) {
    if directory.is_none() && inline.is_none() {
        tx.prepare_cached("delete from ckeys where key = ?1")
            .unwrap()
            .execute([key])
            .unwrap();
        return;
    }
    tx.prepare_cached("insert or replace into ckeys (key, id, documents, directory, inline) values (?1, ?2, ?3, ?4, ?5)")
        .unwrap()
        .execute(params![key, id, documents, directory, inline])
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_and_a_short_list_read_back_as_written() {
        let entries = [
            Entry {
                first: 4,
                last: 900,
                most_tf: 3,
                least_len: 7,
            },
            Entry {
                first: 901,
                last: 70_000,
                most_tf: 1,
                least_len: 2,
            },
        ];
        let back = decode_directory(&encode_directory(&entries));
        assert_eq!(
            back.iter()
                .map(|entry| (entry.first, entry.last, entry.most_tf, entry.least_len))
                .collect::<Vec<_>>(),
            [(4, 900, 3, 7), (901, 70_000, 1, 2)]
        );
        let short = [
            Posting {
                doc: 5,
                tf: 2,
                len: 30,
            },
            Posting {
                doc: 9,
                tf: 1,
                len: 4,
            },
        ];
        let read = unpack_inline(&pack_inline(&short), &HashSet::from([5]));
        assert_eq!(
            read.iter()
                .map(|posting| (posting.doc, posting.tf, posting.len))
                .collect::<Vec<_>>(),
            [(9, 1, 4)]
        );
    }
}
