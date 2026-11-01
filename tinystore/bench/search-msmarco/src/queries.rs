//! The query sets in the same tokens as the passages.
//!
//! Each query's tokens are those of `prepare::terms`, each distinct term
//! once in the order first met, and a term no passage holds left out, since
//! it adds nothing in any engine. Every engine is asked exactly these terms.
//!
//! ```text
//! <out>/q/<set>.tsv    qid \t terms joined by spaces     compact, FTS5, Tantivy, Lucene, the exact scorer
//! <out>/q/<set>.pisa   qid:term numbers                  PISA, which then needs no tokenizer of its own
//! ```
//!
//! The sets: `dev`, MS MARCO's 6,980 small dev queries; `dl19` and `dl20`,
//! the TREC deep-learning queries NIST judged, 43 and 54; `sample`, the
//! 1,000 dev queries of lowest SHA-256 of their qid with both TREC sets.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use flate2::read::GzDecoder;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::analyze::{Analysis, Analyzer};
use crate::collection::term_numbers;
use crate::prepare::terms;

pub const SAMPLE_DEV: usize = 1000;

fn read_tsv(reader: impl BufRead) -> Vec<(String, String)> {
    reader
        .lines()
        .map(Result::unwrap)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (qid, text) = line.split_once('\t').unwrap();
            (qid.to_owned(), text.to_owned())
        })
        .collect()
}

/// The qids a TREC qrels file judges.
fn judged(path: &Path) -> HashSet<String> {
    BufReader::new(File::open(path).unwrap())
        .lines()
        .map(Result::unwrap)
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

pub fn queries(corpus: &Path, out: &Path) {
    let analyzer = Analyzer::new(Analysis::named("auto"));
    let numbers = term_numbers(out);
    let dir = out.join("q");
    std::fs::create_dir_all(&dir).unwrap();
    let dev = read_tsv(BufReader::new(File::open(corpus.join("queries.dev.small.tsv")).unwrap()));
    let gz = |name: &str| {
        read_tsv(BufReader::new(GzDecoder::new(File::open(corpus.join(name)).unwrap())))
    };
    let dl19_judged = judged(&corpus.join("2019qrels-pass.txt"));
    let dl20_judged = judged(&corpus.join("2020qrels-pass.txt"));
    let dl19: Vec<_> = gz("msmarco-test2019-queries.tsv.gz")
        .into_iter()
        .filter(|(qid, _)| dl19_judged.contains(qid))
        .collect();
    let dl20: Vec<_> = gz("msmarco-test2020-queries.tsv.gz")
        .into_iter()
        .filter(|(qid, _)| dl20_judged.contains(qid))
        .collect();
    let mut by_hash: Vec<(String, &(String, String))> = dev
        .iter()
        .map(|query| {
            let digest = Sha256::digest(query.0.as_bytes());
            (crate::corpus::hex(&digest), query)
        })
        .collect();
    by_hash.sort();
    let mut sample: Vec<(String, String)> =
        by_hash[..SAMPLE_DEV].iter().map(|(_, query)| (*query).clone()).collect();
    sample.extend(dl19.iter().cloned());
    sample.extend(dl20.iter().cloned());
    let mut seen = HashSet::new();
    for (qid, _) in dev.iter().chain(&dl19).chain(&dl20) {
        assert!(seen.insert(qid.clone()), "qid {qid} is in two sets");
    }
    let mut counts = serde_json::Map::new();
    for (name, set) in [("dev", &dev), ("dl19", &dl19), ("dl20", &dl20), ("sample", &sample)] {
        let mut tsv = File::create(dir.join(format!("{name}.tsv"))).unwrap();
        let mut pisa = File::create(dir.join(format!("{name}.pisa"))).unwrap();
        let (mut words, mut unknown, mut empty) = (0usize, 0usize, 0usize);
        for (qid, text) in set.iter() {
            let mut met = HashSet::new();
            let mut kept: Vec<(String, u32)> = Vec::new();
            for term in terms(&analyzer, text) {
                if !met.insert(term.clone()) {
                    continue;
                }
                match numbers.get(&term) {
                    Some(number) => kept.push((term, *number)),
                    None => unknown += 1,
                }
            }
            words += kept.len();
            empty += usize::from(kept.is_empty());
            let joined: Vec<&str> = kept.iter().map(|(term, _)| term.as_str()).collect();
            writeln!(tsv, "{qid}\t{}", joined.join(" ")).unwrap();
            let ids: Vec<String> = kept.iter().map(|(_, number)| number.to_string()).collect();
            writeln!(pisa, "{qid}:{}", ids.join(" ")).unwrap();
        }
        counts.insert(
            name.to_owned(),
            json!({"queries": set.len(), "terms": words, "unknown_terms": unknown, "empty": empty,
                   "mean_terms": words as f64 / set.len() as f64}),
        );
    }
    let line = json!({"step": "queries", "sets": counts});
    std::fs::write(dir.join("queries.json"), format!("{line}\n")).unwrap();
    println!("{line}");
}

/// A query file: each line's qid and terms.
pub fn read(path: &Path) -> Vec<(String, Vec<String>)> {
    BufReader::new(File::open(path).unwrap())
        .lines()
        .map(Result::unwrap)
        .map(|line| {
            let (qid, terms) = line.split_once('\t').unwrap();
            (
                qid.to_owned(),
                terms.split(' ').filter(|term| !term.is_empty()).map(str::to_owned).collect(),
            )
        })
        .collect()
}
