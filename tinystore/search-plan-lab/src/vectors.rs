//! Vectors beside the words: a cohort's passages as embed.py wrote them, kept
//! in SQLite as given, clustered by spherical k-means, and coded three ways
//! in each cluster's list: a byte a dimension with what it leaves out, the
//! sign of each dimension, and RaBitQ's bit a dimension of the vector's
//! offset from its cluster, turned by a random rotation. A question reads
//! them exactly, by bounds, or approximately, from the clusters nearest it,
//! its candidates scored again on the vectors as given. Controls: sqlite-vec,
//! which scans every vector in SQLite, SQLite's own vec1, IVF with OPQ codes,
//! and usearch's HNSW graph in memory.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Instant;

use rusqlite::{Connection, params};
use serde_json::json;

pub const DIM: usize = 1024;
const SIGN_BYTES: usize = DIM / 8;
/// Members a row of a cluster's list holds at most.
const PART: usize = 256;
/// What a kernel's rounding may move a score by: far more than a sum of
/// 1,024 products of numbers below one loses in f32.
const SLACK: f32 = 1e-4;
/// A cluster's rows are numbered cluster × 2¹⁶ + part.
const PARTS: i64 = 1 << 16;

const SCHEMA: &str = "
create table vf32 (doc integer primary key, v blob not null);
create table centroids (cluster integer primary key, centroid blob not null, center real not null,
  cos_min real not null, norm_max real not null, members integer not null);
create table lcodes (id integer primary key, docs blob not null, scales blob not null, errors blob not null,
  codes blob not null);
create table lsigns (id integer primary key, docs blob not null, signs blob not null);
create table lrabitq (id integer primary key, docs blob not null, norms blob not null, factors blob not null,
  bits blob not null);
";

/// Kernels for AVX2 and FMA, which `check` makes sure of first.
mod kernel {
    use super::DIM;

    pub fn check() {
        assert!(
            is_x86_feature_detected!("avx2")
                && is_x86_feature_detected!("fma")
                && is_x86_feature_detected!("popcnt"),
            "the vectors' kernels need AVX2, FMA and POPCNT"
        );
    }

    #[target_feature(enable = "avx2,fma")]
    fn dot_avx2(a: &[f32], b: &[f32]) -> f32 {
        // Eight registers of sums, so that eight FMAs are in flight at once.
        let mut acc = [0f32; 64];
        for (x, y) in a.chunks_exact(64).zip(b.chunks_exact(64)) {
            for i in 0..64 {
                acc[i] = x[i].mul_add(y[i], acc[i]);
            }
        }
        acc.iter().sum()
    }

    #[target_feature(enable = "avx2,fma")]
    fn dot_codes_avx2(q: &[f32], code: &[u8]) -> f32 {
        let mut acc = [0f32; 64];
        for (x, y) in q.chunks_exact(64).zip(code.chunks_exact(64)) {
            for i in 0..64 {
                acc[i] = x[i].mul_add(f32::from(y[i] as i8), acc[i]);
            }
        }
        acc.iter().sum()
    }

    #[target_feature(enable = "avx2,popcnt")]
    fn hamming_avx2(a: &[u8], b: &[u8]) -> u32 {
        a.chunks_exact(8)
            .zip(b.chunks_exact(8))
            .map(|(x, y)| {
                (u64::from_le_bytes(x.try_into().unwrap())
                    ^ u64::from_le_bytes(y.try_into().unwrap()))
                .count_ones()
            })
            .sum()
    }

    /// The bits set in `bits`, and the sum of the query's 4-bit values where
    /// they are set, from the query's four bit planes.
    #[target_feature(enable = "avx2,popcnt")]
    fn planes_avx2(bits: &[u8], planes: &[[u64; DIM / 64]; 4]) -> (u32, u32) {
        let (mut ones, mut weighted) = (0, 0);
        for (word, chunk) in bits.chunks_exact(8).enumerate() {
            let b = u64::from_le_bytes(chunk.try_into().unwrap());
            ones += b.count_ones();
            for (shift, plane) in planes.iter().enumerate() {
                weighted += (b & plane[word]).count_ones() << shift;
            }
        }
        (ones, weighted)
    }

    pub fn dot(a: &[f32], b: &[f32]) -> f32 {
        // SAFETY: every command that reaches a kernel ran `check` first.
        unsafe { dot_avx2(a, b) }
    }

    pub fn dot_codes(q: &[f32], code: &[u8]) -> f32 {
        // SAFETY: as in `dot`.
        unsafe { dot_codes_avx2(q, code) }
    }

    pub fn hamming(a: &[u8], b: &[u8]) -> u32 {
        // SAFETY: as in `dot`.
        unsafe { hamming_avx2(a, b) }
    }

    pub fn planes(bits: &[u8], planes: &[[u64; DIM / 64]; 4]) -> (u32, u32) {
        // SAFETY: as in `dot`.
        unsafe { planes_avx2(bits, planes) }
    }
}

pub use kernel::{check, dot};

/// A document and its score; the greater is the better, ties to the lower doc.
#[derive(Clone, Copy, PartialEq)]
pub struct Scored {
    pub score: f32,
    pub doc: u32,
}

impl Eq for Scored {}

impl Ord for Scored {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then(other.doc.cmp(&self.doc))
    }
}

impl PartialOrd for Scored {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The `k` best of what it is given.
pub struct Top {
    k: usize,
    heap: BinaryHeap<Reverse<Scored>>,
}

impl Top {
    pub fn new(k: usize) -> Top {
        Top {
            k,
            heap: BinaryHeap::with_capacity(k + 1),
        }
    }

    pub fn push(&mut self, scored: Scored) {
        if self.heap.len() < self.k {
            self.heap.push(Reverse(scored));
        } else if scored > self.heap.peek().unwrap().0 {
            self.heap.pop();
            self.heap.push(Reverse(scored));
        }
    }

    /// The score a newcomer must reach, once `k` are held.
    pub fn threshold(&self) -> f32 {
        if self.heap.len() < self.k {
            f32::NEG_INFINITY
        } else {
            self.heap.peek().unwrap().0.score
        }
    }

    pub fn sorted(self) -> Vec<Scored> {
        self.heap
            .into_sorted_vec()
            .into_iter()
            .map(|reversed| reversed.0)
            .collect()
    }
}

/// Rows of DIM floats from a file embed.py wrote, a block at a time.
pub struct Rows {
    reader: BufReader<File>,
    pub count: usize,
    read: usize,
}

impl Rows {
    pub fn open(path: &Path) -> Rows {
        let file = File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let count = (file.metadata().unwrap().len() / (DIM as u64 * 4)) as usize;
        Rows {
            reader: BufReader::with_capacity(1 << 22, file),
            count,
            read: 0,
        }
    }

    /// Up to `most` rows into `out`; the number read, 0 at the end.
    pub fn next(&mut self, most: usize, out: &mut Vec<f32>) -> usize {
        let rows = most.min(self.count - self.read);
        let mut bytes = vec![0u8; rows * DIM * 4];
        self.reader.read_exact(&mut bytes).unwrap();
        out.clear();
        out.extend(
            bytes
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap())),
        );
        self.read += rows;
        rows
    }
}

pub fn read_all(path: &Path) -> Vec<f32> {
    let mut rows = Rows::open(path);
    let mut out = Vec::new();
    rows.next(rows.count, &mut out);
    out
}

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

fn float_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn word_bytes(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn norm(v: &[f32]) -> f64 {
    v.iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt()
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// The best of `columns` for every row by inner product, the rows split
/// among the threads, 64 rows against every column at a time so that each
/// column is read once a block.
fn nearest(rows: &[f32], columns: &[f32]) -> Vec<Scored> {
    let n = rows.len() / DIM;
    let mut out = vec![
        Scored {
            score: f32::NEG_INFINITY,
            doc: 0
        };
        n
    ];
    let per = n.div_ceil(threads()).max(1);
    std::thread::scope(|scope| {
        for (mine, outs) in rows.chunks(per * DIM).zip(out.chunks_mut(per)) {
            scope.spawn(move || {
                for (block, outs) in mine.chunks(64 * DIM).zip(outs.chunks_mut(64)) {
                    for (column, c) in columns.chunks_exact(DIM).enumerate() {
                        for (row, best) in block.chunks_exact(DIM).zip(outs.iter_mut()) {
                            let scored = Scored {
                                score: dot(row, c),
                                doc: column as u32,
                            };
                            if scored > *best {
                                *best = scored;
                            }
                        }
                    }
                }
            });
        }
    });
    out
}

/// Spherical k-means: centroids of length one, each the direction of its
/// members' sum, seeded from evenly spaced rows of the sample; a cluster
/// left empty takes the row that fits its own centroid worst.
fn train(sample: &[f32], clusters: usize, iterations: usize) -> (Vec<f32>, Vec<f64>) {
    let n = sample.len() / DIM;
    let mut centroids: Vec<f32> = (0..clusters)
        .flat_map(|c| sample[c * n / clusters * DIM..][..DIM].iter().copied())
        .collect();
    let mut fits = Vec::new();
    for _ in 0..iterations {
        let assigned = nearest(sample, &centroids);
        fits.push(
            assigned
                .iter()
                .map(|best| f64::from(best.score))
                .sum::<f64>()
                / n as f64,
        );
        let mut sums = vec![0f64; clusters * DIM];
        let mut counts = vec![0u32; clusters];
        for (row, best) in sample.chunks_exact(DIM).zip(&assigned) {
            let cluster = best.doc as usize;
            counts[cluster] += 1;
            for (sum, x) in sums[cluster * DIM..][..DIM].iter_mut().zip(row) {
                *sum += f64::from(*x);
            }
        }
        let mut worst: Vec<usize> = (0..n).collect();
        worst.sort_by(|a, b| assigned[*a].score.total_cmp(&assigned[*b].score));
        let mut reseeded = 0;
        for (cluster, centroid) in centroids.chunks_exact_mut(DIM).enumerate() {
            if counts[cluster] == 0 {
                centroid.copy_from_slice(&sample[worst[reseeded] * DIM..][..DIM]);
                reseeded += 1;
                continue;
            }
            let sum = &sums[cluster * DIM..][..DIM];
            let length = sum.iter().map(|x| x * x).sum::<f64>().sqrt();
            for (value, x) in centroid.iter_mut().zip(sum) {
                *value = (x / length) as f32;
            }
        }
    }
    (centroids, fits)
}

/// A byte a dimension, scaled by the vector's largest magnitude, and the
/// length of what the bytes leave out, rounded up: |q·v − scale·q·code| is at
/// most |q| times it.
fn quantize(v: &[f32], code: &mut Vec<u8>) -> (f32, f32) {
    let largest = v.iter().fold(0f32, |most, x| most.max(x.abs()));
    let scale = if largest > 0.0 { largest / 127.0 } else { 1.0 };
    let mut left = 0f64;
    for x in v {
        let level = (x / scale).round().clamp(-127.0, 127.0);
        code.push(level as i8 as u8);
        let rest = f64::from(*x) - f64::from(scale) * f64::from(level);
        left += rest * rest;
    }
    (scale, (left.sqrt() as f32).next_up())
}

fn signs(v: &[f32], out: &mut Vec<u8>) {
    for chunk in v.chunks_exact(8) {
        out.push(
            chunk
                .iter()
                .enumerate()
                .fold(0u8, |byte, (bit, x)| byte | (u8::from(*x > 0.0) << bit)),
        );
    }
}

/// Three rounds of random signs and a Walsh–Hadamard transform: an
/// orthogonal rotation at D log D, seeded so that every process turns alike.
pub struct Rotation {
    signs: Vec<[f32; DIM]>,
}

impl Rotation {
    pub fn new() -> Rotation {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let signs = (0..3)
            .map(|_| {
                let mut round = [0f32; DIM];
                for value in &mut round {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    *value = if state & 1 == 1 { 1.0 } else { -1.0 };
                }
                round
            })
            .collect();
        Rotation { signs }
    }

    pub fn turn(&self, v: &[f32]) -> Vec<f32> {
        let mut out = v.to_vec();
        let scale = 1.0 / (DIM as f32).sqrt();
        for round in &self.signs {
            for (value, sign) in out.iter_mut().zip(round) {
                *value *= sign;
            }
            let mut half = 1;
            while half < DIM {
                for start in (0..DIM).step_by(half * 2) {
                    for at in start..start + half {
                        let (a, b) = (out[at], out[at + half]);
                        out[at] = a + b;
                        out[at + half] = a - b;
                    }
                }
                half *= 2;
            }
            for value in &mut out {
                *value *= scale;
            }
        }
        out
    }
}

/// RaBitQ's code of `v` in a cluster centred on `center`: the bits of its
/// turned offset's signs, the offset's length, and ⟨ō, o⟩, the cosine of
/// the offset with what its bits stand for, which the estimate divides by.
fn rabitq(v: &[f32], center: &[f32], rotation: &Rotation, bits: &mut Vec<u8>) -> (f32, f32) {
    let offset: Vec<f32> = v.iter().zip(center).map(|(x, c)| x - c).collect();
    let length = norm(&offset);
    let unit: Vec<f32> = offset
        .iter()
        .map(|x| (f64::from(*x) / length) as f32)
        .collect();
    let turned = rotation.turn(&unit);
    signs(&turned, bits);
    let factor = turned.iter().map(|x| f64::from(x.abs())).sum::<f64>() / (DIM as f64).sqrt();
    (length as f32, factor as f32)
}

/// The exact k best of every question, in memory: the answer every arm is
/// measured against.
pub fn truth(vectors: &Path, k: usize) -> serde_json::Value {
    check();
    let started = Instant::now();
    let questions = read_all(&vectors.join("questions.f32"));
    let asked = questions.len() / DIM;
    let mut rows = Rows::open(&vectors.join("docs.f32"));
    let mut tops: Vec<Top> = (0..asked).map(|_| Top::new(k)).collect();
    let mut block = Vec::new();
    let mut first = 0usize;
    loop {
        let read = rows.next(16_384, &mut block);
        if read == 0 {
            break;
        }
        let per = read.div_ceil(threads());
        let found: Vec<Vec<Top>> = std::thread::scope(|scope| {
            let handles: Vec<_> = block
                .chunks(per * DIM)
                .enumerate()
                .map(|(part, mine)| {
                    let questions = &questions;
                    scope.spawn(move || {
                        let mut tops: Vec<Top> = (0..asked).map(|_| Top::new(k)).collect();
                        for (offset, rows) in mine.chunks(64 * DIM).enumerate() {
                            for (question, q) in questions.chunks_exact(DIM).enumerate() {
                                for (at, row) in rows.chunks_exact(DIM).enumerate() {
                                    let doc = (first + part * per + offset * 64 + at + 1) as u32;
                                    tops[question].push(Scored {
                                        score: dot(q, row),
                                        doc,
                                    });
                                }
                            }
                        }
                        tops
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });
        for part in found {
            for (top, theirs) in tops.iter_mut().zip(part) {
                for scored in theirs.sorted() {
                    top.push(scored);
                }
            }
        }
        first += read;
    }
    let answers: Vec<Vec<(u32, f32)>> = tops
        .into_iter()
        .map(|top| top.sorted().into_iter().map(|s| (s.doc, s.score)).collect())
        .collect();
    std::fs::write(
        vectors.join("truth.json"),
        serde_json::to_vec(&answers).unwrap(),
    )
    .unwrap();
    json!({"step": "truth", "questions": asked, "documents": first, "k": k,
        "seconds": started.elapsed().as_secs_f64(), "threads": threads()})
}

pub fn read_truth(vectors: &Path) -> Vec<Vec<Scored>> {
    let answers: Vec<Vec<(u32, f32)>> =
        serde_json::from_slice(&std::fs::read(vectors.join("truth.json")).unwrap()).unwrap();
    answers
        .into_iter()
        .map(|answer| {
            answer
                .into_iter()
                .map(|(doc, score)| Scored { score, doc })
                .collect()
        })
        .collect()
}

/// One cluster's members, coded every way, gathered before its rows are written.
#[derive(Default)]
struct List {
    docs: Vec<u32>,
    scales: Vec<f32>,
    errors: Vec<f32>,
    codes: Vec<u8>,
    signs: Vec<u8>,
    norms: Vec<f32>,
    factors: Vec<f32>,
    bits: Vec<u8>,
    cos_min: f64,
    norm_max: f64,
    projections: f64,
}

pub fn open(path: &Path) -> Connection {
    crate::fts::open(path)
}

/// vectors.db: every vector as given, the centroids, and each cluster's
/// list coded three ways.
pub fn build(
    vectors: &Path,
    out: &Path,
    clusters: usize,
    sample: usize,
    iterations: usize,
) -> serde_json::Value {
    check();
    let docs = vectors.join("docs.f32");
    let path = out.join("vectors.db");
    let _ = std::fs::remove_file(&path);
    std::fs::create_dir_all(out).unwrap();

    let started = Instant::now();
    let mut rows = Rows::open(&docs);
    let count = rows.count;
    let step = (count / sample).max(1);
    let mut taken = Vec::with_capacity(sample * DIM);
    let mut block = Vec::new();
    let mut at = 0;
    while rows.next(16_384, &mut block) > 0 {
        for row in block.chunks_exact(DIM) {
            if at % step == 0 && taken.len() < sample * DIM {
                taken.extend_from_slice(row);
            }
            at += 1;
        }
    }
    let sampled = started.elapsed().as_secs_f64();
    let (centroids, fits) = train(&taken, clusters, iterations);
    drop(taken);
    let trained = started.elapsed().as_secs_f64() - sampled;

    // Every vector as given, its cluster, its byte and sign codes.
    let mut connection = open(&path);
    connection.execute_batch(SCHEMA).unwrap();
    let mut lists: Vec<List> = (0..clusters)
        .map(|_| List {
            cos_min: 1.0,
            ..List::default()
        })
        .collect();
    let mut assigned = Vec::with_capacity(count);
    let mut rows = Rows::open(&docs);
    let mut doc = 0u32;
    while rows.next(16_384, &mut block) > 0 {
        let best = nearest(&block, &centroids);
        let tx = connection.transaction().unwrap();
        {
            let mut keep = tx
                .prepare_cached("insert into vf32 (doc, v) values (?1, ?2)")
                .unwrap();
            for (row, best) in block.chunks_exact(DIM).zip(best) {
                doc += 1;
                keep.execute(params![doc, float_bytes(row)]).unwrap();
                let list = &mut lists[best.doc as usize];
                list.docs.push(doc);
                let (scale, error) = quantize(row, &mut list.codes);
                list.scales.push(scale);
                list.errors.push(error);
                signs(row, &mut list.signs);
                let length = norm(row);
                let centroid = &centroids[best.doc as usize * DIM..][..DIM];
                let projection = row
                    .iter()
                    .zip(centroid)
                    .map(|(x, c)| f64::from(*x) * f64::from(*c))
                    .sum::<f64>();
                list.cos_min = list.cos_min.min(projection / length);
                list.norm_max = list.norm_max.max(length);
                list.projections += projection;
                assigned.push(best.doc);
            }
        }
        tx.commit().unwrap();
    }
    let placed = started.elapsed().as_secs_f64() - sampled - trained;

    // RaBitQ's codes, offsets from each cluster's centre: its centroid times
    // its members' mean projection on it.
    let rotation = Rotation::new();
    let centers: Vec<f64> = lists
        .iter()
        .map(|list| list.projections / list.docs.len().max(1) as f64)
        .collect();
    let mut rows = Rows::open(&docs);
    let mut at = 0;
    while rows.next(16_384, &mut block) > 0 {
        for row in block.chunks_exact(DIM) {
            let cluster = assigned[at] as usize;
            let center: Vec<f32> = centroids[cluster * DIM..][..DIM]
                .iter()
                .map(|c| (f64::from(*c) * centers[cluster]) as f32)
                .collect();
            let list = &mut lists[cluster];
            let (length, factor) = rabitq(row, &center, &rotation, &mut list.bits);
            list.norms.push(length);
            list.factors.push(factor);
            at += 1;
        }
    }
    let coded = started.elapsed().as_secs_f64() - sampled - trained - placed;

    let tx = connection.transaction().unwrap();
    {
        let mut centroid_row = tx
            .prepare("insert into centroids (cluster, centroid, center, cos_min, norm_max, members) values (?1, ?2, ?3, ?4, ?5, ?6)")
            .unwrap();
        let mut codes_row = tx
            .prepare(
                "insert into lcodes (id, docs, scales, errors, codes) values (?1, ?2, ?3, ?4, ?5)",
            )
            .unwrap();
        let mut signs_row = tx
            .prepare("insert into lsigns (id, docs, signs) values (?1, ?2, ?3)")
            .unwrap();
        let mut rabitq_row = tx
            .prepare(
                "insert into lrabitq (id, docs, norms, factors, bits) values (?1, ?2, ?3, ?4, ?5)",
            )
            .unwrap();
        for (cluster, list) in lists.iter().enumerate() {
            // Rounded away from the bound's favour, so that it stays a bound.
            centroid_row
                .execute(params![
                    cluster as i64,
                    float_bytes(&centroids[cluster * DIM..][..DIM]),
                    centers[cluster],
                    list.cos_min - 1e-6,
                    list.norm_max + 1e-6,
                    list.docs.len() as i64
                ])
                .unwrap();
            for (part, docs) in list.docs.chunks(PART).enumerate() {
                let id = cluster as i64 * PARTS + part as i64;
                let range = part * PART..part * PART + docs.len();
                codes_row
                    .execute(params![
                        id,
                        word_bytes(docs),
                        float_bytes(&list.scales[range.clone()]),
                        float_bytes(&list.errors[range.clone()]),
                        &list.codes[range.start * DIM..range.end * DIM]
                    ])
                    .unwrap();
                signs_row
                    .execute(params![
                        id,
                        word_bytes(docs),
                        &list.signs[range.start * SIGN_BYTES..range.end * SIGN_BYTES]
                    ])
                    .unwrap();
                rabitq_row
                    .execute(params![
                        id,
                        word_bytes(docs),
                        float_bytes(&list.norms[range.clone()]),
                        float_bytes(&list.factors[range.clone()]),
                        &list.bits[range.start * SIGN_BYTES..range.end * SIGN_BYTES]
                    ])
                    .unwrap();
            }
        }
    }
    tx.commit().unwrap();
    connection
        .pragma_update(None, "wal_checkpoint", "truncate")
        .unwrap();
    let written = started.elapsed().as_secs_f64() - sampled - trained - placed - coded;
    let objects = crate::fts::objects(&connection);
    drop(connection);

    let mut sizes: Vec<usize> = lists.iter().map(|list| list.docs.len()).collect();
    sizes.sort_unstable();
    let mut caps: Vec<f64> = lists
        .iter()
        .filter(|list| !list.docs.is_empty())
        .map(|list| list.cos_min)
        .collect();
    caps.sort_by(f64::total_cmp);
    let at = |values: &[f64], share: f64| {
        values[((values.len() as f64 * share) as usize).min(values.len() - 1)]
    };
    json!({"step": "vbuild", "backend": "vectors", "documents": count, "clusters": clusters, "sample": sample,
        "iterations": iterations, "fits": fits, "threads": threads(),
        "seconds": {"sample": sampled, "train": trained, "place": placed, "rabitq": coded, "lists": written,
            "total": started.elapsed().as_secs_f64()},
        "members": {"min": sizes[0], "p50": sizes[sizes.len() / 2], "p99": sizes[sizes.len() * 99 / 100],
            "max": sizes[sizes.len() - 1], "empty": sizes.iter().filter(|size| **size == 0).count()},
        "cap_cos_min": {"p1": at(&caps, 0.01), "p50": at(&caps, 0.5), "p99": at(&caps, 0.99)},
        "bytes": std::fs::metadata(&path).unwrap().len(), "objects": objects})
}

/// What a search read, summed over the questions asked.
#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub clusters: u64,
    pub rows: u64,
    pub members: u64,
    pub bytes: u64,
    pub rescored: u64,
}

impl Counts {
    pub fn per(&self, questions: usize) -> serde_json::Value {
        let n = questions as f64;
        json!({"clusters": self.clusters as f64 / n, "rows": self.rows as f64 / n, "members": self.members as f64 / n,
            "bytes": self.bytes as f64 / n, "rescored": self.rescored as f64 / n})
    }
}

/// A reader of vectors.db: the centroids in memory, the lists and the
/// vectors read as a search needs them.
pub struct Reader {
    connection: Connection,
    centroids: Vec<f32>,
    centers: Vec<f32>,
    /// The widest angle between a cluster's centroid and a member.
    angles: Vec<f64>,
    norm_max: Vec<f64>,
    rotation: Rotation,
    pub counts: Counts,
}

impl Reader {
    pub fn open(out: &Path) -> Reader {
        check();
        let connection = open(&out.join("vectors.db"));
        connection.pragma_update(None, "query_only", true).unwrap();
        let (mut centroids, mut centers, mut angles, mut norm_max) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        {
            let mut statement = connection
                .prepare(
                    "select centroid, center, cos_min, norm_max from centroids order by cluster",
                )
                .unwrap();
            let mut rows = statement.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                centroids.extend(floats(row.get_ref(0).unwrap().as_blob().unwrap()));
                centers.push(row.get::<_, f64>(1).unwrap() as f32);
                angles.push(row.get::<_, f64>(2).unwrap().clamp(-1.0, 1.0).acos());
                norm_max.push(row.get(3).unwrap());
            }
        }
        Reader {
            connection,
            centroids,
            centers,
            angles,
            norm_max,
            rotation: Rotation::new(),
            counts: Counts::default(),
        }
    }

    fn affinities(&self, q: &[f32]) -> Vec<Scored> {
        self.centroids
            .chunks_exact(DIM)
            .enumerate()
            .map(|(cluster, c)| Scored {
                score: dot(q, c),
                doc: cluster as u32,
            })
            .collect()
    }

    /// The `probes` clusters whose centroids lie nearest the question.
    fn nearest_clusters(mut all: Vec<Scored>, probes: usize) -> Vec<u32> {
        let probes = probes.min(all.len());
        all.select_nth_unstable_by(probes - 1, |a, b| b.cmp(a));
        all.truncate(probes);
        all.into_iter().map(|scored| scored.doc).collect()
    }

    /// `docs` scored on their vectors as given, the best first.
    pub fn score(&mut self, q: &[f32], docs: &[u32]) -> Vec<Scored> {
        self.rescore(q, docs.iter().copied(), docs.len())
    }

    /// `docs` scored on their vectors as given.
    fn rescore(&mut self, q: &[f32], docs: impl Iterator<Item = u32>, k: usize) -> Vec<Scored> {
        let mut statement = self
            .connection
            .prepare_cached("select v from vf32 where doc = ?1")
            .unwrap();
        let mut top = Top::new(k);
        for doc in docs {
            let v: Vec<f32> = statement
                .query_row(params![doc], |row| Ok(floats(row.get_ref(0)?.as_blob()?)))
                .unwrap();
            top.push(Scored {
                score: dot(q, &v),
                doc,
            });
            self.counts.rescored += 1;
        }
        top.sorted()
    }

    /// Each member of `cluster`'s byte codes: its doc, scale, error and code.
    fn each_code(&mut self, cluster: u32, mut visit: impl FnMut(u32, f32, f32, &[u8])) {
        let mut statement = self
            .connection
            .prepare_cached(
                "select docs, scales, errors, codes from lcodes where id >= ?1 and id < ?2",
            )
            .unwrap();
        let first = i64::from(cluster) * PARTS;
        let mut rows = statement.query(params![first, first + PARTS]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let docs = row.get_ref(0).unwrap().as_blob().unwrap();
            let scales = floats(row.get_ref(1).unwrap().as_blob().unwrap());
            let errors = floats(row.get_ref(2).unwrap().as_blob().unwrap());
            let codes = row.get_ref(3).unwrap().as_blob().unwrap();
            for (at, doc) in docs.chunks_exact(4).enumerate() {
                let doc = u32::from_le_bytes(doc.try_into().unwrap());
                visit(doc, scales[at], errors[at], &codes[at * DIM..][..DIM]);
            }
            self.counts.rows += 1;
            self.counts.members += scales.len() as u64;
            self.counts.bytes += (docs.len() + scales.len() * 8 + codes.len()) as u64;
        }
        self.counts.clusters += 1;
    }

    /// The exact `k` best: clusters in the order of what their members could
    /// score at most, each member's byte code giving a range its score lies
    /// in, every member whose range reaches the k-th best lower end scored
    /// again on its vector as given. A cluster stops the search once its
    /// bound falls below that lower end.
    ///
    /// A cluster's bound: no member lies further than its widest angle θ
    /// from the centroid, so one at angle α from the question is no nearer
    /// it than α − θ, and q·v ≤ |q| |v|max cos(max(0, α − θ)).
    pub fn exact(&mut self, q: &[f32], k: usize) -> Vec<Scored> {
        let length = norm(q);
        let mut order: Vec<Scored> = self
            .affinities(q)
            .into_iter()
            .map(|scored| {
                let cluster = scored.doc as usize;
                let alpha = (f64::from(scored.score) / length).clamp(-1.0, 1.0).acos();
                let bound =
                    length * self.norm_max[cluster] * (alpha - self.angles[cluster]).max(0.0).cos();
                Scored {
                    score: bound as f32 + SLACK,
                    doc: scored.doc,
                }
            })
            .collect();
        order.sort_unstable_by(|a, b| b.cmp(a));
        let mut lower = Top::new(k);
        let mut candidates: Vec<Scored> = Vec::new();
        let length = length as f32;
        for cluster in order {
            if cluster.score < lower.threshold() {
                break;
            }
            self.each_code(cluster.doc, |doc, scale, error, code| {
                let estimate = scale * kernel::dot_codes(q, code);
                let reach = length * error + SLACK;
                lower.push(Scored {
                    score: estimate - reach,
                    doc,
                });
                if estimate + reach >= lower.threshold() {
                    candidates.push(Scored {
                        score: estimate + reach,
                        doc,
                    });
                }
            });
        }
        let floor = lower.threshold();
        self.rescore(
            q,
            candidates
                .into_iter()
                .filter(|upper| upper.score >= floor)
                .map(|upper| upper.doc),
            k,
        )
    }

    /// The `keep` best by byte code among the `probes` nearest clusters,
    /// scored again on their vectors.
    pub fn codes(&mut self, q: &[f32], k: usize, probes: usize, keep: usize) -> Vec<Scored> {
        let mut top = Top::new(keep);
        for cluster in Reader::nearest_clusters(self.affinities(q), probes) {
            self.each_code(cluster, |doc, scale, _, code| {
                top.push(Scored {
                    score: scale * kernel::dot_codes(q, code),
                    doc,
                });
            });
        }
        let found = top.sorted();
        self.rescore(q, found.into_iter().map(|scored| scored.doc), k)
    }

    /// The `keep` nearest by the Hamming distance of their signs among the
    /// `probes` nearest clusters, scored again on their vectors.
    pub fn signs(&mut self, q: &[f32], k: usize, probes: usize, keep: usize) -> Vec<Scored> {
        let mut mine = Vec::with_capacity(SIGN_BYTES);
        signs(q, &mut mine);
        let mut top = Top::new(keep);
        let mut statement = self
            .connection
            .prepare_cached("select docs, signs from lsigns where id >= ?1 and id < ?2")
            .unwrap();
        for cluster in Reader::nearest_clusters(self.affinities(q), probes) {
            let first = i64::from(cluster) * PARTS;
            let mut rows = statement.query(params![first, first + PARTS]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let docs = row.get_ref(0).unwrap().as_blob().unwrap();
                let all = row.get_ref(1).unwrap().as_blob().unwrap();
                for (doc, theirs) in docs.chunks_exact(4).zip(all.chunks_exact(SIGN_BYTES)) {
                    let doc = u32::from_le_bytes(doc.try_into().unwrap());
                    top.push(Scored {
                        score: -(kernel::hamming(&mine, theirs) as f32),
                        doc,
                    });
                }
                self.counts.rows += 1;
                self.counts.members += (docs.len() / 4) as u64;
                self.counts.bytes += (docs.len() + all.len()) as u64;
            }
            self.counts.clusters += 1;
        }
        drop(statement);
        let found = top.sorted();
        self.rescore(q, found.into_iter().map(|scored| scored.doc), k)
    }

    /// The `keep` best by RaBitQ's estimate among the `probes` nearest
    /// clusters, scored again on their vectors. The estimate of q·v is
    /// q·c + |o| ⟨q', ō⟩ / ⟨ō, o⟩, q' the turned question held in four bits a
    /// dimension, so that ⟨q', ō⟩ comes from five popcounts a word.
    pub fn rabitq(&mut self, q: &[f32], k: usize, probes: usize, keep: usize) -> Vec<Scored> {
        let turned = self.rotation.turn(q);
        let low = turned.iter().copied().fold(f32::INFINITY, f32::min);
        let high = turned.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let step = (high - low) / 15.0;
        let mut planes = [[0u64; DIM / 64]; 4];
        let mut levels = 0u32;
        for (at, value) in turned.iter().enumerate() {
            let level = ((value - low) / step).round().clamp(0.0, 15.0) as u32;
            levels += level;
            for (shift, plane) in planes.iter_mut().enumerate() {
                plane[at / 64] |= u64::from((level >> shift) & 1) << (at % 64);
            }
        }
        let sum = low * DIM as f32 + step * levels as f32;
        let root = (DIM as f32).sqrt();
        let mut top = Top::new(keep);
        let mut statement = self
            .connection
            .prepare_cached(
                "select docs, norms, factors, bits from lrabitq where id >= ?1 and id < ?2",
            )
            .unwrap();
        let affinities = self.affinities(q);
        for cluster in Reader::nearest_clusters(affinities.clone(), probes) {
            let toward = affinities[cluster as usize].score * self.centers[cluster as usize];
            let first = i64::from(cluster) * PARTS;
            let mut rows = statement.query(params![first, first + PARTS]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let docs = row.get_ref(0).unwrap().as_blob().unwrap();
                let norms = floats(row.get_ref(1).unwrap().as_blob().unwrap());
                let factors = floats(row.get_ref(2).unwrap().as_blob().unwrap());
                let bits = row.get_ref(3).unwrap().as_blob().unwrap();
                for (at, (doc, theirs)) in docs
                    .chunks_exact(4)
                    .zip(bits.chunks_exact(SIGN_BYTES))
                    .enumerate()
                {
                    let doc = u32::from_le_bytes(doc.try_into().unwrap());
                    let (ones, weighted) = kernel::planes(theirs, &planes);
                    let set = low * ones as f32 + step * weighted as f32;
                    let estimate = toward + norms[at] * ((2.0 * set - sum) / root) / factors[at];
                    top.push(Scored {
                        score: estimate,
                        doc,
                    });
                }
                self.counts.rows += 1;
                self.counts.members += norms.len() as u64;
                self.counts.bytes += (docs.len() + norms.len() * 8 + bits.len()) as u64;
            }
            self.counts.clusters += 1;
        }
        drop(statement);
        let found = top.sorted();
        self.rescore(q, found.into_iter().map(|scored| scored.doc), k)
    }
}

/// usearch's HNSW graph, its vectors in f16, built from the same rows.
#[cfg(feature = "usearch")]
pub mod hnsw {
    use std::path::Path;
    use std::time::Instant;

    use serde_json::json;
    use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

    use super::{DIM, Rows, Scored, threads};

    fn options() -> IndexOptions {
        IndexOptions {
            dimensions: DIM,
            metric: MetricKind::IP,
            quantization: ScalarKind::F16,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        }
    }

    pub fn build(vectors: &Path, out: &Path) -> serde_json::Value {
        let started = Instant::now();
        let mut rows = Rows::open(&vectors.join("docs.f32"));
        let index = Index::new(&options()).unwrap();
        index
            .reserve_capacity_and_threads(rows.count, threads())
            .unwrap();
        let mut block = Vec::new();
        let mut first = 0usize;
        loop {
            let read = rows.next(16_384, &mut block);
            if read == 0 {
                break;
            }
            let per = read.div_ceil(threads());
            std::thread::scope(|scope| {
                for (part, mine) in block.chunks(per * DIM).enumerate() {
                    let index = &index;
                    scope.spawn(move || {
                        for (at, row) in mine.chunks_exact(DIM).enumerate() {
                            index
                                .add((first + part * per + at + 1) as u64, row)
                                .unwrap();
                        }
                    });
                }
            });
            first += read;
        }
        let path = out.join("hnsw.usearch");
        index.save(path.to_str().unwrap()).unwrap();
        json!({"step": "vbuild", "backend": "hnsw", "documents": first, "seconds": started.elapsed().as_secs_f64(),
            "bytes": std::fs::metadata(&path).unwrap().len(), "memory_usage": index.memory_usage(),
            "hardware": index.hardware_acceleration(), "threads": threads()})
    }

    pub struct Hnsw {
        index: Index,
    }

    impl Hnsw {
        pub fn open(out: &Path, expansion: usize) -> Hnsw {
            let index = Index::new(&options()).unwrap();
            index
                .load(out.join("hnsw.usearch").to_str().unwrap())
                .unwrap();
            index.change_expansion_search(expansion);
            Hnsw { index }
        }

        /// usearch's inner-product distance is one less the product.
        pub fn search(&self, q: &[f32], k: usize) -> Vec<Scored> {
            let found = self.index.search(q, k).unwrap();
            found
                .keys
                .iter()
                .zip(&found.distances)
                .map(|(key, distance)| Scored {
                    score: 1.0 - distance,
                    doc: *key as u32,
                })
                .collect()
        }
    }
}

/// sqlite-vec's vec0 table, which scans every vector, as its crate builds it.
#[cfg(feature = "sqlitevec")]
pub mod scan {
    use std::path::Path;
    use std::time::Instant;

    use rusqlite::{Connection, params};
    use serde_json::json;

    use super::{DIM, Rows, Scored, float_bytes};

    fn register() {
        // SAFETY: sqlite-vec's entry point has the signature SQLite's
        // auto-extensions take; the crate's own test registers it this way.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    }

    pub fn build(vectors: &Path, out: &Path) -> serde_json::Value {
        register();
        let started = Instant::now();
        let path = out.join("sqlitevec.db");
        let _ = std::fs::remove_file(&path);
        let mut connection = super::open(&path);
        connection
            .execute_batch(&format!(
                "create virtual table vec using vec0(embedding float[{DIM}])"
            ))
            .unwrap();
        let mut rows = Rows::open(&vectors.join("docs.f32"));
        let mut block = Vec::new();
        let mut doc = 0i64;
        while rows.next(16_384, &mut block) > 0 {
            let tx = connection.transaction().unwrap();
            {
                let mut insert = tx
                    .prepare_cached("insert into vec (rowid, embedding) values (?1, ?2)")
                    .unwrap();
                for row in block.chunks_exact(DIM) {
                    doc += 1;
                    insert.execute(params![doc, float_bytes(row)]).unwrap();
                }
            }
            tx.commit().unwrap();
        }
        connection
            .pragma_update(None, "wal_checkpoint", "truncate")
            .unwrap();
        let objects = crate::fts::objects(&connection);
        drop(connection);
        json!({"step": "vbuild", "backend": "sqlitevec", "documents": doc, "seconds": started.elapsed().as_secs_f64(),
            "bytes": std::fs::metadata(&path).unwrap().len(), "objects": objects})
    }

    pub struct Scan {
        connection: Connection,
    }

    impl Scan {
        pub fn open(out: &Path) -> Scan {
            register();
            let connection = super::open(&out.join("sqlitevec.db"));
            connection.pragma_update(None, "query_only", true).unwrap();
            Scan { connection }
        }

        /// vec0 orders by Euclidean distance, which for vectors of length
        /// one is the order of their products: |q − v|² = 2 − 2 q·v.
        pub fn search(&self, q: &[f32], k: usize) -> Vec<Scored> {
            let mut statement = self
                .connection
                .prepare_cached(
                    "select rowid, distance from vec where embedding match ?1 and k = ?2",
                )
                .unwrap();
            statement
                .query_map(params![float_bytes(q), k as i64], |row| {
                    let distance: f64 = row.get(1)?;
                    Ok(Scored {
                        score: (1.0 - distance * distance / 2.0) as f32,
                        doc: row.get::<_, i64>(0)? as u32,
                    })
                })
                .unwrap()
                .map(Result::unwrap)
                .collect()
        }
    }
}

/// SQLite's vec1: IVF over OPQ codes trained by `vec1_train`, its candidates
/// ranked again on their vectors in SQL, as its manual says to.
#[cfg(feature = "vec1")]
pub mod ivf1 {
    use std::path::Path;
    use std::time::Instant;

    use rusqlite::{Connection, params};
    use serde_json::json;

    use super::{DIM, Rows, Scored, float_bytes, threads};

    unsafe extern "C" {
        fn sqlite3_vec1_extra_init(z: *const std::os::raw::c_char) -> std::os::raw::c_int;
    }

    fn register() {
        // SAFETY: built with VEC1_STATIC, vec1 registers itself as an
        // auto-extension and reads nothing from its argument.
        unsafe {
            sqlite3_vec1_extra_init(std::ptr::null());
        }
    }

    pub fn build(
        vectors: &Path,
        out: &Path,
        buckets: usize,
        codesize: usize,
        sample: usize,
    ) -> serde_json::Value {
        register();
        let started = Instant::now();
        let path = out.join("vec1.db");
        let _ = std::fs::remove_file(&path);
        let mut connection = super::open(&path);
        connection
            .execute_batch("create virtual table v using vec1(vector)")
            .unwrap();
        connection
            .query_row(
                "select vec1_config('nthread', ?1)",
                params![threads() as i64],
                |_| Ok(()),
            )
            .unwrap();
        let mut rows = Rows::open(&vectors.join("docs.f32"));
        let count = rows.count;
        let mut block = Vec::new();
        let mut doc = 0i64;
        while rows.next(16_384, &mut block) > 0 {
            let tx = connection.transaction().unwrap();
            {
                let mut insert = tx
                    .prepare_cached("insert into v (rowid, vector) values (?1, ?2)")
                    .unwrap();
                for row in block.chunks_exact(DIM) {
                    doc += 1;
                    insert.execute(params![doc, float_bytes(row)]).unwrap();
                }
            }
            tx.commit().unwrap();
        }
        let inserted = started.elapsed().as_secs_f64();
        let step = (count / sample).max(1) as i64;
        let settings = format!(
            "{{nbucket:{buckets}, codesize:{codesize}, quantizer:\"opq\", distance:\"cos\", residual:0}}"
        );
        let model: Vec<u8> = connection
            .query_row(
                "select vec1_train(vector, ?1) from (select vector from v where rowid % ?2 = 0)",
                params![settings, step],
                |row| row.get(0),
            )
            .unwrap();
        let trained = started.elapsed().as_secs_f64() - inserted;
        connection
            .execute(
                "insert into v (cmd, arg) values ('rebuild', ?1)",
                params![model],
            )
            .unwrap();
        connection
            .pragma_update(None, "wal_checkpoint", "truncate")
            .unwrap();
        let rebuilt = started.elapsed().as_secs_f64() - inserted - trained;
        let objects = crate::fts::objects(&connection);
        drop(connection);
        json!({"step": "vbuild", "backend": "vec1", "documents": doc, "settings": settings, "sample_every": step,
            "seconds": {"insert": inserted, "train": trained, "rebuild": rebuilt, "total": started.elapsed().as_secs_f64()},
            "bytes": std::fs::metadata(&path).unwrap().len(), "objects": objects, "threads": threads()})
    }

    pub struct Ivf {
        connection: Connection,
        settings: String,
    }

    impl Ivf {
        pub fn open(out: &Path, probes: usize, keep: usize) -> Ivf {
            register();
            let connection = super::open(&out.join("vec1.db"));
            connection.pragma_update(None, "query_only", true).unwrap();
            Ivf {
                connection,
                settings: format!("{{k:{keep}, nprobe:{probes}}}"),
            }
        }

        /// vec1_cos_distance is one less the cosine, the product for vectors
        /// of length one.
        pub fn search(&self, q: &[f32], k: usize) -> Vec<Scored> {
            let mut statement = self
                .connection
                .prepare_cached(
                    "select rowid, vec1_cos_distance(?1, vector) as d from v(?1, ?2) order by d limit ?3",
                )
                .unwrap();
            statement
                .query_map(params![float_bytes(q), self.settings, k as i64], |row| {
                    let distance: f64 = row.get(1)?;
                    Ok(Scored {
                        score: (1.0 - distance) as f32,
                        doc: row.get::<_, i64>(0)? as u32,
                    })
                })
                .unwrap()
                .map(Result::unwrap)
                .collect()
        }
    }
}

/// An arm as a run script names it: `exact`, `codes:P:R`, `signs:P:R`,
/// `rabitq:P:R`, `hnsw:EF`, `sqlitevec`, `vec1:P:R`, and `truth`, which
/// answers from truth.json for the hybrid.
pub enum Searcher {
    Exact(Reader),
    Codes(Reader, usize, usize),
    Signs(Reader, usize, usize),
    Rabitq(Reader, usize, usize),
    #[cfg(feature = "usearch")]
    Hnsw(hnsw::Hnsw),
    #[cfg(feature = "sqlitevec")]
    Scan(scan::Scan),
    #[cfg(feature = "vec1")]
    Vec1(ivf1::Ivf),
    Truth(Vec<Vec<Scored>>, usize),
}

impl Searcher {
    pub fn open(vectors: &Path, out: &Path, arm: &str) -> Searcher {
        let parts: Vec<&str> = arm.split(':').collect();
        let number = |at: usize| -> usize { parts[at].parse().unwrap() };
        match parts[0] {
            "exact" => Searcher::Exact(Reader::open(out)),
            "codes" => Searcher::Codes(Reader::open(out), number(1), number(2)),
            "signs" => Searcher::Signs(Reader::open(out), number(1), number(2)),
            "rabitq" => Searcher::Rabitq(Reader::open(out), number(1), number(2)),
            #[cfg(feature = "usearch")]
            "hnsw" => Searcher::Hnsw(hnsw::Hnsw::open(out, number(1))),
            #[cfg(feature = "sqlitevec")]
            "sqlitevec" => Searcher::Scan(scan::Scan::open(out)),
            #[cfg(feature = "vec1")]
            "vec1" => Searcher::Vec1(ivf1::Ivf::open(out, number(1), number(2))),
            "truth" => Searcher::Truth(read_truth(vectors), 0),
            other => panic!("no arm {other}"),
        }
    }

    pub fn search(&mut self, q: &[f32], k: usize) -> Vec<Scored> {
        match self {
            Searcher::Exact(reader) => reader.exact(q, k),
            Searcher::Codes(reader, probes, keep) => reader.codes(q, k, *probes, *keep),
            Searcher::Signs(reader, probes, keep) => reader.signs(q, k, *probes, *keep),
            Searcher::Rabitq(reader, probes, keep) => reader.rabitq(q, k, *probes, *keep),
            #[cfg(feature = "usearch")]
            Searcher::Hnsw(index) => index.search(q, k),
            #[cfg(feature = "sqlitevec")]
            Searcher::Scan(scan) => scan.search(q, k),
            #[cfg(feature = "vec1")]
            Searcher::Vec1(ivf) => ivf.search(q, k),
            Searcher::Truth(answers, next) => {
                let answer = answers[*next].iter().take(k).copied().collect();
                *next = (*next + 1) % answers.len();
                answer
            }
        }
    }

    /// What the search read since the last call, for the arms over vectors.db.
    pub fn take_counts(&mut self) -> Counts {
        match self {
            Searcher::Exact(reader)
            | Searcher::Codes(reader, ..)
            | Searcher::Signs(reader, ..)
            | Searcher::Rabitq(reader, ..) => std::mem::take(&mut reader.counts),
            _ => Counts::default(),
        }
    }
}
