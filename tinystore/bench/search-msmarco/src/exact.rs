//! Exhaustive BM25 over the prepared collection, written apart from every
//! engine: each query's every posting scored, in f64, under each variant the
//! engines of this round compute, with k1 1.2 and b 0.75.
//!
//! | Variant | idf                                        | a document's length     | engines            |
//! |---------|--------------------------------------------|-------------------------|--------------------|
//! | `a`     | ln(1 + (N − df + ½)/(df + ½))              | exact                   | the compact blocks |
//! | `b`     | ln((N − df + ½)/(df + ½)), at least 10⁻⁶   | exact                   | FTS5, PISA         |
//! | `l`     | as `a`                                     | Lucene's byte, SmallFloat.intToByte4 | Lucene |
//! | `t`     | as `a`                                     | Tantivy's byte, FIELD_NORMS_TABLE    | Tantivy |
//!
//! A term adds idf × tf (k1 + 1) / (tf + k1 (1 − b + b · len / avg)), avg the
//! collection's tokens over its documents; Lucene leaves out the constant
//! k1 + 1, which ranks alike. Ties go to the lower pid. Each query reports its
//! ten, and how many documents share the tenth score, in and out of the ten.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};

use crate::collection::{Collection, term_numbers};

const K1: f64 = 1.2;
const B: f64 = 0.75;
const VARIANTS: [&str; 4] = ["a", "b", "l", "t"];

/// Tantivy 0.26.2's fieldnorm table, from src/fieldnorm/code.rs.
const TANTIVY_NORMS: [u32; 256] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 42, 44, 46, 48, 50, 52, 54, 56, 60, 64, 68, 72, 76, 80, 84,
    88, 96, 104, 112, 120, 128, 136, 144, 152, 168, 184, 200, 216, 232, 248, 264, 280, 312, 344, 376, 408, 440,
    472, 504, 536, 600, 664, 728, 792, 856, 920, 984, 1_048, 1_176, 1_304, 1_432, 1_560, 1_688, 1_816, 1_944,
    2_072, 2_328, 2_584, 2_840, 3_096, 3_352, 3_608, 3_864, 4_120, 4_632, 5_144, 5_656, 6_168, 6_680, 7_192,
    7_704, 8_216, 9_240, 10_264, 11_288, 12_312, 13_336, 14_360, 15_384, 16_408, 18_456, 20_504, 22_552, 24_600,
    26_648, 28_696, 30_744, 32_792, 36_888, 40_984, 45_080, 49_176, 53_272, 57_368, 61_464, 65_560, 73_752,
    81_944, 90_136, 98_328, 106_520, 114_712, 122_904, 131_096, 147_480, 163_864, 180_248, 196_632, 213_016,
    229_400, 245_784, 262_168, 294_936, 327_704, 360_472, 393_240, 426_008, 458_776, 491_544, 524_312, 589_848,
    655_384, 720_920, 786_456, 851_992, 917_528, 983_064, 1_048_600, 1_179_672, 1_310_744, 1_441_816, 1_572_888,
    1_703_960, 1_835_032, 1_966_104, 2_097_176, 2_359_320, 2_621_464, 2_883_608, 3_145_752, 3_407_896, 3_670_040,
    3_932_184, 4_194_328, 4_718_616, 5_242_904, 5_767_192, 6_291_480, 6_815_768, 7_340_056, 7_864_344, 8_388_632,
    9_437_208, 10_485_784, 11_534_360, 12_582_936, 13_631_512, 14_680_088, 15_728_664, 16_777_240, 18_874_392,
    20_971_544, 23_068_696, 25_165_848, 27_263_000, 29_360_152, 31_457_304, 33_554_456, 37_748_760, 41_943_064,
    46_137_368, 50_331_672, 54_525_976, 58_720_280, 62_914_584, 67_108_888, 75_497_496, 83_886_104, 92_274_712,
    100_663_320, 109_051_928, 117_440_536, 125_829_144, 134_217_752, 150_994_968, 167_772_184, 184_549_400,
    201_326_616, 218_103_832, 234_881_048, 251_658_264, 268_435_480, 301_989_912, 335_544_344, 369_098_776,
    402_653_208, 436_207_640, 469_762_072, 503_316_504, 536_870_936, 603_979_800, 671_088_664, 738_197_528,
    805_306_392, 872_415_256, 939_524_120, 1_006_632_984, 1_073_741_848, 1_207_959_576, 1_342_177_304,
    1_476_395_032, 1_610_612_760, 1_744_830_488, 1_879_048_216, 2_013_265_944,
];

fn tantivy_length(len: u32) -> u32 {
    let id = TANTIVY_NORMS.binary_search(&len).unwrap_or_else(|at| at - 1);
    TANTIVY_NORMS[id]
}

/// Lucene's SmallFloat.longToInt4.
fn long_to_int4(i: u64) -> u32 {
    let bits = 64 - i.leading_zeros();
    if bits < 4 {
        return i as u32;
    }
    let shift = bits - 4;
    let encoded = ((i >> shift) as u32) & 0x07;
    encoded | ((shift + 1) << 3)
}

/// Lucene's SmallFloat.int4ToLong.
fn int4_to_long(i: u32) -> u64 {
    let bits = u64::from(i & 0x07);
    let shift = (i >> 3) as i32 - 1;
    if shift == -1 { bits } else { (bits | 0x08) << shift }
}

/// A length through Lucene's norm byte and back: intToByte4, then byte4ToInt.
fn lucene_length(len: u32) -> u32 {
    let free = 255 - long_to_int4(i32::MAX as u64);
    if len < free {
        return len;
    }
    let byte = free + long_to_int4(u64::from(len - free));
    (u64::from(free) + int4_to_long(byte - free)) as u32
}

fn idf_lucene(n: f64, df: f64) -> f64 {
    (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
}

fn idf_fts5(n: f64, df: f64) -> f64 {
    let idf = ((n - df + 0.5) / (df + 0.5)).ln();
    if idf <= 0.0 { 1e-6 } else { idf }
}

/// The ten best of the touched documents by one variant, and how many share
/// the tenth score: in all, and among the ten.
fn ten(touched: &[u32], scores: &[[f64; 4]], variant: usize, k: usize) -> Value {
    let mut all: Vec<(f64, u32)> = touched.iter().map(|doc| (scores[*doc as usize][variant], *doc)).collect();
    let order = |a: &(f64, u32), b: &(f64, u32)| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1));
    if all.len() > k {
        all.select_nth_unstable_by(k - 1, order);
    }
    let mut best: Vec<(f64, u32)> = all[..all.len().min(k)].to_vec();
    best.sort_by(order);
    let (tied, tied_in) = match best.last() {
        Some((last, _)) if best.len() == k => (
            all.iter().filter(|(score, _)| score == last).count(),
            best.iter().filter(|(score, _)| score == last).count(),
        ),
        _ => (0, 0),
    };
    json!({"ten": best.iter().map(|(score, doc)| json!([doc, score])).collect::<Vec<_>>(),
           "tied_at_tenth": tied, "tied_in_ten": tied_in})
}

pub fn exact(dir: &Path, queries: &Path, out: &Path, threads: usize) {
    let started = Instant::now();
    let collection = Collection::open(dir);
    let numbers = term_numbers(dir);
    let n = f64::from(collection.documents);
    let average = collection.tokens as f64 / n;
    let mut lengths_differ = 0u64;
    // A document's k1 (1 − b + b · len / avg), by variant.
    let norms: Vec<[f64; 4]> = collection
        .sizes
        .iter()
        .map(|size| {
            let (lucene, tantivy) = (lucene_length(*size), tantivy_length(*size));
            lengths_differ += u64::from(lucene != tantivy);
            let norm = |len: u32| K1 * (1.0 - B + B * f64::from(len) / average);
            [norm(*size), norm(*size), norm(lucene), norm(tantivy)]
        })
        .collect();
    let asked = crate::queries::read(queries);
    let lines: Vec<String> = std::thread::scope(|scope| {
        let chunk = asked.len().div_ceil(threads).max(1);
        let handles: Vec<_> = asked
            .chunks(chunk)
            .map(|slice| {
                let (collection, numbers, norms) = (&collection, &numbers, &norms);
                scope.spawn(move || {
                    let mut scores = vec![[0.0f64; 4]; collection.documents as usize];
                    let mut touched: Vec<u32> = Vec::new();
                    let mut lines = Vec::new();
                    for (qid, terms) in slice {
                        let mut postings = 0u64;
                        for term in terms {
                            let number = numbers[term] as usize;
                            let (docs, freqs) = collection.list(number);
                            let df = docs.len() as f64;
                            postings += docs.len() as u64;
                            let (lucene, fts5) = (idf_lucene(n, df), idf_fts5(n, df));
                            let idfs = [lucene, fts5, lucene, lucene];
                            for (doc, tf) in docs.iter().zip(freqs.iter()) {
                                let at = *doc as usize;
                                let tf = f64::from(*tf);
                                let entry = &mut scores[at];
                                if entry[0] == 0.0 {
                                    touched.push(*doc);
                                }
                                for variant in 0..4 {
                                    entry[variant] += idfs[variant] * tf * (K1 + 1.0) / (tf + norms[at][variant]);
                                }
                            }
                        }
                        let mut line = json!({"qid": qid, "terms": terms.len(), "postings": postings,
                            "matched": touched.len()});
                        for (variant, name) in VARIANTS.iter().enumerate() {
                            line[*name] = ten(&touched, &scores, variant, 10);
                        }
                        for doc in touched.drain(..) {
                            scores[doc as usize] = [0.0; 4];
                        }
                        lines.push(line.to_string());
                    }
                    lines
                })
            })
            .collect();
        handles.into_iter().flat_map(|handle| handle.join().unwrap()).collect()
    });
    let mut file = std::fs::File::create(out).unwrap();
    for line in &lines {
        writeln!(file, "{line}").unwrap();
    }
    println!(
        "{}",
        json!({"step": "exact", "queries": asked.len(), "seconds": started.elapsed().as_secs_f64(),
            "documents": collection.documents, "average_length": average,
            "documents_whose_lucene_and_tantivy_lengths_differ": lengths_differ, "threads": threads})
    );
}
