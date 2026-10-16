//! The index in SQLite's FTS5, opened as TinyStore opens a file: 4 KiB
//! pages, WAL, a 1 MiB cache a connection.
//!
//! The analyzer runs in Rust and FTS5 is given its tokens joined by spaces,
//! through FTS5's `ascii` tokenizer: every byte past ASCII is a token's, and
//! a token holds no ASCII byte but letters and digits, so FTS5 splits only
//! where the analyzer did. A title and a body are two columns each, the words
//! as written and their stems, a stem only where it differs:
//!
//! ```text
//! title "Кошки и running"  →  t  "кошки и running"   ts "кошк run"
//! ```
//!
//! A query word `w` with stem `s` asks `{t b}:"w" OR {ts bs}:"s" OR {t b}:"s"`,
//! which finds a document's word whose stem is `s` whether it was kept as
//! written (its stem was itself) or as a stem.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use rusqlite::{Connection, OptionalExtension, params};

use crate::analyze::Analyzer;
use crate::corpus::Doc;
use crate::plan::{Backend, Word};

/// bm25's weights: a title's word over a body's, a word as written over its stem.
const RANK: &str = "bm25(2.0, 1.5, 1.0, 0.75)";

pub fn open(path: &Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection.pragma_update(None, "page_size", 4096).unwrap();
    connection
        .pragma_update(None, "journal_mode", "wal")
        .unwrap();
    connection
        .pragma_update(None, "synchronous", "full")
        .unwrap();
    connection.pragma_update(None, "cache_size", -1024).unwrap();
    connection
}

pub struct Built {
    pub seconds: f64,
    /// Bytes by b-tree, from dbstat.
    pub objects: Vec<(String, u64)>,
    pub file: u64,
}

/// Indexes every document, a thousand a transaction, then merges FTS5's
/// segments into one, as a store's maintenance would in time.
pub fn build(path: &Path, analyzer: &Analyzer, docs: &[Doc]) -> Built {
    let started = Instant::now();
    let mut connection = open(path);
    connection
        .execute_batch(&format!(
            "create virtual table idx using fts5(t, ts, b, bs, content='', contentless_delete=1, tokenize='ascii');
             insert into idx(idx, rank) values ('rank', '{RANK}');"
        ))
        .unwrap();
    for (chunk, docs) in docs.chunks(1000).enumerate() {
        let tx = connection.transaction().unwrap();
        {
            let mut insert = tx
                .prepare("insert into idx(rowid, t, ts, b, bs) values (?1, ?2, ?3, ?4, ?5)")
                .unwrap();
            for (at, doc) in docs.iter().enumerate() {
                let (t, ts) = columns(analyzer, &doc.title);
                let (b, bs) = columns(analyzer, &doc.text);
                insert
                    .execute(params![(chunk * 1000 + at + 1) as i64, t, ts, b, bs])
                    .unwrap();
            }
        }
        tx.commit().unwrap();
    }
    connection
        .execute("insert into idx(idx) values ('optimize')", [])
        .unwrap();
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
    }
}

/// A text's words as written and its stems, each joined by spaces.
pub fn columns(analyzer: &Analyzer, text: &str) -> (String, String) {
    let tokens = analyzer.tokens(text);
    let surface = tokens
        .iter()
        .map(|token| token.surface.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let stems = tokens
        .iter()
        .filter_map(|token| token.stem.as_deref())
        .collect::<Vec<_>>()
        .join(" ");
    (surface, stems)
}

pub struct Fts {
    connection: Connection,
    documents: u64,
    /// Documents a term is in, for the terms in more than `KEPT` of them:
    /// the statistics a store would keep beside an index, since counting a
    /// frequent term reads its whole doclist.
    frequent: HashMap<String, u64>,
    pub opened: Opened,
}

/// What opening cost: the frequent terms' table above all.
pub struct Opened {
    pub frequent_terms: usize,
    pub frequent_millis: f64,
}

/// A term kept in memory once it is in more than this share of the documents.
const KEPT: f64 = 0.005;

impl Fts {
    pub fn open(path: &Path) -> Fts {
        let connection = open(path);
        // A temporary table is a write: query_only would refuse it.
        connection
            .execute_batch("create virtual table temp.vocab using fts5vocab(main, idx, 'row');")
            .unwrap();
        connection.pragma_update(None, "query_only", true).unwrap();
        let documents = connection
            .query_row("select count(*) from idx", [], |row| row.get::<_, i64>(0))
            .unwrap() as u64;
        let started = Instant::now();
        let least = (documents as f64 * KEPT) as i64;
        let frequent: HashMap<String, u64> = connection
            .prepare("select term, doc from temp.vocab where doc > ?1")
            .unwrap()
            .query_map([least], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let opened = Opened {
            frequent_terms: frequent.len(),
            frequent_millis: started.elapsed().as_secs_f64() * 1e3,
        };
        Fts {
            connection,
            documents,
            frequent,
            opened,
        }
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

impl Backend for Fts {
    fn documents(&self) -> u64 {
        self.documents
    }

    fn frequency(&mut self, term: &str) -> u64 {
        if let Some(count) = self.frequent.get(term) {
            return *count;
        }
        let mut statement = self
            .connection
            .prepare_cached("select doc from temp.vocab where term = ?1")
            .unwrap();
        statement
            .query_row([term], |row| row.get::<_, i64>(0))
            .optional()
            .unwrap()
            .unwrap_or(0) as u64
    }

    fn top(&mut self, words: &[Word], every: bool, k: usize) -> Vec<u32> {
        if words.is_empty() {
            return Vec::new();
        }
        let joiner = if every { " AND " } else { " OR " };
        let expression = words.iter().map(clause).collect::<Vec<_>>().join(joiner);
        let mut statement = self
            .connection
            .prepare_cached("select rowid from idx where idx match ?1 order by rank limit ?2")
            .unwrap();
        statement
            .query_map(params![expression, k as i64], |row| row.get::<_, i64>(0))
            .unwrap()
            .map(|id| id.unwrap() as u32)
            .collect()
    }
}

/// A word's forms, any of which matches: `({t b}:"w" OR {ts bs}:"s" OR {t b}:"s" OR "typo")`.
fn clause(word: &Word) -> String {
    let mut forms = vec![format!("{{t b}}:\"{}\"", word.surface)];
    match &word.stem {
        Some(stem) => {
            forms.push(format!("{{ts bs}}:\"{stem}\""));
            forms.push(format!("{{t b}}:\"{stem}\""));
        }
        None => forms.push(format!("{{ts bs}}:\"{}\"", word.surface)),
    }
    forms.extend(word.typos.iter().map(|typo| format!("\"{typo}\"")));
    format!("({})", forms.join(" OR "))
}
