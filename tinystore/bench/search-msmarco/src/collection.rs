//! The prepared collection read back: PISA's binary collection mapped, a
//! term's documents and frequencies as slices, the documents' lengths, the
//! terms by number.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use memmap2::Mmap;

pub struct Collection {
    docs: Mmap,
    freqs: Mmap,
    /// Where each term's values begin, in u32s, in `docs`; `freqs` has no
    /// leading [N], so its values begin two u32s earlier.
    starts: Vec<usize>,
    pub sizes: Vec<u32>,
    pub documents: u32,
    pub tokens: u64,
}

fn words(map: &Mmap) -> &[u32] {
    // Little-endian u32s on a page-aligned map, on the x86-64 the rounds run on.
    unsafe { std::slice::from_raw_parts(map.as_ptr().cast::<u32>(), map.len() / 4) }
}

impl Collection {
    pub fn open(dir: &Path) -> Collection {
        let docs = unsafe { Mmap::map(&File::open(dir.join("msmarco.docs")).unwrap()).unwrap() };
        let freqs = unsafe { Mmap::map(&File::open(dir.join("msmarco.freqs")).unwrap()).unwrap() };
        let all = words(&docs);
        assert_eq!(all[0], 1, "the collection opens with a singleton sequence");
        let documents = all[1];
        let mut starts = Vec::new();
        let mut at = 2;
        while at < all.len() {
            starts.push(at + 1);
            at += 1 + all[at] as usize;
        }
        assert_eq!(at, all.len(), "the docs file ends with its last list");
        let bytes = std::fs::read(dir.join("msmarco.sizes")).unwrap();
        let sizes: Vec<u32> = bytes[4..]
            .chunks(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect();
        assert_eq!(sizes.len(), documents as usize);
        let tokens = sizes.iter().map(|size| u64::from(*size)).sum();
        Collection {
            docs,
            freqs,
            starts,
            sizes,
            documents,
            tokens,
        }
    }

    pub fn terms(&self) -> usize {
        self.starts.len()
    }

    pub fn df(&self, term: usize) -> u32 {
        words(&self.docs)[self.starts[term] - 1]
    }

    /// A term's documents and their frequencies.
    pub fn list(&self, term: usize) -> (&[u32], &[u32]) {
        let (from, count) = (self.starts[term], self.df(term) as usize);
        (
            &words(&self.docs)[from..from + count],
            &words(&self.freqs)[from - 2..from - 2 + count],
        )
    }
}

/// The terms in number order, as `prepare` wrote them.
pub fn read_terms(dir: &Path) -> Vec<String> {
    BufReader::new(File::open(dir.join("msmarco.terms")).unwrap())
        .lines()
        .map(Result::unwrap)
        .collect()
}

pub fn term_numbers(dir: &Path) -> HashMap<String, u32> {
    read_terms(dir)
        .into_iter()
        .enumerate()
        .map(|(at, term)| (term, at as u32))
        .collect()
}
