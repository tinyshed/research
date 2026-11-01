//! TinyStore's own search, the engine of the `main` that `tinystore/source`
//! holds, built from the token stream and read through its public API: a
//! passage is a document whose value is its tokens, its id the passage's
//! number, and a query is its terms joined by spaces. TinyStore analyzes
//! both again; the tokens are lower-case words already, which its analyzer
//! keeps as they are, and English is not stemmed, so it indexes the stream
//! as the other engines do.
//!
//! The build sets a thousand passages a commit and packs the head after
//! each, as the store's background work does once a thousand documents
//! wait; the store commits to the operating system alone, as the other
//! engines' builds do. A search reads the ten hits' values too, which every
//! search of TinyStore does.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use tinystore::search::Index;
use tinystore::{Durability, Options, Store};

const INDEX: &str = "msmarco";
const BATCH: usize = 1000;

fn options() -> Options {
    Options { background: false, durability: Some(Durability::Os), ..Options::default() }
}

fn open(store: &Store) -> Index<String> {
    store.index::<String>(INDEX).typo_tolerance(false).open().unwrap()
}

pub fn build(dir: &Path, store_dir: &Path) -> Value {
    let started = Instant::now();
    std::fs::create_dir_all(store_dir).unwrap();
    let store = Store::open(store_dir, options()).unwrap();
    let index = open(&store);
    let input = BufReader::with_capacity(1 << 22, File::open(dir.join("tokens.tsv")).unwrap());
    let mut batch: Vec<(String, String)> = Vec::with_capacity(BATCH);
    let (mut rows, mut setting, mut packing) = (0u64, 0.0, 0.0);
    let mut flush = |batch: &mut Vec<(String, String)>| {
        let set = Instant::now();
        index.set_many(batch.iter().map(|(id, tokens)| (id.as_str(), tokens))).unwrap();
        setting += set.elapsed().as_secs_f64();
        let pack = Instant::now();
        tinystore::search::maintain(&store).unwrap();
        packing += pack.elapsed().as_secs_f64();
        batch.clear();
    };
    for line in input.lines() {
        let line = line.unwrap();
        let (pid, tokens) = line.split_once('\t').unwrap();
        batch.push((pid.to_owned(), tokens.to_owned()));
        rows += 1;
        if batch.len() == BATCH {
            flush(&mut batch);
        }
        if rows % 1_000_000 == 0 {
            eprintln!("tinystore {rows} documents, {:.0} s", started.elapsed().as_secs_f64());
        }
    }
    flush(&mut batch);
    let seconds = started.elapsed().as_secs_f64();
    drop(index);
    store.close().unwrap();
    drop(store);
    json!({"step": "build", "engine": "tinystore", "documents": rows, "seconds": seconds,
        "set_seconds": setting, "pack_seconds": packing, "objects": objects(&store_dir.join("search.db")),
        "postings": postings(&store_dir.join("search.db")), "memory": crate::memory()})
}

/// Each b-tree of the file, its bytes, pages and payload, by `dbstat`.
fn objects(file: &Path) -> Value {
    let connection = rusqlite::Connection::open(file).unwrap();
    let mut select = connection
        .prepare("select name, sum(pgsize), count(*), sum(payload) from dbstat group by name order by 2 desc")
        .unwrap();
    let rows = select
        .query_map([], |row| {
            Ok(json!({"name": row.get::<_, String>(0)?, "bytes": row.get::<_, i64>(1)?,
                "pages": row.get::<_, i64>(2)?, "payload": row.get::<_, i64>(3)?}))
        })
        .unwrap();
    Value::Array(rows.map(Result::unwrap).collect())
}

/// The postings of the words of every document, which a search's keys hold.
fn postings(file: &Path) -> i64 {
    let connection = rusqlite::Connection::open(file).unwrap();
    connection
        .query_row("select sum(documents) from _tinystore_search_keys where part < 1073741824", [], |row| row.get(0))
        .unwrap()
}

/// TinyStore's store and index, the index before the store so that it drops first.
pub struct Reader {
    index: Index<String>,
    _store: Store,
}

impl Reader {
    pub fn open(store_dir: &Path) -> Reader {
        let store = Store::open(store_dir, options()).unwrap();
        Reader { index: open(&store), _store: store }
    }

    pub fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        let found = self.index.search(&terms.join(" ")).limit(k).run().unwrap();
        found.hits.into_iter().map(|hit| (hit.score, hit.id.parse::<u32>().unwrap() + 1)).collect()
    }
}
