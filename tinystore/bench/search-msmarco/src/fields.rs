//! search-fields: two fields of MS MARCO's passages, the words as written
//! (`b`) and their stems (`bs`), as the search lab keeps a body, and queries
//! whose last word is a prefix. Three layouts answer the same lists:
//!
//! - `lab`: compact.rs's tables, a key a field and a word with the field's
//!   name in front, `b|word` and `bs|word`, read by the lab's own reader,
//!   which asks every form of a word in each of its four fields.
//! - `native`: one dictionary row a word holding its documents in each field;
//!   a list's rows by rowid, word × 4 + field, its blocks bit-packed in one
//!   blob and its directory fixed-width, as fast2.rs lays them; one byte of
//!   length a document a field. A prefix is one range of the dictionary.
//! - `tantivy`: the same two fields, a term query a form, boosted as the lab
//!   weighs its fields; a prefix one range of its term dictionary.
//!
//! A word's forms are compact.rs's in fields 2 and 3: as written in `b`, its
//! stem in `bs`, and its stem in `b` where it differs. A prefix becomes the 32
//! words of `b` that begin with it and hold the most documents, ties by word,
//! each a word with no stem of its own; every engine expands it in its own
//! dictionary and scores the same lists.
//!
//! ```text
//! msmarco fields-queries <corpus> <stems out>                 q/fields-sample.tsv and q/prefix-sample.tsv
//! msmarco fields-build lab <stems out> <surface out> <lab.db>
//! msmarco fields-build native <lab.db> <native.db>
//! msmarco fields-build tantivy <stems out> <surface out> <index>
//! msmarco fields-answer <lab|lab-exhaustive|native|tantivy> <index> <queries> <results>
//! msmarco fields-time <lab|native|tantivy> <index> <queries> <results> <warm|cold>
//! ```
//!
//! FIELDS_EQUAL=1 weighs both fields 1.0 in `native` and `tantivy`, and asks
//! Tantivy plain term queries, no `BoostQuery`: whether a boost costs Tantivy
//! its block-max path is what it measures.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use flate2::read::GzDecoder;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::analyze::{Analysis, Analyzer};
use crate::blocks::{END, FIELDS, Posting, get_varint, idf, key, put_varint, saturated};
use crate::collection::{Collection, read_terms};
use crate::compact1::{Entry, INLINE, KEYS_A_COMMIT, NORMS, SCHEMA, encode_directory, pack_inline};
use crate::fts::open;

pub const B: usize = 2;
pub const BS: usize = 3;
const EXPAND: usize = 32;
const SEEDS: usize = 10;

/// A field's weight: the lab's, or 1.0 under FIELDS_EQUAL=1.
fn weight(field: usize) -> f64 {
    static EQUAL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *EQUAL.get_or_init(|| std::env::var("FIELDS_EQUAL").as_deref() == Ok("1")) { 1.0 } else { FIELDS[field].1 }
}

// --- the queries -------------------------------------------------------------

/// A query: its words, each as written and its stem where that differs, and
/// the prefix its last word was cut to.
pub struct Asked {
    pub qid: String,
    pub words: Vec<(String, Option<String>)>,
    pub prefix: Option<String>,
}

fn texts(corpus: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut add = |reader: Box<dyn BufRead>| {
        for line in reader.lines().map(Result::unwrap) {
            if let Some((qid, text)) = line.split_once('\t') {
                out.insert(qid.to_owned(), text.to_owned());
            }
        }
    };
    add(Box::new(BufReader::new(File::open(corpus.join("queries.dev.small.tsv")).unwrap())));
    for name in ["msmarco-test2019-queries.tsv.gz", "msmarco-test2020-queries.tsv.gz"] {
        add(Box::new(BufReader::new(GzDecoder::new(File::open(corpus.join(name)).unwrap()))));
    }
    out
}

fn word_text((surface, stem): &(String, Option<String>)) -> String {
    match stem {
        Some(stem) => format!("{surface}/{stem}"),
        None => surface.clone(),
    }
}

/// The sample's queries as words: q/fields-sample.tsv every query, and
/// q/prefix-sample.tsv those whose last word has four letters or more, cut
/// to its first half, three letters at least.
pub fn queries(corpus: &Path, stems: &Path) -> Value {
    let analyzer = Analyzer::new(Analysis::named("auto"));
    let texts = texts(corpus);
    let sample: Vec<String> = crate::queries::read(&stems.join("q/sample.tsv")).into_iter().map(|(qid, _)| qid).collect();
    let mut all = File::create(stems.join("q/fields-sample.tsv")).unwrap();
    let mut prefixed = File::create(stems.join("q/prefix-sample.tsv")).unwrap();
    let (mut words_total, mut prefixes) = (0usize, 0usize);
    for qid in &sample {
        let mut seen = HashSet::new();
        let words: Vec<(String, Option<String>)> = analyzer
            .tokens(&texts[qid])
            .into_iter()
            .filter(|token| seen.insert(token.surface.clone()))
            .map(|token| (token.surface, token.stem))
            .collect();
        assert!(words.iter().all(|(surface, _)| !surface.contains('/')));
        words_total += words.len();
        let joined: Vec<String> = words.iter().map(word_text).collect();
        writeln!(all, "{qid}\t{}", joined.join(" ")).unwrap();
        if let Some((last, rest)) = words.split_last() {
            let letters: Vec<char> = last.0.chars().collect();
            if letters.len() >= 4 {
                let cut = letters.len().div_ceil(2).max(3);
                let prefix: String = letters[..cut].iter().collect();
                let joined: Vec<String> = rest.iter().map(word_text).collect();
                writeln!(prefixed, "{qid}\t{}\t{prefix}", joined.join(" ")).unwrap();
                prefixes += 1;
            }
        }
    }
    json!({"step": "fields-queries", "queries": sample.len(), "words": words_total, "prefix_queries": prefixes})
}

pub fn read_asked(path: &Path) -> Vec<Asked> {
    BufReader::new(File::open(path).unwrap())
        .lines()
        .map(Result::unwrap)
        .map(|line| {
            let mut parts = line.split('\t');
            let qid = parts.next().unwrap().to_owned();
            let words = parts
                .next()
                .unwrap_or("")
                .split(' ')
                .filter(|word| !word.is_empty())
                .map(|word| match word.split_once('/') {
                    Some((surface, stem)) => (surface.to_owned(), Some(stem.to_owned())),
                    None => (word.to_owned(), None),
                })
                .collect();
            let prefix = parts.next().map(str::to_owned);
            Asked { qid, words, prefix }
        })
        .collect()
}

/// compact.rs's forms of each word in fields 2 and 3, in its order.
fn forms(words: &[(String, Option<String>)]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (surface, stem) in words {
        let base = stem.as_deref().unwrap_or(surface);
        out.push((B, surface.clone()));
        out.push((BS, base.to_owned()));
        if stem.is_some() {
            out.push((B, base.to_owned()));
        }
    }
    out
}

/// The end of a prefix's range: no word that begins with it sorts after it.
fn past(prefix: &str) -> String {
    format!("{prefix}\u{10FFFF}")
}

// --- the lab's layout ---------------------------------------------------------

/// compact1::build over two prepared collections: `b|word` from the words
/// as written, `bs|word` from their stems, keys numbered field after field.
pub fn build_lab(stems: &Path, surface: &Path, path: &Path) -> Value {
    let started = Instant::now();
    let fields = [(B, surface), (BS, stems)];
    let collections: Vec<Collection> = fields.iter().map(|(_, dir)| Collection::open(dir)).collect();
    let documents = collections[0].documents as usize;
    assert_eq!(documents, collections[1].documents as usize);
    let mut connection = open(path);
    connection.execute_batch(SCHEMA).unwrap();
    let tx = connection.transaction().unwrap();
    {
        let mut keep = tx.prepare("insert into docs (id, key, title, body) values (?1, ?2, '', '')").unwrap();
        for pid in 0..documents {
            keep.execute(params![pid as i64 + 1, pid.to_string()]).unwrap();
        }
    }
    tx.commit().unwrap();
    let (mut postings, mut blocks, mut inline, mut id) = (0u64, 0u64, 0u64, 0i64);
    let mut tx = connection.transaction().unwrap();
    for ((field, dir), collection) in fields.iter().zip(&collections) {
        let names = read_terms(dir);
        for (number, term) in names.iter().enumerate() {
            if id > 0 && id as usize % KEYS_A_COMMIT == 0 {
                tx.commit().unwrap();
                tx = connection.transaction().unwrap();
            }
            id += 1;
            let (docs, freqs) = collection.list(number);
            let list: Vec<Posting> = docs
                .iter()
                .zip(freqs)
                .map(|(pid, tf)| Posting { doc: pid + 1, tf: *tf, len: collection.sizes[*pid as usize] })
                .collect();
            postings += list.len() as u64;
            let mut row = tx
                .prepare_cached("insert into ckeys (key, id, documents, directory, inline) values (?1, ?2, ?3, ?4, ?5)")
                .unwrap();
            if list.len() <= INLINE {
                inline += 1;
                row.execute(params![key(*field, term), id, list.len() as i64, None::<Vec<u8>>, pack_inline(&list)])
                    .unwrap();
                continue;
            }
            let mut entries = Vec::new();
            let mut blob = tx.prepare_cached("insert into cblob (id, first, payload) values (?1, ?2, ?3)").unwrap();
            for block in list.chunks(crate::blocks::BLOCK) {
                blocks += 1;
                entries.push(Entry {
                    first: block[0].doc,
                    last: block[block.len() - 1].doc,
                    most_tf: block.iter().map(|posting| posting.tf).max().unwrap(),
                    least_len: block.iter().map(|posting| posting.len).min().unwrap(),
                });
                blob.execute(params![id, block[0].doc, crate::compact1::pack(block)]).unwrap();
            }
            row.execute(params![key(*field, term), id, list.len() as i64, encode_directory(&entries), None::<Vec<u8>>])
                .unwrap();
        }
    }
    {
        let mut norms = vec![0u16; (documents + 1) * 4];
        for ((field, _), collection) in fields.iter().zip(&collections) {
            for (pid, size) in collection.sizes.iter().enumerate() {
                norms[(pid + 1) * 4 + field] = (*size).min(u32::from(u16::MAX)) as u16;
            }
        }
        let mut lengths = tx.prepare("insert into norms (chunk, lengths) values (?1, ?2)").unwrap();
        for (chunk, slice) in norms.chunks(NORMS as usize * 4).enumerate() {
            let bytes: Vec<u8> = slice.iter().flat_map(|length| length.to_le_bytes()).collect();
            lengths.execute(params![chunk as i64, bytes]).unwrap();
        }
        for field in 0..4 {
            let tokens = fields
                .iter()
                .zip(&collections)
                .find(|((f, _), _)| *f == field)
                .map_or(0, |(_, collection)| collection.tokens);
            tx.execute(
                "insert into stats (field, tokens, documents) values (?1, ?2, ?3)",
                params![field as i64, tokens as i64, documents as i64],
            )
            .unwrap();
        }
    }
    tx.commit().unwrap();
    connection.pragma_update(None, "wal_checkpoint", "truncate").unwrap();
    let objects = crate::compact1::objects(&connection);
    json!({"step": "fields-build", "layout": "lab", "seconds": started.elapsed().as_secs_f64(), "keys": id,
        "postings": postings, "blocks": blocks, "inline": inline, "file": std::fs::metadata(path).unwrap().len(),
        "objects": objects})
}

/// The lab's own reader, compact.rs's `Compact`, on the two fields: it asks
/// every form of a word in its four fields, two of them empty here, and a
/// prefix is a range of its keys, `b|` and the prefix.
pub struct Lab {
    compact: crate::compact::Compact,
    keys: Connection,
}

impl Lab {
    pub fn open(path: &Path, exhaustive: bool) -> Lab {
        assert_eq!(std::env::var("LAB_EXECUTOR").as_deref(), Ok("window"));
        let mut compact = crate::compact::Compact::open(path);
        compact.exhaustive = exhaustive;
        let keys = open(path);
        keys.pragma_update(None, "query_only", true).unwrap();
        Lab { compact, keys }
    }

    fn expand(&self, prefix: &str) -> Vec<String> {
        self.keys
            .prepare_cached(
                "select key from ckeys where key >= ?1 and key < ?2 order by documents desc, key limit ?3",
            )
            .unwrap()
            .query_map(params![key(B, prefix), key(B, &past(prefix)), EXPAND as i64], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|found| found.unwrap()[FIELDS[B].0.len() + 1..].to_owned())
            .collect()
    }

    pub fn top(&mut self, asked: &Asked, k: usize) -> Vec<(f64, u32)> {
        use crate::plan::{Backend, Word};
        let mut words: Vec<Word> = asked
            .words
            .iter()
            .map(|(surface, stem)| Word { surface: surface.clone(), stem: stem.clone(), typos: Vec::new() })
            .collect();
        if let Some(prefix) = &asked.prefix {
            for term in self.expand(prefix) {
                words.push(Word { surface: term, stem: None, typos: Vec::new() });
            }
        }
        let ids = self.compact.top(&words, false, k);
        self.compact.scores.iter().copied().zip(ids).collect()
    }
}

// --- the native layout --------------------------------------------------------

/// Writes the native layout from the lab's tables, the same postings.
pub fn build_native(lab: &Path, path: &Path) -> Value {
    let started = Instant::now();
    let source = open(lab);
    source.pragma_update(None, "query_only", true).unwrap();
    let mut average = [1.0f64; 4];
    let mut documents = 0i64;
    for (field, tokens, holding) in source
        .prepare("select field, tokens, documents from stats")
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)))
        .unwrap()
        .map(Result::unwrap)
    {
        average[field as usize] = (tokens as f64 / holding.max(1) as f64).max(1.0);
        documents = holding;
    }
    let mut lengths: [Vec<u32>; 4] = Default::default();
    {
        let mut rows = source.prepare("select chunk, lengths from norms order by chunk").unwrap();
        let mut read = rows.query([]).unwrap();
        while let Some(row) = read.next().unwrap() {
            let chunk: i64 = row.get(0).unwrap();
            let bytes: Vec<u8> = row.get(1).unwrap();
            let base = chunk as usize * NORMS as usize;
            for field in [B, BS] {
                if lengths[field].len() < base + NORMS as usize {
                    lengths[field].resize(base + NORMS as usize, 0);
                }
                for (i, doc) in bytes.chunks(8).enumerate() {
                    lengths[field][base + i] = u32::from(u16::from_le_bytes([doc[field * 2], doc[field * 2 + 1]]));
                }
            }
        }
    }
    // Every key: its field, its word, its documents, its short list if inline.
    type Key = (usize, String, i64, Option<Vec<u8>>);
    let mut keys: HashMap<i64, Key> = HashMap::new();
    let mut words: BTreeMap<String, [i64; 4]> = BTreeMap::new();
    for (name, id, holding, inline) in source
        .prepare("select key, id, documents, inline from ckeys")
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
    {
        let (prefix, word) = name.split_once('|').unwrap();
        let field = FIELDS.iter().position(|(short, _)| *short == prefix).unwrap();
        words.entry(word.to_owned()).or_default()[field] = holding;
        keys.insert(id, (field, word.to_owned(), holding, inline));
    }
    let numbers: HashMap<&str, i64> = words.keys().enumerate().map(|(n, word)| (word.as_str(), n as i64 + 1)).collect();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    let mut out = open(path);
    out.execute_batch(
        "create table words (word text primary key, id integer not null, b integer not null, bs integer not null) without rowid;
         create table lists (id integer primary key, payload blob not null);
         create table dirs (id integer primary key, directory blob not null, seeds blob not null);
         create table lengths (id integer primary key, bytes blob not null);
         create table long (field integer not null, doc integer not null, len integer not null, primary key (field, doc)) without rowid;
         create table meta (name text primary key, value real not null);",
    )
    .unwrap();
    let tx = out.transaction().unwrap();
    let (mut lists, mut blocks, mut list_bytes) = (0u64, 0u64, 0u64);
    {
        let mut put_word = tx.prepare("insert into words (word, id, b, bs) values (?1, ?2, ?3, ?4)").unwrap();
        for (word, held) in &words {
            put_word.execute(params![word, numbers[word.as_str()], held[B], held[BS]]).unwrap();
        }
        let mut put_list = tx.prepare("insert into lists (id, payload) values (?1, ?2)").unwrap();
        let mut put_dir = tx.prepare("insert into dirs (id, directory, seeds) values (?1, ?2, ?3)").unwrap();
        let mut write = |key: &Key, blocks_of: &[(Vec<u32>, Vec<u32>)], list_bytes: &mut u64| {
            let (field, word, _, _) = key;
            let len_of = |doc: u32| lengths[*field][doc as usize];
            let row = numbers[word.as_str()] * 4 + *field as i64;
            // A list too short for blocks: its postings as varints, no directory.
            if blocks_of.len() == 1 && blocks_of[0].0.len() <= INLINE {
                let mut payload = Vec::new();
                let mut previous = 0;
                for (doc, tf) in blocks_of[0].0.iter().zip(&blocks_of[0].1) {
                    put_varint(&mut payload, doc - previous);
                    put_varint(&mut payload, *tf);
                    previous = *doc;
                }
                *list_bytes += payload.len() as u64;
                put_list.execute(params![row, payload]).unwrap();
                return;
            }
            let (mut payload, mut directory, mut best) = (Vec::new(), Vec::new(), Vec::new());
            for (docs, tfs) in blocks_of {
                let top = docs
                    .iter()
                    .zip(tfs)
                    .map(|(doc, tf)| saturated(*tf, len_of(*doc), average[*field]))
                    .fold(0.0, f64::max);
                for (doc, tf) in docs.iter().zip(tfs) {
                    best.push((saturated(*tf, len_of(*doc), average[*field]), *doc, *tf, len_of(*doc)));
                }
                let offset = payload.len() as u32;
                payload.extend_from_slice(&crate::fast2::pack(docs, tfs));
                directory.extend_from_slice(&docs[0].to_le_bytes());
                directory.extend_from_slice(&docs[docs.len() - 1].to_le_bytes());
                directory.extend_from_slice(&crate::fast2::up(top).to_le_bytes());
                directory.extend_from_slice(&offset.to_le_bytes());
            }
            let order = |a: &(f64, u32, u32, u32), b: &(f64, u32, u32, u32)| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1));
            if best.len() > SEEDS {
                best.select_nth_unstable_by(SEEDS - 1, order);
                best.truncate(SEEDS);
            }
            best.sort_unstable_by(order);
            let mut seeds = Vec::new();
            for &(_, doc, tf, len) in &best {
                put_varint(&mut seeds, doc);
                put_varint(&mut seeds, tf);
                put_varint(&mut seeds, len);
            }
            *list_bytes += payload.len() as u64;
            put_list.execute(params![row, payload]).unwrap();
            put_dir.execute(params![row, directory, seeds]).unwrap();
        };
        // Short lists, one block each.
        for key in keys.values() {
            if let Some(inline) = &key.3 {
                let (mut docs, mut tfs) = (Vec::new(), Vec::new());
                let (mut at, mut doc) = (0, 0);
                while at < inline.len() {
                    doc += get_varint(inline, &mut at);
                    docs.push(doc);
                    tfs.push(get_varint(inline, &mut at));
                    get_varint(inline, &mut at);
                }
                write(key, &[(docs, tfs)], &mut list_bytes);
                lists += 1;
                blocks += 1;
            }
        }
        let mut scan = source.prepare("select id, payload from cblob order by id, first").unwrap();
        let mut rows = scan.query([]).unwrap();
        let mut current = -1i64;
        let mut gathered: Vec<(Vec<u32>, Vec<u32>)> = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            let id: i64 = row.get(0).unwrap();
            let raw: Vec<u8> = row.get(1).unwrap();
            if id != current {
                if current >= 0 {
                    write(&keys[&current], &gathered, &mut list_bytes);
                    lists += 1;
                }
                gathered.clear();
                current = id;
            }
            gathered.push(crate::fast2::unpack_varints(&raw));
            blocks += 1;
        }
        if current >= 0 {
            write(&keys[&current], &gathered, &mut list_bytes);
            lists += 1;
        }
        let mut put_len = tx.prepare("insert into lengths (id, bytes) values (?1, ?2)").unwrap();
        let mut put_long = tx.prepare("insert into long (field, doc, len) values (?1, ?2, ?3)").unwrap();
        for field in [B, BS] {
            for (chunk, slice) in lengths[field].chunks(NORMS as usize).enumerate() {
                let bytes: Vec<u8> = slice.iter().map(|len| (*len).min(255) as u8).collect();
                for (i, len) in slice.iter().enumerate() {
                    if *len >= 255 {
                        put_long.execute(params![field as i64, (chunk * NORMS as usize + i) as i64, *len as i64]).unwrap();
                    }
                }
                put_len.execute(params![(field * 100_000 + chunk) as i64, bytes]).unwrap();
            }
        }
        let mut put_meta = tx.prepare("insert into meta (name, value) values (?1, ?2)").unwrap();
        put_meta.execute(params!["documents", documents as f64]).unwrap();
        for field in [B, BS] {
            put_meta.execute(params![format!("average{field}"), average[field]]).unwrap();
        }
    }
    tx.commit().unwrap();
    out.pragma_update(None, "wal_checkpoint", "truncate").unwrap();
    let objects = crate::compact1::objects(&out);
    json!({"step": "fields-build", "layout": "native", "seconds": started.elapsed().as_secs_f64(),
        "words": words.len(), "lists": lists, "blocks": blocks, "list_bytes": list_bytes,
        "file": std::fs::metadata(path).unwrap().len(), "objects": objects})
}

/// A list of a query, as fast2.rs's cursor, with its field's lengths.
struct Cursor<'a> {
    field: usize,
    scale: f64,
    average: f64,
    firsts: Rc<Vec<u32>>,
    lasts: Rc<Vec<u32>>,
    bounds: Vec<f64>,
    offsets: Rc<Vec<u32>>,
    size: usize,
    blob: Option<rusqlite::blob::Blob<'a>>,
    block: usize,
    decoded: usize,
    docs: Vec<u32>,
    tfs: Vec<u32>,
    at: usize,
    wprobe: usize,
}

struct Dir {
    firsts: Rc<Vec<u32>>,
    lasts: Rc<Vec<u32>>,
    raw: Vec<f64>,
    offsets: Rc<Vec<u32>>,
    seeds: Vec<(u32, u32)>,
}

fn decode_dir(directory: &[u8], seeds: &[u8]) -> Dir {
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
    let mut kept = Vec::new();
    let mut at = 0;
    while at < seeds.len() {
        get_varint(seeds, &mut at);
        let tf = get_varint(seeds, &mut at);
        let len = get_varint(seeds, &mut at);
        kept.push((tf, len));
    }
    Dir { firsts: Rc::new(firsts), lasts: Rc::new(lasts), raw, offsets: Rc::new(offsets), seeds: kept }
}

struct Lengths {
    short: Vec<u8>,
    long: HashMap<u32, u32>,
}

impl Lengths {
    #[inline]
    fn of(&self, doc: u32) -> u32 {
        let short = u32::from(self.short.get(doc as usize).copied().unwrap_or(0));
        if short == 255 { self.long[&doc] } else { short }
    }
}

impl Cursor<'_> {
    fn load(&mut self, buffer: &mut Vec<u8>, blocks: &mut usize) {
        if self.decoded == self.block {
            return;
        }
        let start = self.offsets[self.block] as usize;
        let end = self.offsets.get(self.block + 1).map_or(self.size, |end| *end as usize);
        buffer.clear();
        buffer.resize(end - start + 8, 0);
        self.blob.as_ref().unwrap().read_at_exact(&mut buffer[..end - start], start).unwrap();
        crate::fast2::unpack_words(buffer, self.firsts[self.block], &mut self.docs, &mut self.tfs);
        *blocks += 1;
        self.decoded = self.block;
    }

    fn doc(&mut self, buffer: &mut Vec<u8>, blocks: &mut usize) -> u32 {
        loop {
            if self.block >= self.firsts.len() {
                return END;
            }
            self.load(buffer, blocks);
            if self.at < self.docs.len() {
                return self.docs[self.at];
            }
            self.block += 1;
            self.at = 0;
        }
    }

    fn seek(&mut self, doc: u32, buffer: &mut Vec<u8>, blocks: &mut usize) -> u32 {
        while self.block < self.lasts.len() && self.lasts[self.block] < doc {
            self.block += 1;
            self.at = 0;
        }
        if self.block >= self.lasts.len() {
            return END;
        }
        self.load(buffer, blocks);
        while self.at < self.docs.len() && self.docs[self.at] < doc {
            self.at += 1;
        }
        self.doc(buffer, blocks)
    }

    fn bound_over(&mut self, start: u32, end: u32) -> f64 {
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

    fn next_from(&mut self, end: u32) -> u32 {
        let mut at = self.wprobe.max(self.block).min(self.lasts.len());
        while at < self.lasts.len() && self.lasts[at] < end {
            at += 1;
        }
        self.wprobe = at;
        self.firsts.get(at).map_or(END, |first| (*first).max(end))
    }

    #[inline]
    fn score(&self, lengths: &[Lengths]) -> f64 {
        let doc = self.docs[self.at];
        self.scale * saturated(self.tfs[self.at], lengths[self.field].of(doc), self.average)
    }
}

/// The native layout's reader: fast2.rs's executor with every switch of
/// `fast4-cache`, over lists of either field; FIELDS_CACHE=0 reads every
/// directory again a query.
pub struct Native {
    connection: Connection,
    documents: u64,
    average: [f64; 4],
    lengths: Vec<Lengths>,
    cache: RefCell<HashMap<i64, Rc<Dir>>>,
    keep: bool,
    pub blocks: usize,
}

impl Native {
    pub fn open(path: &Path) -> Native {
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
        let connection = open(path);
        connection
            .execute_batch(&format!(
                "pragma cache_size = -{}; pragma mmap_size = {};",
                var("FAST_CACHE_KIB", "1024"),
                var("FAST_MMAP", "0")
            ))
            .unwrap();
        connection.pragma_update(None, "query_only", true).unwrap();
        let meta: HashMap<String, f64> = connection
            .prepare("select name, value from meta")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut average = [1.0; 4];
        for field in [B, BS] {
            average[field] = meta[&format!("average{field}")];
        }
        let mut lengths: Vec<Lengths> = (0..4).map(|_| Lengths { short: Vec::new(), long: HashMap::new() }).collect();
        {
            let mut rows = connection.prepare("select id, bytes from lengths order by id").unwrap();
            let mut read = rows.query([]).unwrap();
            while let Some(row) = read.next().unwrap() {
                let id: i64 = row.get(0).unwrap();
                let bytes: Vec<u8> = row.get(1).unwrap();
                let (field, chunk) = ((id / 100_000) as usize, (id % 100_000) as usize);
                lengths[field].short.resize(chunk * NORMS as usize, 0);
                lengths[field].short.extend_from_slice(&bytes);
            }
        }
        for (field, doc, len) in connection
            .prepare("select field, doc, len from long")
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)))
            .unwrap()
            .map(Result::unwrap)
        {
            lengths[field as usize].long.insert(doc as u32, len as u32);
        }
        Native {
            connection,
            documents: meta["documents"] as u64,
            average,
            lengths,
            cache: RefCell::new(HashMap::new()),
            keep: var("FIELDS_CACHE", "1") == "1",
            blocks: 0,
        }
    }

    fn expand(&self, prefix: &str) -> Vec<String> {
        self.connection
            .prepare_cached(
                "select word from words where word >= ?1 and word < ?2 and b > 0 order by b desc, word limit ?3",
            )
            .unwrap()
            .query_map(params![prefix, past(prefix), EXPAND as i64], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn cursors(&self, asked: &Asked, k: usize) -> (Vec<Cursor<'_>>, f64) {
        let mut words = asked.words.clone();
        if let Some(prefix) = &asked.prefix {
            words.extend(self.expand(prefix).into_iter().map(|word| (word, None)));
        }
        let mut find = self.connection.prepare_cached("select id, b, bs from words where word = ?1").unwrap();
        let mut found: HashMap<String, Option<(i64, [i64; 4])>> = HashMap::new();
        let mut cursors = Vec::new();
        let mut seed = 0.0f64;
        for (field, word) in forms(&words) {
            let entry = found.entry(word.clone()).or_insert_with(|| {
                find.query_row([&word], |row| {
                    let mut held = [0i64; 4];
                    held[B] = row.get(1)?;
                    held[BS] = row.get(2)?;
                    Ok((row.get::<_, i64>(0)?, held))
                })
                .optional()
                .unwrap()
            });
            let Some((id, held)) = *entry else { continue };
            if held[field] == 0 {
                continue;
            }
            let row = id * 4 + field as i64;
            let scale = weight(field) * idf(self.documents, held[field] as u64);
            let average = self.average[field];
            if held[field] as usize <= INLINE {
                // A short list, whole: its bound and its seed from its postings.
                let payload: Vec<u8> = self
                    .connection
                    .prepare_cached("select payload from lists where id = ?1")
                    .unwrap()
                    .query_row([row], |row| row.get(0))
                    .unwrap();
                let (docs, tfs) = crate::fast2::unpack_varints(&payload);
                let lengths = &self.lengths[field];
                let mut scores: Vec<f64> = docs
                    .iter()
                    .zip(&tfs)
                    .map(|(doc, tf)| scale * saturated(*tf, lengths.of(*doc), average))
                    .collect();
                let best = scores.iter().copied().fold(0.0, f64::max);
                if scores.len() >= k {
                    scores.sort_unstable_by(|a, b| b.total_cmp(a));
                    seed = seed.max(scores[k - 1]);
                }
                cursors.push(Cursor {
                    field,
                    scale,
                    average,
                    firsts: Rc::new(vec![docs[0]]),
                    lasts: Rc::new(vec![docs[docs.len() - 1]]),
                    bounds: vec![best],
                    offsets: Rc::new(vec![0]),
                    size: 0,
                    blob: None,
                    block: 0,
                    decoded: 0,
                    docs,
                    tfs,
                    at: 0,
                    wprobe: 0,
                });
                continue;
            }
            let cached = self.cache.borrow().get(&row).cloned();
            let dir = match cached {
                Some(dir) => dir,
                None => {
                    let (directory, seeds): (Vec<u8>, Vec<u8>) = self
                        .connection
                        .prepare_cached("select directory, seeds from dirs where id = ?1")
                        .unwrap()
                        .query_row([row], |row| Ok((row.get(0)?, row.get(1)?)))
                        .unwrap();
                    let dir = Rc::new(decode_dir(&directory, &seeds));
                    if self.keep {
                        self.cache.borrow_mut().insert(row, Rc::clone(&dir));
                    }
                    dir
                }
            };
            let mut scores: Vec<f64> = dir.seeds.iter().map(|(tf, len)| scale * saturated(*tf, *len, average)).collect();
            if scores.len() >= k {
                scores.sort_unstable_by(|a, b| b.total_cmp(a));
                seed = seed.max(scores[k - 1]);
            }
            let blob = self.connection.blob_open("main", "lists", "payload", row, true).unwrap();
            let size = blob.len();
            let growth = 1.0 + 4.0 * f64::EPSILON;
            cursors.push(Cursor {
                field,
                scale,
                average,
                firsts: Rc::clone(&dir.firsts),
                lasts: Rc::clone(&dir.lasts),
                bounds: dir.raw.iter().map(|raw| scale * raw * growth).collect(),
                offsets: Rc::clone(&dir.offsets),
                size,
                blob: Some(blob),
                block: 0,
                decoded: usize::MAX,
                docs: Vec::with_capacity(128),
                tfs: Vec::with_capacity(128),
                at: 0,
                wprobe: 0,
            });
        }
        (cursors, seed)
    }

    pub fn top(&mut self, asked: &Asked, k: usize) -> Vec<(f64, u32)> {
        let (mut cursors, seed) = self.cursors(asked, k);
        let mut blocks = 0;
        let found = windowed(&mut cursors, &self.lengths, k, seed, &mut blocks);
        drop(cursors);
        self.blocks = blocks;
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

/// fast2.rs's `windowed` with `walk`, `direct` and `windows` on.
fn windowed(lists: &mut [Cursor], lengths: &[Lengths], k: usize, seed: f64, blocks: &mut usize) -> Vec<(f64, u32)> {
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
    let mut start = lists.iter_mut().map(|list| list.next_from(0)).min().unwrap_or(END);
    while start != END {
        let end = start.saturating_add(WINDOW);
        let bounds: Vec<f64> = lists.iter_mut().map(|list| list.bound_over(start, end)).collect();
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
                let mut doc = list.seek(start, &mut buffer, blocks);
                while doc < end {
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
                        list.at += 1;
                    }
                    doc = list.doc(&mut buffer, blocks);
                }
            }
            touched.sort_unstable();
            let window_rest: f64 = order[..passive].iter().map(|at| bounds[*at]).sum();
            probes.clear();
            for &at in &order[..passive] {
                let list = &lists[at];
                let from = list.block.min(list.lasts.len());
                probes.push(from + list.lasts[from..].partition_point(|last| *last < start));
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
                let mut rest: f64 = passive_bounds.iter().sum();
                for (j, &at) in order[..passive].iter().enumerate().rev() {
                    if full && score + rest <= theta {
                        break;
                    }
                    let bound = passive_bounds[j];
                    rest -= bound;
                    if bound > 0.0 && lists[at].seek(doc, &mut buffer, blocks) == doc {
                        score += lists[at].score(lengths);
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
        start = lists.iter_mut().map(|list| list.next_from(end)).min().unwrap_or(END);
    }
    let mut found: Vec<(f64, u32)> = best.into_iter().map(|Reverse(Scored(score, Reverse(doc)))| (score, doc)).collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    found
}

// --- Tantivy over the two fields -----------------------------------------------

pub fn build_tantivy(stems: &Path, surface: &Path, index_dir: &Path) -> Value {
    use tantivy::schema::{FAST, IndexRecordOption, Schema, TextFieldIndexing, TextOptions};
    use tantivy::{Index, IndexWriter, TantivyDocument};
    let started = Instant::now();
    std::fs::create_dir_all(index_dir).unwrap();
    let mut builder = Schema::builder();
    let text = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("ws")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let b = builder.add_text_field("b", text.clone());
    let bs = builder.add_text_field("bs", text);
    let id = builder.add_u64_field("id", FAST);
    let index = Index::create_in_dir(index_dir, builder.build()).unwrap();
    crate::tant1::register(&index);
    let mut writer: IndexWriter<TantivyDocument> = index.writer_with_num_threads(1, 1024 << 20).unwrap();
    let words = BufReader::with_capacity(1 << 22, File::open(surface.join("tokens.tsv")).unwrap());
    let stemmed = BufReader::with_capacity(1 << 22, File::open(stems.join("tokens.tsv")).unwrap());
    let mut rows = 0u64;
    for (a, s) in words.lines().zip(stemmed.lines()) {
        let (a, s) = (a.unwrap(), s.unwrap());
        let (pid, written) = a.split_once('\t').unwrap();
        let (pid_s, stem_tokens) = s.split_once('\t').unwrap();
        assert_eq!(pid, pid_s);
        let mut document = TantivyDocument::default();
        document.add_text(b, written);
        document.add_text(bs, stem_tokens);
        document.add_u64(id, pid.parse::<u64>().unwrap() + 1);
        writer.add_document(document).unwrap();
        rows += 1;
    }
    writer.commit().unwrap();
    let segments: Vec<tantivy::index::SegmentId> = {
        let reader = index.reader().unwrap();
        let mut by_first: Vec<(u64, tantivy::index::SegmentId)> = reader
            .searcher()
            .segment_readers()
            .iter()
            .map(|segment| {
                let ids: tantivy::columnar::Column<u64> = segment.fast_fields().u64("id").unwrap();
                (ids.min_value(), segment.segment_id())
            })
            .collect();
        by_first.sort();
        by_first.into_iter().map(|(_, id)| id).collect()
    };
    if segments.len() > 1 {
        writer.merge(&segments).wait().unwrap();
    }
    writer.wait_merging_threads().unwrap();
    json!({"step": "fields-build", "layout": "tantivy", "documents": rows, "seconds": started.elapsed().as_secs_f64(),
        "segments_merged": segments.len(), "files": crate::sizes::files(index_dir)})
}

pub struct Tant {
    _reader: tantivy::IndexReader,
    searcher: tantivy::Searcher,
    fields: [tantivy::schema::Field; 4],
    ids: tantivy::columnar::Column<u64>,
}

impl Tant {
    pub fn open(index_dir: &Path) -> Tant {
        let index = tantivy::Index::open_in_dir(index_dir).unwrap();
        crate::tant1::register(&index);
        let schema = index.schema();
        let (b, bs) = (schema.get_field("b").unwrap(), schema.get_field("bs").unwrap());
        let reader: tantivy::IndexReader =
            index.reader_builder().reload_policy(tantivy::ReloadPolicy::Manual).try_into().unwrap();
        let searcher = reader.searcher();
        assert_eq!(searcher.segment_readers().len(), 1, "one segment, merged in pid order");
        let ids = searcher.segment_readers()[0].fast_fields().u64("id").unwrap();
        Tant { _reader: reader, searcher, fields: [b, bs, b, bs], ids }
    }

    /// The 32 words of `b` that begin with `prefix` and hold the most
    /// documents, ties by word, from Tantivy's own term dictionary.
    fn expand(&self, prefix: &str) -> Vec<String> {
        let inverted = self.searcher.segment_readers()[0].inverted_index(self.fields[B]).unwrap();
        let mut stream = inverted.terms().range().ge(prefix.as_bytes()).into_stream().unwrap();
        let mut found: Vec<(u32, String)> = Vec::new();
        while stream.advance() {
            let key = stream.key();
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            found.push((stream.value().doc_freq, String::from_utf8(key.to_vec()).unwrap()));
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        found.truncate(EXPAND);
        found.into_iter().map(|(_, word)| word).collect()
    }

    pub fn top(&mut self, asked: &Asked, k: usize) -> Vec<(f64, u32)> {
        use tantivy::collector::TopDocs;
        use tantivy::query::{BooleanQuery, BoostQuery, Query, TermQuery};
        use tantivy::schema::IndexRecordOption;
        let mut words = asked.words.clone();
        if let Some(prefix) = &asked.prefix {
            words.extend(self.expand(prefix).into_iter().map(|word| (word, None)));
        }
        let clauses: Vec<Box<dyn Query>> = forms(&words)
            .into_iter()
            .map(|(field, word)| {
                let field_handle = self.fields[field];
                let term = TermQuery::new(
                    tantivy::Term::from_field_text(field_handle, &word),
                    IndexRecordOption::WithFreqs,
                );
                if std::env::var("FIELDS_EQUAL").as_deref() == Ok("1") {
                    Box::new(term) as Box<dyn Query>
                } else {
                    Box::new(BoostQuery::new(Box::new(term), weight(field) as f32)) as Box<dyn Query>
                }
            })
            .collect();
        if clauses.is_empty() {
            return Vec::new();
        }
        let query = BooleanQuery::union(clauses);
        self.searcher
            .search(&query, &TopDocs::with_limit(k).order_by_score())
            .unwrap()
            .into_iter()
            .map(|(score, address)| (f64::from(score), self.ids.first(address.doc_id).unwrap() as u32))
            .collect()
    }
}

// --- answering and timing ---------------------------------------------------------

enum Engine {
    Lab(Lab),
    Native(Native),
    Tant(Tant),
}

impl Engine {
    fn open(name: &str, index: &Path) -> Engine {
        match name {
            "lab" => Engine::Lab(Lab::open(index, false)),
            "lab-exhaustive" => Engine::Lab(Lab::open(index, true)),
            "native" => Engine::Native(Native::open(index)),
            "tantivy" => Engine::Tant(Tant::open(index)),
            other => panic!("no engine {other}"),
        }
    }

    fn top(&mut self, asked: &Asked, k: usize) -> Vec<(f64, u32)> {
        match self {
            Engine::Lab(lab) => lab.top(asked, k),
            Engine::Native(native) => native.top(asked, k),
            Engine::Tant(tant) => tant.top(asked, k),
        }
    }
}

pub fn answer(name: &str, index: &Path, queries: &Path, results: &Path) -> Value {
    let started = Instant::now();
    let mut engine = Engine::open(name, index);
    let asked = read_asked(queries);
    let mut out = std::io::BufWriter::new(File::create(results).unwrap());
    for query in &asked {
        let found = engine.top(query, 10);
        let ten: Vec<_> = found.iter().map(|(score, doc)| json!([doc - 1, score])).collect();
        writeln!(out, "{}", json!({"qid": query.qid, "ten": ten})).unwrap();
    }
    out.flush().unwrap();
    json!({"step": "fields-answer", "engine": name, "queries": asked.len(), "seconds": started.elapsed().as_secs_f64(),
        "note": "a run for answers, not a measurement"})
}

pub fn time(name: &str, index: &Path, queries: &Path, results: &Path, mode: &str) -> Value {
    let opened = Instant::now();
    let mut engine = Engine::open(name, index);
    let open_ms = opened.elapsed().as_secs_f64() * 1e3;
    let asked = read_asked(queries);
    if mode == "warm" {
        for query in &asked {
            std::hint::black_box(engine.top(query, 10));
        }
    } else {
        assert_eq!(mode, "cold", "a pass is warm or cold");
    }
    let mut out = std::io::BufWriter::new(File::create(results).unwrap());
    for query in &asked {
        let took = Instant::now();
        let found = engine.top(query, 10);
        let nanos = took.elapsed().as_nanos();
        let ids: Vec<u32> = found.iter().map(|(_, doc)| doc - 1).collect();
        writeln!(out, "{}", json!({"qid": query.qid, "ns": nanos as u64, "ids": ids})).unwrap();
    }
    out.flush().unwrap();
    json!({"step": "fields-time", "engine": name, "mode": mode, "queries": asked.len(), "open_ms": open_ms,
        "memory": crate::memory()})
}
