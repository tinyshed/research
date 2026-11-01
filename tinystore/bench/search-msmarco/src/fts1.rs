//! FTS5 over the one field, as the lab's fts.rs indexes four: the tokens
//! joined by spaces through FTS5's `ascii` tokenizer, which splits only where
//! the analyzer did, a contentless table whose rows can be deleted, rowid
//! pid + 1, and FTS5's own `bm25()` (k1 1.2, b 0.75, fixed in its source).
//! A query is an OR of its terms, each a phrase of one token; FTS5 scores
//! every row that holds any of them and sorts them.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::fts::open;

pub fn build(dir: &Path, path: &Path) -> Value {
    let started = Instant::now();
    let mut connection = open(path);
    connection
        .execute_batch(
            "create virtual table idx using fts5(b, content='', contentless_delete=1, tokenize='ascii');
             insert into idx(idx, rank) values ('rank', 'bm25()');",
        )
        .unwrap();
    let input = BufReader::with_capacity(1 << 22, File::open(dir.join("tokens.tsv")).unwrap());
    let mut lines = input.lines().peekable();
    let mut rows = 0u64;
    while lines.peek().is_some() {
        let tx = connection.transaction().unwrap();
        {
            let mut insert = tx.prepare_cached("insert into idx(rowid, b) values (?1, ?2)").unwrap();
            for line in lines.by_ref().take(1000) {
                let line = line.unwrap();
                let (pid, tokens) = line.split_once('\t').unwrap();
                insert.execute(params![pid.parse::<i64>().unwrap() + 1, tokens]).unwrap();
                rows += 1;
            }
        }
        tx.commit().unwrap();
        if rows % 1_000_000 == 0 {
            eprintln!("fts {rows} rows, {:.0} s", started.elapsed().as_secs_f64());
        }
    }
    let inserted = started.elapsed().as_secs_f64();
    connection.execute("insert into idx(idx) values ('optimize')", []).unwrap();
    connection.pragma_update(None, "wal_checkpoint", "truncate").unwrap();
    let seconds = started.elapsed().as_secs_f64();
    let objects = crate::compact1::objects(&connection);
    let postings: i64 = {
        connection
            .execute_batch("create virtual table temp.vocab using fts5vocab(main, idx, 'row');")
            .unwrap();
        connection
            .query_row("select sum(doc) from temp.vocab", [], |row| row.get(0))
            .unwrap()
    };
    drop(connection);
    json!({"step": "build", "engine": "fts5", "rows": rows, "insert_seconds": inserted, "seconds": seconds,
        "file": std::fs::metadata(path).unwrap().len(), "postings": postings, "objects": objects,
        "memory": crate::memory()})
}

pub struct Reader {
    connection: Connection,
}

impl Reader {
    pub fn open(path: &Path) -> Reader {
        let connection = open(path);
        connection.pragma_update(None, "query_only", true).unwrap();
        Reader { connection }
    }

    /// The best `k` rows for any of `terms`, with bm25's score made positive.
    pub fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        if terms.is_empty() {
            return Vec::new();
        }
        let expression: Vec<String> = terms.iter().map(|term| format!("\"{term}\"")).collect();
        self.connection
            .prepare_cached("select rowid, rank from idx where idx match ?1 order by rank limit ?2")
            .unwrap()
            .query_map(params![expression.join(" OR "), k as i64], |row| {
                Ok((-row.get::<_, f64>(1)?, row.get::<_, i64>(0)? as u32))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }
}

/// A set replaces a row's postings by its rowid, all of a call in one commit.
pub struct Writer {
    connection: Connection,
}

impl Writer {
    pub fn open(path: &Path) -> Writer {
        Writer { connection: open(path) }
    }

    pub fn set(&mut self, rows: &[(u32, String)]) {
        let tx = self.connection.transaction().unwrap();
        {
            let mut forget = tx.prepare_cached("delete from idx where rowid = ?1").unwrap();
            let mut insert = tx.prepare_cached("insert into idx(rowid, b) values (?1, ?2)").unwrap();
            for (id, tokens) in rows {
                forget.execute([*id]).unwrap();
                insert.execute(params![*id, tokens]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
}
