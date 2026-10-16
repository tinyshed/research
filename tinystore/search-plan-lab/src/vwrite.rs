//! Writes into the vectors' lists, and a long mix of them. A set or a
//! delete commits alone: a new member waits in its cluster's head, a
//! replaced or deleted one is marked gone. A cluster is packed in a
//! transaction of its own, its gone members dropped and its head appended,
//! and one grown past twice the mean is split in two by 2-means, its
//! RaBitQ codes made again around the new centres.
//!
//! ```text
//! lab vchurn <vectors> <out> <first> <batches> <inserts> <updates> <deletes> <never|clusters|split> <check every> <seed>
//! ```
//!
//! The index in `<out>` was built from the first `<first>` rows; inserts take
//! the rows after them in order, an update gives a document the vector of a
//! random earlier row. At every checkpoint the 600 questions are asked
//! against the exact answer over what is live then.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

use rusqlite::{Connection, Transaction, params};
use serde_json::json;

use crate::vectors::{
    DIM, PART, PARTS, Reader, Rotation, Rows, Scored, Top, dot, float_bytes, floats, norm,
    quantize, rabitq, threads, word_bytes,
};

const HEAD: &str = "
create table if not exists vhead (doc integer primary key, cluster integer not null, scale real not null,
  error real not null, code blob not null, norm real not null, factor real not null, bits blob not null);
create index if not exists vhead_cluster on vhead (cluster);
create table if not exists vgone (doc integer primary key, cluster integer not null);
";

/// What the writer keeps in memory, apart from its connection.
struct State {
    centroids: Vec<f32>,
    centers: Vec<f32>,
    cos_min: Vec<f64>,
    norm_max: Vec<f64>,
    members: Vec<usize>,
    rotation: Rotation,
    /// Each document's cluster, by its number.
    placed: Vec<u32>,
    /// Gone documents a cluster's rows or head still hold.
    gone_in: HashMap<u32, Vec<u32>>,
    /// Clusters with a head or a gone member, waiting for a pack.
    touched: BTreeSet<u32>,
    /// Clusters whose centroid or cap changed since readers last heard.
    changed: BTreeSet<u32>,
    next: u32,
}

/// A cluster's members, both codes of each, in one order.
#[derive(Default)]
struct Members {
    docs: Vec<u32>,
    scales: Vec<f32>,
    errors: Vec<f32>,
    codes: Vec<u8>,
    norms: Vec<f32>,
    factors: Vec<f32>,
    bits: Vec<u8>,
}

const BITS: usize = DIM / 8;

impl State {
    fn centroid(&self, cluster: u32) -> &[f32] {
        &self.centroids[cluster as usize * DIM..][..DIM]
    }

    fn center(&self, cluster: u32) -> Vec<f32> {
        let scale = self.centers[cluster as usize];
        self.centroid(cluster).iter().map(|c| c * scale).collect()
    }

    fn nearest(&self, v: &[f32]) -> u32 {
        self.centroids
            .chunks_exact(DIM)
            .enumerate()
            .map(|(cluster, c)| Scored {
                score: dot(v, c),
                doc: cluster as u32,
            })
            .max()
            .unwrap()
            .doc
    }

    /// Widens `cluster`'s cap to take `v`, rounded away from the bound's favour.
    fn widen(&mut self, cluster: u32, v: &[f32]) {
        let length = norm(v);
        let projection: f64 = v
            .iter()
            .zip(self.centroid(cluster))
            .map(|(x, c)| f64::from(*x) * f64::from(*c))
            .sum();
        let at = cluster as usize;
        self.cos_min[at] = self.cos_min[at].min(projection / length - 1e-6);
        self.norm_max[at] = self.norm_max[at].max(length + 1e-6);
        self.changed.insert(cluster);
    }

    /// A new document holding `v`, in its nearest cluster's head.
    fn add(&mut self, tx: &Transaction, v: &[f32]) -> u32 {
        let doc = self.next;
        self.next += 1;
        let cluster = self.nearest(v);
        let mut code = Vec::with_capacity(DIM);
        let (scale, error) = quantize(v, &mut code);
        let mut bits = Vec::with_capacity(BITS);
        let (length, factor) = rabitq(v, &self.center(cluster), &self.rotation, &mut bits);
        tx.prepare_cached("insert into vf32 (doc, v) values (?1, ?2)")
            .unwrap()
            .execute(params![doc, float_bytes(v)])
            .unwrap();
        tx.prepare_cached(
            "insert into vhead (doc, cluster, scale, error, code, norm, factor, bits) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .unwrap()
        .execute(params![doc, cluster, scale, error, code, length, factor, bits])
        .unwrap();
        self.widen(cluster, v);
        let at = cluster as usize;
        self.members[at] += 1;
        tx.prepare_cached(
            "update centroids set cos_min = ?2, norm_max = ?3, members = ?4 where cluster = ?1",
        )
        .unwrap()
        .execute(params![
            cluster,
            self.cos_min[at],
            self.norm_max[at],
            self.members[at] as i64
        ])
        .unwrap();
        if self.placed.len() <= doc as usize {
            self.placed.resize(doc as usize + 1, u32::MAX);
        }
        self.placed[doc as usize] = cluster;
        self.touched.insert(cluster);
        doc
    }

    fn remove(&mut self, tx: &Transaction, doc: u32) {
        let cluster = self.placed[doc as usize];
        tx.prepare_cached("insert into vgone (doc, cluster) values (?1, ?2)")
            .unwrap()
            .execute(params![doc, cluster])
            .unwrap();
        self.gone_in.entry(cluster).or_default().push(doc);
        self.touched.insert(cluster);
    }
}

pub struct Writer {
    connection: Connection,
    state: State,
}

/// What packing and splitting cost.
#[derive(Default)]
pub struct Upkeep {
    pub packs: Vec<f64>,
    pub splits: Vec<f64>,
}

impl Writer {
    pub fn open(out: &Path) -> Writer {
        let connection = crate::vectors::open(&out.join("vectors.db"));
        connection.execute_batch(HEAD).unwrap();
        let (mut centroids, mut centers, mut cos_min, mut norm_max, mut members) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        {
            let mut statement = connection
                .prepare("select centroid, center, cos_min, norm_max, members from centroids order by cluster")
                .unwrap();
            let mut rows = statement.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                centroids.extend(floats(row.get_ref(0).unwrap().as_blob().unwrap()));
                centers.push(row.get::<_, f64>(1).unwrap() as f32);
                cos_min.push(row.get(2).unwrap());
                norm_max.push(row.get(3).unwrap());
                members.push(row.get::<_, i64>(4).unwrap() as usize);
            }
        }
        let mut placed = Vec::new();
        {
            let mut statement = connection.prepare("select id, docs from lcodes").unwrap();
            let mut rows = statement.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let cluster = (row.get::<_, i64>(0).unwrap() / PARTS) as u32;
                for doc in row.get_ref(1).unwrap().as_blob().unwrap().chunks_exact(4) {
                    let doc = u32::from_le_bytes(doc.try_into().unwrap()) as usize;
                    if placed.len() <= doc {
                        placed.resize(doc + 1, u32::MAX);
                    }
                    placed[doc] = cluster;
                }
            }
        }
        let next = placed.len() as u32;
        let state = State {
            centroids,
            centers,
            cos_min,
            norm_max,
            members,
            rotation: Rotation::new(),
            placed,
            gone_in: HashMap::new(),
            touched: BTreeSet::new(),
            changed: BTreeSet::new(),
            next,
        };
        Writer { connection, state }
    }

    pub fn insert(&mut self, v: &[f32]) -> u32 {
        let tx = self.connection.transaction().unwrap();
        let doc = self.state.add(&tx, v);
        tx.commit().unwrap();
        doc
    }

    pub fn update(&mut self, doc: u32, v: &[f32]) -> u32 {
        let tx = self.connection.transaction().unwrap();
        self.state.remove(&tx, doc);
        let new = self.state.add(&tx, v);
        tx.commit().unwrap();
        new
    }

    pub fn delete(&mut self, doc: u32) {
        let tx = self.connection.transaction().unwrap();
        self.state.remove(&tx, doc);
        tx.commit().unwrap();
    }

    fn read_members(tx: &Transaction, cluster: u32, gone: &HashSet<u32>) -> Members {
        let first = i64::from(cluster) * PARTS;
        let mut members = Members::default();
        let mut codes = tx
            .prepare_cached("select docs, scales, errors, codes from lcodes where id >= ?1 and id < ?2 order by id")
            .unwrap();
        let mut rabits = tx
            .prepare_cached("select docs, norms, factors, bits from lrabitq where id >= ?1 and id < ?2 order by id")
            .unwrap();
        let mut a = codes.query(params![first, first + PARTS]).unwrap();
        let mut b = rabits.query(params![first, first + PARTS]).unwrap();
        while let (Some(code_row), Some(bit_row)) = (a.next().unwrap(), b.next().unwrap()) {
            let docs = code_row.get_ref(0).unwrap().as_blob().unwrap();
            assert_eq!(
                docs,
                bit_row.get_ref(0).unwrap().as_blob().unwrap(),
                "a cluster's lists in one order"
            );
            let scales = floats(code_row.get_ref(1).unwrap().as_blob().unwrap());
            let errors = floats(code_row.get_ref(2).unwrap().as_blob().unwrap());
            let all_codes = code_row.get_ref(3).unwrap().as_blob().unwrap();
            let norms = floats(bit_row.get_ref(1).unwrap().as_blob().unwrap());
            let factors = floats(bit_row.get_ref(2).unwrap().as_blob().unwrap());
            let all_bits = bit_row.get_ref(3).unwrap().as_blob().unwrap();
            for (at, doc) in docs.chunks_exact(4).enumerate() {
                let doc = u32::from_le_bytes(doc.try_into().unwrap());
                if gone.contains(&doc) {
                    continue;
                }
                members.docs.push(doc);
                members.scales.push(scales[at]);
                members.errors.push(errors[at]);
                members
                    .codes
                    .extend_from_slice(&all_codes[at * DIM..][..DIM]);
                members.norms.push(norms[at]);
                members.factors.push(factors[at]);
                members
                    .bits
                    .extend_from_slice(&all_bits[at * BITS..][..BITS]);
            }
        }
        let mut head = tx
            .prepare_cached("select doc, scale, error, code, norm, factor, bits from vhead where cluster = ?1 order by doc")
            .unwrap();
        let mut rows = head.query(params![cluster]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let doc: u32 = row.get(0).unwrap();
            if gone.contains(&doc) {
                continue;
            }
            members.docs.push(doc);
            members.scales.push(row.get::<_, f64>(1).unwrap() as f32);
            members.errors.push(row.get::<_, f64>(2).unwrap() as f32);
            members
                .codes
                .extend_from_slice(row.get_ref(3).unwrap().as_blob().unwrap());
            members.norms.push(row.get::<_, f64>(4).unwrap() as f32);
            members.factors.push(row.get::<_, f64>(5).unwrap() as f32);
            members
                .bits
                .extend_from_slice(row.get_ref(6).unwrap().as_blob().unwrap());
        }
        members
    }

    fn write_members(tx: &Transaction, cluster: u32, members: &Members) {
        let first = i64::from(cluster) * PARTS;
        for table in ["lcodes", "lrabitq", "lsigns"] {
            tx.execute(
                &format!("delete from {table} where id >= ?1 and id < ?2"),
                params![first, first + PARTS],
            )
            .unwrap();
        }
        for (part, docs) in members.docs.chunks(PART).enumerate() {
            let id = first + part as i64;
            let range = part * PART..part * PART + docs.len();
            tx.prepare_cached(
                "insert into lcodes (id, docs, scales, errors, codes) values (?1, ?2, ?3, ?4, ?5)",
            )
            .unwrap()
            .execute(params![
                id,
                word_bytes(docs),
                float_bytes(&members.scales[range.clone()]),
                float_bytes(&members.errors[range.clone()]),
                &members.codes[range.start * DIM..range.end * DIM]
            ])
            .unwrap();
            tx.prepare_cached(
                "insert into lrabitq (id, docs, norms, factors, bits) values (?1, ?2, ?3, ?4, ?5)",
            )
            .unwrap()
            .execute(params![
                id,
                word_bytes(docs),
                float_bytes(&members.norms[range.clone()]),
                float_bytes(&members.factors[range.clone()]),
                &members.bits[range.start * BITS..range.end * BITS]
            ])
            .unwrap();
        }
    }

    /// Packs one cluster in a transaction of its own: its gone members
    /// dropped with their vectors, its head appended to its lists.
    fn pack(&mut self, cluster: u32, gone: &HashSet<u32>) -> usize {
        let tx = self.connection.transaction().unwrap();
        let members = Writer::read_members(&tx, cluster, gone);
        Writer::write_members(&tx, cluster, &members);
        tx.execute("delete from vhead where cluster = ?1", params![cluster])
            .unwrap();
        for doc in self.state.gone_in.remove(&cluster).unwrap_or_default() {
            tx.execute("delete from vf32 where doc = ?1", params![doc])
                .unwrap();
            tx.execute("delete from vgone where doc = ?1", params![doc])
                .unwrap();
        }
        self.state.members[cluster as usize] = members.docs.len();
        tx.execute(
            "update centroids set members = ?2 where cluster = ?1",
            params![cluster, members.docs.len() as i64],
        )
        .unwrap();
        tx.commit().unwrap();
        members.docs.len()
    }

    /// Splits a packed cluster in two by spherical 2-means over its vectors as
    /// given; both halves' RaBitQ codes are made again around their centres.
    fn split(&mut self, cluster: u32) {
        let tx = self.connection.transaction().unwrap();
        let members = Writer::read_members(&tx, cluster, &HashSet::new());
        let vectors: Vec<Vec<f32>> = {
            let mut statement = tx
                .prepare_cached("select v from vf32 where doc = ?1")
                .unwrap();
            members
                .docs
                .iter()
                .map(|doc| {
                    statement
                        .query_row(params![doc], |row| Ok(floats(row.get_ref(0)?.as_blob()?)))
                        .unwrap()
                })
                .collect()
        };
        let state = &mut self.state;
        let centroid = state.centroid(cluster).to_vec();
        let a = vectors
            .iter()
            .min_by(|x, y| dot(x, &centroid).total_cmp(&dot(y, &centroid)))
            .unwrap()
            .clone();
        let b = vectors
            .iter()
            .min_by(|x, y| dot(x, &a).total_cmp(&dot(y, &a)))
            .unwrap()
            .clone();
        let mut sides = [a, b];
        let mut side_of = vec![0usize; vectors.len()];
        for _ in 0..6 {
            for (side, v) in side_of.iter_mut().zip(&vectors) {
                *side = usize::from(dot(v, &sides[1]) > dot(v, &sides[0]));
            }
            for (which, centre) in sides.iter_mut().enumerate() {
                let mut sum = vec![0f64; DIM];
                for (v, side) in vectors.iter().zip(&side_of) {
                    if *side == which {
                        for (s, x) in sum.iter_mut().zip(v) {
                            *s += f64::from(*x);
                        }
                    }
                }
                let length = sum.iter().map(|x| x * x).sum::<f64>().sqrt();
                if length > 0.0 {
                    *centre = sum.iter().map(|x| (x / length) as f32).collect();
                }
            }
        }
        if side_of.iter().all(|side| *side == side_of[0]) {
            tx.commit().unwrap();
            return;
        }
        let new = state.members.len() as u32;
        state.centroids.extend_from_slice(&sides[1]);
        state.centers.push(0.0);
        state.cos_min.push(1.0);
        state.norm_max.push(0.0);
        state.members.push(0);
        state.centroids[cluster as usize * DIM..][..DIM].copy_from_slice(&sides[0]);
        for (which, id) in [cluster, new].into_iter().enumerate() {
            let at = id as usize;
            let picked: Vec<usize> = (0..vectors.len())
                .filter(|i| side_of[*i] == which)
                .collect();
            let projection: f64 = picked
                .iter()
                .map(|i| {
                    vectors[*i]
                        .iter()
                        .zip(&sides[which])
                        .map(|(x, c)| f64::from(*x) * f64::from(*c))
                        .sum::<f64>()
                })
                .sum();
            state.centers[at] = (projection / picked.len() as f64) as f32;
            state.cos_min[at] = 1.0;
            state.norm_max[at] = 0.0;
            let center = state.center(id);
            let mut half = Members::default();
            for i in &picked {
                let v = &vectors[*i];
                half.docs.push(members.docs[*i]);
                half.scales.push(members.scales[*i]);
                half.errors.push(members.errors[*i]);
                half.codes
                    .extend_from_slice(&members.codes[i * DIM..][..DIM]);
                let (length, factor) = rabitq(v, &center, &state.rotation, &mut half.bits);
                half.norms.push(length);
                half.factors.push(factor);
                state.widen(id, v);
                state.placed[members.docs[*i] as usize] = id;
            }
            state.members[at] = picked.len();
            Writer::write_members(&tx, id, &half);
            tx.execute(
                "insert or replace into centroids (cluster, centroid, center, cos_min, norm_max, members) values (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, float_bytes(state.centroid(id)), f64::from(state.centers[at]), state.cos_min[at], state.norm_max[at], picked.len() as i64],
            )
            .unwrap();
            state.changed.insert(id);
        }
        tx.commit().unwrap();
    }

    /// Packs every touched cluster, and splits each that has grown past
    /// `split_at` once packed, if asked to.
    pub fn upkeep(&mut self, gone: &HashSet<u32>, split_at: Option<usize>, cost: &mut Upkeep) {
        let touched: Vec<u32> = std::mem::take(&mut self.state.touched)
            .into_iter()
            .collect();
        for cluster in touched {
            let started = Instant::now();
            let members = self.pack(cluster, gone);
            cost.packs.push(started.elapsed().as_secs_f64() * 1e3);
            if split_at.is_some_and(|most| members > most) {
                let started = Instant::now();
                self.split(cluster);
                cost.splits.push(started.elapsed().as_secs_f64() * 1e3);
            }
        }
    }

    /// Tells a reader every cluster that changed since it last heard.
    pub fn inform(&mut self, reader: &mut Reader) {
        for cluster in std::mem::take(&mut self.state.changed) {
            let at = cluster as usize;
            reader.set_cluster(
                at,
                self.state.centroid(cluster),
                self.state.centers[at],
                self.state.cos_min[at],
                self.state.norm_max[at],
            );
        }
    }

    pub fn clusters(&self) -> &[usize] {
        &self.state.members
    }

    pub fn head_rows(&self) -> i64 {
        self.connection
            .query_row("select count(*) from vhead", [], |row| row.get(0))
            .unwrap()
    }
}

/// A seeded xorshift, so that every arm makes the same writes.
struct Draw(u64);

impl Draw {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Reads one row of a file embed.py wrote.
struct Source(File);

impl Source {
    fn row(&mut self, row: usize) -> Vec<f32> {
        let mut bytes = vec![0u8; DIM * 4];
        self.0
            .seek(SeekFrom::Start((row * DIM * 4) as u64))
            .unwrap();
        self.0.read_exact(&mut bytes).unwrap();
        floats(&bytes)
    }
}

/// The exact best of every question over what is live: the first `first`
/// rows as documents 1 to `first` unless gone, and each added document's
/// row. Kept by seed and batch, since every arm makes the same writes.
fn live_truth(
    vectors: &Path,
    questions: &[f32],
    first: usize,
    gone: &HashSet<u32>,
    added: &HashMap<u32, Vec<u32>>,
    k: usize,
) -> Vec<Vec<Scored>> {
    let asked = questions.len() / DIM;
    let mut rows = Rows::open(&vectors.join("docs.f32"));
    let mut tops: Vec<Top> = (0..asked).map(|_| Top::new(k)).collect();
    let mut block = Vec::new();
    let mut start = 0usize;
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
                    scope.spawn(move || {
                        let mut tops: Vec<Top> = (0..asked).map(|_| Top::new(k)).collect();
                        for (at, row) in mine.chunks_exact(DIM).enumerate() {
                            let index = start + part * per + at;
                            let mut docs: Vec<u32> = Vec::new();
                            if index < first && !gone.contains(&(index as u32 + 1)) {
                                docs.push(index as u32 + 1);
                            }
                            if let Some(more) = added.get(&(index as u32)) {
                                docs.extend(more.iter().filter(|doc| !gone.contains(doc)));
                            }
                            if docs.is_empty() {
                                continue;
                            }
                            for (question, q) in questions.chunks_exact(DIM).enumerate() {
                                let score = dot(q, row);
                                for doc in &docs {
                                    tops[question].push(Scored { score, doc: *doc });
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
        start += read;
    }
    tops.into_iter().map(Top::sorted).collect()
}

fn percentile(values: &mut [f64], share: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    values[((values.len() as f64 * share).ceil() as usize).clamp(1, values.len()) - 1]
}

fn spread(values: &mut Vec<f64>) -> serde_json::Value {
    let total: f64 = values.iter().sum();
    let line = json!({"count": values.len(), "p50": percentile(values, 0.5), "p99": percentile(values, 0.99),
        "max": values.iter().copied().fold(0.0, f64::max), "total": total});
    values.clear();
    line
}

/// The questions through one search, against the exact answers: latency,
/// recall in all and by language (ru, en, zh, 200 each), members read.
fn ask(
    reader: &mut Reader,
    questions: &[f32],
    truth: &[Vec<Scored>],
    search: &mut dyn FnMut(&mut Reader, &[f32]) -> Vec<Scored>,
) -> serde_json::Value {
    for q in questions.chunks_exact(DIM) {
        search(reader, q);
    }
    reader.counts = Default::default();
    let mut millis = Vec::new();
    let mut found_share = vec![0.0; 3];
    let mut differ = 0;
    for (at, (q, exact)) in questions.chunks_exact(DIM).zip(truth).enumerate() {
        let started = Instant::now();
        let found = search(reader, q);
        millis.push(started.elapsed().as_secs_f64() * 1e3);
        let exact = &exact[..10];
        found_share[at * 3 * DIM / questions.len()] += found
            .iter()
            .filter(|s| exact.iter().any(|e| e.doc == s.doc))
            .count() as f64
            / 10.0;
        let same = found.len() == 10
            && found
                .iter()
                .zip(exact)
                .all(|(a, b)| a.doc == b.doc && a.score.to_bits() == b.score.to_bits());
        differ += usize::from(!same);
    }
    let n = millis.len() as f64;
    let each = n / 3.0;
    json!({"questions": millis.len(), "p50_ms": percentile(&mut millis, 0.5), "p99_ms": percentile(&mut millis, 0.99),
        "recall": found_share.iter().sum::<f64>() / n,
        "recall_ru": found_share[0] / each, "recall_en": found_share[1] / each, "recall_zh": found_share[2] / each,
        "differ": differ, "members": reader.counts.members as f64 / n, "rescored": reader.counts.rescored as f64 / n})
}

pub fn churn(args: &[String]) {
    let [
        vectors,
        out,
        first,
        batches,
        inserts,
        updates,
        deletes,
        mode,
        check_every,
        seed,
    ] = args
    else {
        panic!(
            "vchurn <vectors> <out> <first> <batches> <inserts> <updates> <deletes> <never|clusters|split> <check every> <seed>"
        )
    };
    let (vectors, out) = (Path::new(vectors), Path::new(out));
    let number = |text: &String| -> usize { text.parse().unwrap() };
    let (first, batches, check_every) = (number(first), number(batches), number(check_every));
    let (inserts, updates, deletes) = (number(inserts), number(updates), number(deletes));
    crate::vectors::check();
    let questions = crate::vectors::read_all(&vectors.join("questions.f32"));
    let mut writer = Writer::open(out);
    let mut reader = Reader::open(out);
    let split_at = (mode == "split").then(|| 2 * first / writer.clusters().len());
    let mut source = Source(File::open(vectors.join("docs.f32")).unwrap());
    let mut draw = Draw(number(seed) as u64 * 0x9E37_79B9_7F4A_7C15 | 1);
    let mut live: Vec<u32> = (1..=first as u32).collect();
    let mut added: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut gone: HashSet<u32> = HashSet::new();
    let mut next_row = first;
    let (mut insert_ms, mut update_ms, mut delete_ms) = (Vec::new(), Vec::new(), Vec::new());
    let mut cost = Upkeep::default();
    let context = json!({"mode": mode, "first": first, "inserts": inserts, "updates": updates, "deletes": deletes,
        "seed": seed, "split_at": split_at});
    let cache = vectors.join("churn-truth");
    std::fs::create_dir_all(&cache).unwrap();
    for batch in 0..=batches {
        // A batch's writes, then the checkpoint while its head still waits,
        // then its upkeep.
        if batch > 0 {
            let mut ops: Vec<u8> =
                [vec![0u8; inserts], vec![1u8; updates], vec![2u8; deletes]].concat();
            for at in (1..ops.len()).rev() {
                ops.swap(at, draw.below(at + 1));
            }
            for op in ops {
                match op {
                    0 => {
                        let v = source.row(next_row);
                        let started = Instant::now();
                        let doc = writer.insert(&v);
                        insert_ms.push(started.elapsed().as_secs_f64() * 1e3);
                        added.entry(next_row as u32).or_default().push(doc);
                        live.push(doc);
                        next_row += 1;
                    }
                    1 => {
                        let at = draw.below(live.len());
                        let row = draw.below(first);
                        let v = source.row(row);
                        let started = Instant::now();
                        let doc = writer.update(live[at], &v);
                        update_ms.push(started.elapsed().as_secs_f64() * 1e3);
                        gone.insert(live[at]);
                        reader.gone.insert(live[at]);
                        added.entry(row as u32).or_default().push(doc);
                        live[at] = doc;
                    }
                    _ => {
                        let at = draw.below(live.len());
                        let doc = live.swap_remove(at);
                        let started = Instant::now();
                        writer.delete(doc);
                        delete_ms.push(started.elapsed().as_secs_f64() * 1e3);
                        gone.insert(doc);
                        reader.gone.insert(doc);
                    }
                }
            }
        }
        if batch % check_every == 0 {
            writer.inform(&mut reader);
            reader.gone.clone_from(&gone);
            let cached = cache.join(format!(
                "{seed}-{first}-{inserts}-{updates}-{deletes}-{batch}.json"
            ));
            let started = Instant::now();
            let truth: Vec<Vec<Scored>> = if let Ok(bytes) = std::fs::read(&cached) {
                let answers: Vec<Vec<(u32, f32)>> = serde_json::from_slice(&bytes).unwrap();
                answers
                    .into_iter()
                    .map(|a| {
                        a.into_iter()
                            .map(|(doc, score)| Scored { score, doc })
                            .collect()
                    })
                    .collect()
            } else {
                let truth = live_truth(vectors, &questions, first, &gone, &added, 10);
                let answers: Vec<Vec<(u32, f32)>> = truth
                    .iter()
                    .map(|a| a.iter().map(|s| (s.doc, s.score)).collect())
                    .collect();
                std::fs::write(&cached, serde_json::to_vec(&answers).unwrap()).unwrap();
                truth
            };
            let truth_s = started.elapsed().as_secs_f64();
            let fixed = ask(&mut reader, &questions, &truth, &mut |r, q| {
                r.rabitq(q, 10, 128, 50)
            });
            let budget = ask(&mut reader, &questions, &truth, &mut |r, q| {
                r.rabitq_budget(q, 10, 30_000, 50)
            });
            // Twenty questions of each language, the exact arm being slow.
            let picked: Vec<usize> = (0..3)
                .flat_map(|language| language * 200..language * 200 + 20)
                .collect();
            let some: Vec<f32> = picked
                .iter()
                .flat_map(|i| questions[i * DIM..][..DIM].iter().copied())
                .collect();
            let their: Vec<Vec<Scored>> = picked.iter().map(|i| truth[*i].clone()).collect();
            let exact = ask(&mut reader, &some, &their, &mut |r, q| r.exact(q, 10));
            let mut sizes: Vec<f64> = writer.clusters().iter().map(|m| *m as f64).collect();
            println!(
                "{}",
                json!({"step": "vchurn", "context": context, "batch": batch, "live": live.len(), "gone": gone.len(),
                    "head_rows": writer.head_rows(), "clusters": sizes.len(),
                    "members_p99": percentile(&mut sizes, 0.99), "members_max": sizes.iter().copied().fold(0.0, f64::max),
                    "insert_ms": spread(&mut insert_ms), "update_ms": spread(&mut update_ms), "delete_ms": spread(&mut delete_ms),
                    "pack_ms": spread(&mut cost.packs), "split_ms": spread(&mut cost.splits), "truth_s": truth_s,
                    "fixed": fixed, "budget": budget, "exact": exact,
                    "bytes": std::fs::metadata(out.join("vectors.db")).unwrap().len()})
            );
        }
        if batch > 0 && mode != "never" {
            writer.upkeep(&gone, split_at, &mut cost);
            writer.inform(&mut reader);
        }
    }
}
