//! A typo's candidates: the index's terms in an FST, and a Levenshtein
//! automaton that counts two letters swapped as one edit, walked over it.
//! Meilisearch's rule says how far a word may be: none under five letters,
//! one up to eight, two from nine.

use std::time::Instant;

use fst::{Automaton, IntoStreamer, Set, Streamer};
use levenshtein_automata::{DFA, Distance, LevenshteinAutomatonBuilder, SINK_STATE};
use rusqlite::Connection;

pub struct Typos {
    terms: Set<Vec<u8>>,
    one: LevenshteinAutomatonBuilder,
    two: LevenshteinAutomatonBuilder,
    pub built: Built,
}

pub struct Built {
    pub terms: u64,
    pub bytes: usize,
    pub millis: f64,
}

/// The most candidates a word takes.
const MOST: usize = 8;

impl Typos {
    /// Builds the FST from FTS5's vocabulary, which comes in term order.
    pub fn build(connection: &Connection) -> Typos {
        let started = Instant::now();
        let mut builder = fst::SetBuilder::memory();
        let mut statement = connection.prepare("select term from temp.vocab").unwrap();
        let mut rows = statement.query([]).unwrap();
        let mut terms = 0;
        while let Some(row) = rows.next().unwrap() {
            builder
                .insert(row.get_ref(0).unwrap().as_str().unwrap())
                .unwrap();
            terms += 1;
        }
        let set = Set::new(builder.into_inner().unwrap()).unwrap();
        let built = Built {
            terms,
            bytes: set.as_fst().as_bytes().len(),
            millis: started.elapsed().as_secs_f64() * 1e3,
        };
        Typos {
            terms: set,
            one: LevenshteinAutomatonBuilder::new(1, true),
            two: LevenshteinAutomatonBuilder::new(2, true),
            built,
        }
    }

    /// The index's terms a word may have meant, nearest first.
    pub fn candidates(&self, word: &str) -> Vec<String> {
        let letters = word.chars().count();
        let builder = match letters {
            0..5 => return Vec::new(),
            5..9 => &self.one,
            _ => &self.two,
        };
        let dfa = builder.build_dfa(word);
        let mut found = Vec::new();
        let mut stream = self.terms.search(Within(&dfa)).into_stream();
        while let Some(term) = stream.next() {
            let term = String::from_utf8_lossy(term).into_owned();
            if term != word {
                let distance = distance(&dfa, &term);
                found.push((distance, term));
            }
        }
        found.sort();
        found.into_iter().take(MOST).map(|(_, term)| term).collect()
    }
}

fn distance(dfa: &DFA, term: &str) -> u8 {
    let state = term.bytes().fold(dfa.initial_state(), |state, byte| {
        dfa.transition(state, byte)
    });
    match dfa.distance(state) {
        Distance::Exact(distance) | Distance::AtLeast(distance) => distance,
    }
}

/// The automaton as fst walks one.
struct Within<'a>(&'a DFA);

impl Automaton for Within<'_> {
    type State = u32;

    fn start(&self) -> u32 {
        self.0.initial_state()
    }

    fn is_match(&self, state: &u32) -> bool {
        matches!(self.0.distance(*state), Distance::Exact(_))
    }

    fn can_match(&self, state: &u32) -> bool {
        *state != SINK_STATE
    }

    fn accept(&self, state: &u32, byte: u8) -> u32 {
        self.0.transition(*state, byte)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swap_is_one_edit() {
        let dfa = LevenshteinAutomatonBuilder::new(1, true).build_dfa("репликация");
        assert_eq!(distance(&dfa, "реплиакция"), 1);
    }
}
