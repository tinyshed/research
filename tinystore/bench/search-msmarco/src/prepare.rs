//! MS MARCO's passages as one token stream, written once for every engine.
//!
//! Each passage goes through the lab's analyzer, `auto`, and each token
//! becomes its stem where it has one and its surface where it has not:
//! English's Snowball stems, no stop words, one field. Every engine indexes
//! exactly these tokens.
//!
//! ```text
//! <out>/tokens.tsv          pid \t tokens joined by spaces     FTS5, Tantivy and Lucene read it
//! <out>/msmarco.docs        [N], then a term's documents       PISA's binary collection, terms in byte order
//! <out>/msmarco.freqs       a term's frequencies, aligned
//! <out>/msmarco.sizes       [a document's tokens] × N
//! <out>/msmarco.terms       a term a line, by its number       PISA's lexicon and everyone's term numbers
//! <out>/msmarco.documents   a pid a line                       document n is pid n
//! <out>/collection.json     the counts
//! ```

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Instant;

use serde_json::json;

use crate::analyze::{Analysis, Analyzer};

/// Passages tokenized at once, split among the threads.
const BATCH: usize = 40_000;

/// A text as the one field every engine indexes; with MSMARCO_TOKENS=surface,
/// as the words written, for search-fields' second field.
pub fn terms(analyzer: &Analyzer, text: &str) -> Vec<String> {
    static SURFACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let surface = *SURFACE.get_or_init(|| std::env::var("MSMARCO_TOKENS").as_deref() == Ok("surface"));
    analyzer
        .tokens(text)
        .into_iter()
        .map(|token| if surface { token.surface } else { token.stem.unwrap_or(token.surface) })
        .collect()
}

pub fn put_u32(out: &mut impl Write, value: u32) {
    out.write_all(&value.to_le_bytes()).unwrap();
}

pub fn get_u32(input: &mut impl Read) -> Option<u32> {
    let mut bytes = [0u8; 4];
    match input.read_exact(&mut bytes) {
        Ok(()) => Some(u32::from_le_bytes(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => None,
        Err(error) => panic!("{error}"),
    }
}

pub fn prepare(corpus: &Path, out: &Path, threads: usize, limit: usize) {
    let started = Instant::now();
    std::fs::create_dir_all(out).unwrap();
    let input = BufReader::with_capacity(1 << 22, File::open(corpus.join("collection.tsv")).unwrap());
    let mut tokens_out = BufWriter::with_capacity(1 << 22, File::create(out.join("tokens.tsv")).unwrap());
    let mut forward = BufWriter::with_capacity(1 << 22, File::create(out.join("forward.bin")).unwrap());
    let mut dictionary: HashMap<String, u32> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut df: Vec<u32> = Vec::new();
    let mut sizes: Vec<u32> = Vec::new();
    let (mut tokens, mut postings, mut longest_token) = (0u64, 0u64, 0usize);
    let (mut checked, mut unstable) = (0u64, 0u64);
    let surface = Analyzer::new(Analysis::named("surface"));
    let mut lines = input.lines();
    let mut done = false;
    while !done {
        let mut batch: Vec<(u32, String)> = Vec::with_capacity(BATCH);
        while batch.len() < BATCH && sizes.len() + batch.len() < limit {
            let Some(line) = lines.next() else {
                done = true;
                break;
            };
            let line = line.unwrap();
            let (pid, text) = line.split_once('\t').expect("a line is pid \\t text");
            batch.push((pid.parse().unwrap(), text.to_owned()));
        }
        if sizes.len() + batch.len() >= limit {
            done = true;
        }
        let analyzed: Vec<Vec<String>> = std::thread::scope(|scope| {
            let chunk = batch.len().div_ceil(threads).max(1);
            let handles: Vec<_> = batch
                .chunks(chunk)
                .map(|slice| {
                    scope.spawn(move || {
                        let analyzer = Analyzer::new(Analysis::named("auto"));
                        slice.iter().map(|(_, text)| terms(&analyzer, text)).collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|handle| handle.join().unwrap()).collect()
        });
        for ((pid, _), stream) in batch.iter().zip(&analyzed) {
            assert_eq!(*pid as usize, sizes.len(), "passages are numbered from 0 in order");
            let joined = stream.join(" ");
            // Every 1,000th passage is analyzed again as the engines that
            // re-tokenize it will: its tokens must come back unchanged.
            if pid % 1000 == 0 {
                checked += 1;
                let again: Vec<String> = surface.tokens(&joined).into_iter().map(|token| token.surface).collect();
                unstable += u64::from(again != *stream);
            }
            tokens_out.write_all(format!("{pid}\t{joined}\n").as_bytes()).unwrap();
            let mut counts: HashMap<u32, u32> = HashMap::new();
            for term in stream {
                longest_token = longest_token.max(term.len());
                let next = names.len() as u32;
                let id = *dictionary.entry(term.clone()).or_insert_with(|| {
                    names.push(term.clone());
                    df.push(0);
                    next
                });
                *counts.entry(id).or_default() += 1;
            }
            let mut counts: Vec<(u32, u32)> = counts.into_iter().collect();
            counts.sort_unstable();
            put_u32(&mut forward, counts.len() as u32);
            for (id, tf) in &counts {
                put_u32(&mut forward, *id);
                put_u32(&mut forward, *tf);
                df[*id as usize] += 1;
            }
            postings += counts.len() as u64;
            tokens += stream.len() as u64;
            sizes.push(stream.len() as u32);
        }
        if sizes.len() % 400_000 < BATCH {
            eprintln!("{} passages, {} terms, {:.0} s", sizes.len(), names.len(), started.elapsed().as_secs_f64());
        }
    }
    tokens_out.flush().unwrap();
    forward.flush().unwrap();
    drop(forward);
    let analyzed_seconds = started.elapsed().as_secs_f64();

    // Terms numbered in byte order, as the compact format's keys are sorted.
    let mut order: Vec<u32> = (0..names.len() as u32).collect();
    order.sort_unstable_by(|a, b| names[*a as usize].as_bytes().cmp(names[*b as usize].as_bytes()));
    let mut renumber = vec![0u32; names.len()];
    for (final_id, provisional) in order.iter().enumerate() {
        renumber[*provisional as usize] = final_id as u32;
    }
    {
        let mut lexicon = BufWriter::new(File::create(out.join("msmarco.terms")).unwrap());
        for provisional in &order {
            lexicon.write_all(names[*provisional as usize].as_bytes()).unwrap();
            lexicon.write_all(b"\n").unwrap();
        }
        let mut documents = BufWriter::new(File::create(out.join("msmarco.documents")).unwrap());
        for pid in 0..sizes.len() {
            writeln!(documents, "{pid}").unwrap();
        }
        let mut lengths = BufWriter::new(File::create(out.join("msmarco.sizes")).unwrap());
        put_u32(&mut lengths, sizes.len() as u32);
        for size in &sizes {
            put_u32(&mut lengths, *size);
        }
    }
    let final_df: Vec<u32> = order.iter().map(|provisional| df[*provisional as usize]).collect();
    drop(names);
    drop(dictionary);
    invert(out, &final_df, &renumber, sizes.len());
    std::fs::remove_file(out.join("forward.bin")).unwrap();
    let n = sizes.len() as u64;
    let line = json!({"step": "prepare", "documents": n, "tokens": tokens, "terms": final_df.len(),
        "postings": postings, "average_length": tokens as f64 / n as f64, "longest_token_bytes": longest_token,
        "reanalyzed": checked, "reanalyzed_changed": unstable, "threads": threads,
        "analyzed_seconds": analyzed_seconds, "seconds": started.elapsed().as_secs_f64(),
        "memory": crate::memory()});
    std::fs::write(out.join("collection.json"), format!("{line}\n")).unwrap();
    println!("{line}");
}

/// The forward file turned term-major, in PISA's binary collection format:
/// each sequence a u32 length and then its u32 values, little-endian.
fn invert(out: &Path, df: &[u32], renumber: &[u32], documents: usize) {
    let mut start = Vec::with_capacity(df.len() + 1);
    let mut total = 0u64;
    for count in df {
        start.push(total);
        total += u64::from(*count);
    }
    start.push(total);
    let mut docs = vec![0u32; total as usize];
    let mut freqs = vec![0u32; total as usize];
    let mut cursor: Vec<u64> = start[..df.len()].to_vec();
    let mut forward = BufReader::with_capacity(1 << 22, File::open(out.join("forward.bin")).unwrap());
    for doc in 0..documents as u32 {
        let count = get_u32(&mut forward).unwrap();
        for _ in 0..count {
            let term = renumber[get_u32(&mut forward).unwrap() as usize] as usize;
            let tf = get_u32(&mut forward).unwrap();
            let at = cursor[term] as usize;
            docs[at] = doc;
            freqs[at] = tf;
            cursor[term] += 1;
        }
    }
    let mut docs_out = BufWriter::with_capacity(1 << 22, File::create(out.join("msmarco.docs")).unwrap());
    let mut freqs_out = BufWriter::with_capacity(1 << 22, File::create(out.join("msmarco.freqs")).unwrap());
    put_u32(&mut docs_out, 1);
    put_u32(&mut docs_out, documents as u32);
    for (term, count) in df.iter().enumerate() {
        let (from, to) = (start[term] as usize, start[term + 1] as usize);
        put_u32(&mut docs_out, *count);
        put_u32(&mut freqs_out, *count);
        for at in from..to {
            put_u32(&mut docs_out, docs[at]);
            put_u32(&mut freqs_out, freqs[at]);
        }
    }
    docs_out.flush().unwrap();
    freqs_out.flush().unwrap();
}
