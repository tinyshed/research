//! What a query becomes before an index answers it: its words, each with the
//! forms it may take, and a plan of which words must match.
//!
//! | Plan              | What it asks                                                            |
//! |-------------------|-------------------------------------------------------------------------|
//! | `or`              | any word, ranked: the previous round's query                            |
//! | `and`             | every word                                                              |
//! | `adaptive`        | every word, then without the most frequent one, and so on, until ten    |
//! | `pruned-or`       | any word but those in more than `cut` of the documents                  |
//! | `adaptive-pruned` | `adaptive` over the words `pruned-or` keeps                             |
//!
//! A word in more than `cut` of the documents is the statistics' stop word:
//! no list a language, the index's own counts.

use std::collections::HashSet;

use crate::analyze::Analyzer;

/// A word of the query: as written, its stem where that differs, and the
/// words of the index a typo may have meant.
#[derive(Clone, Debug)]
pub struct Word {
    pub surface: String,
    pub stem: Option<String>,
    pub typos: Vec<String>,
}

/// What a plan needs of an index.
pub trait Backend {
    fn documents(&self) -> u64;
    /// The documents holding `term` in any field, as surface or stem.
    fn frequency(&mut self, term: &str) -> u64;
    /// The best `k` documents, holding every word or any.
    fn top(&mut self, words: &[Word], every: bool, k: usize) -> Vec<u32>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Or,
    And,
    Adaptive,
    PrunedOr,
    AdaptivePruned,
}

impl Plan {
    pub fn named(name: &str) -> Plan {
        match name {
            "or" => Plan::Or,
            "and" => Plan::And,
            "adaptive" => Plan::Adaptive,
            "pruned-or" => Plan::PrunedOr,
            "adaptive-pruned" => Plan::AdaptivePruned,
            other => panic!("no plan {other}"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Plan::Or => "or",
            Plan::And => "and",
            Plan::Adaptive => "adaptive",
            Plan::PrunedOr => "pruned-or",
            Plan::AdaptivePruned => "adaptive-pruned",
        }
    }
}

/// A query's words, each once.
pub fn words(analyzer: &Analyzer, text: &str) -> Vec<Word> {
    let mut seen = HashSet::new();
    analyzer
        .tokens(text)
        .into_iter()
        .filter(|token| seen.insert(token.surface.clone()))
        .map(|token| Word {
            surface: token.surface,
            stem: token.stem,
            typos: Vec::new(),
        })
        .collect()
}

/// What a query found, and the index queries its plan ran.
pub struct Answer {
    pub ids: Vec<u32>,
    pub queries: usize,
}

pub fn run(backend: &mut dyn Backend, plan: Plan, words: Vec<Word>, cut: f64, k: usize) -> Answer {
    let mut queries = 0;
    let ids = match plan {
        Plan::Or => {
            queries += 1;
            backend.top(&words, false, k)
        }
        Plan::And => {
            queries += 1;
            backend.top(&words, true, k)
        }
        Plan::PrunedOr => {
            let kept = pruned(backend, words, cut)
                .into_iter()
                .map(|(word, _)| word)
                .collect::<Vec<_>>();
            queries += 1;
            backend.top(&kept, false, k)
        }
        Plan::Adaptive => {
            let counted = counted(backend, words);
            adaptive(backend, counted, k, &mut queries)
        }
        Plan::AdaptivePruned => {
            let kept = pruned(backend, words, cut);
            adaptive(backend, kept, k, &mut queries)
        }
    };
    Answer { ids, queries }
}

/// Each word with the documents that hold it, rarest first; a word no
/// document holds is left out, since every word must match in a tier.
fn counted(backend: &mut dyn Backend, words: Vec<Word>) -> Vec<(Word, u64)> {
    let mut counted: Vec<(Word, u64)> = words
        .into_iter()
        .map(|word| {
            let mut count = backend.frequency(&word.surface);
            if let Some(stem) = &word.stem {
                count += backend.frequency(stem);
            }
            count += word
                .typos
                .iter()
                .map(|typo| backend.frequency(typo))
                .sum::<u64>();
            (word, count)
        })
        .filter(|(_, count)| *count > 0)
        .collect();
    counted.sort_by_key(|(_, count)| *count);
    counted
}

/// The words in at most `cut` of the documents; the rarest one when every
/// word is more frequent than that.
fn pruned(backend: &mut dyn Backend, words: Vec<Word>, cut: f64) -> Vec<(Word, u64)> {
    let most = (backend.documents() as f64 * cut) as u64;
    let mut counted = counted(backend, words);
    let rare = counted
        .iter()
        .take_while(|(_, count)| *count <= most)
        .count();
    counted.truncate(rare.max(1));
    counted
}

/// Every word, then the most frequent left out, and so on: the results of a
/// stricter tier rank before a looser one's.
fn adaptive(
    backend: &mut dyn Backend,
    counted: Vec<(Word, u64)>,
    k: usize,
    queries: &mut usize,
) -> Vec<u32> {
    let words: Vec<Word> = counted.into_iter().map(|(word, _)| word).collect();
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for kept in (1..=words.len()).rev() {
        *queries += 1;
        for id in backend.top(&words[..kept], true, k) {
            if seen.insert(id) {
                found.push(id);
            }
        }
        if found.len() >= k {
            break;
        }
    }
    found.truncate(k);
    found
}
