//! Round 1s: round 1p's widened key model `both` read in Rust.
//!
//! The candidates and features are those of round 1p's `widen.py` with both
//! switches on, its letter trigrams sorted as round 1s's `letters.py` sorts
//! them; the hashing is scikit-learn's `FeatureHasher`; the graph stage's
//! nineteen features are round 1b's `graph.py`'s, computed in the same
//! precision as NumPy computes them there; the trees are read from
//! `keys.bin` as round 1p's `pack.py` writes it. A comment names the Python
//! each part follows.

use serde::Deserialize;
use std::collections::HashMap;

pub mod cues;
pub mod fast;
pub mod six;

pub const HASHED: usize = 1 << 15;
const CONFIDENT: f32 = 0.5;
const LEAF: u16 = 0x8000;
const KEY_WORDS: [&str; 8] = ["id", "key", "code", "no", "num", "nr", "fk", "ref"];

// --- the schema, as a line of hidden.jsonl ----------------------------------

#[derive(Deserialize)]
pub struct Schema {
    pub id: String,
    #[serde(default)]
    pub split: String,
    pub tables: Vec<Table>,
    #[serde(default)]
    pub fk: Vec<Vec<String>>,
}

#[derive(Deserialize)]
pub struct Table {
    pub name: String,
    #[serde(default)]
    pub pk: Vec<String>,
    pub columns: Vec<Column>,
}

#[derive(Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(default)]
    pub family: String,
    #[serde(default, rename = "type")]
    pub ty: Option<String>,
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub unique: Option<bool>,
    #[serde(default)]
    pub primary: Option<bool>,
    #[serde(default)]
    pub index: Option<bool>,
    #[serde(default)]
    pub position: i64,
}

impl Column {
    fn unique(&self) -> bool {
        self.unique.unwrap_or(false)
    }
    fn primary(&self) -> bool {
        self.primary.unwrap_or(false)
    }
}

/// train.py's KIND: the families a key may have.
fn kind(family: &str) -> Option<&'static str> {
    match family {
        "int" => Some("int"),
        "uuid" => Some("uuid"),
        "text" => Some("text"),
        _ => None,
    }
}

// --- names, as train.py and keys.py read them --------------------------------

// Python's str predicates; outside ASCII, Rust's Unicode properties, which
// differ from Python's categories on a few marks and numerals.
fn is_alpha(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_alphabetic() } else { c.is_alphabetic() }
}
fn is_upper(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_uppercase() } else { c.is_uppercase() }
}
fn is_lower(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_lowercase() } else { c.is_lowercase() }
}
fn is_alnum(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_alphanumeric() } else { c.is_alphanumeric() }
}

/// train.py's `tokens`, as round 1's names.rs splits a name.
pub fn tokens(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !is_alpha(c) {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        if is_upper(c) && !current.is_empty() {
            let previous = chars[i - 1];
            let next_lower = i + 1 < chars.len() && is_lower(chars[i + 1]);
            if is_lower(previous) || (is_upper(previous) && next_lower) {
                out.push(std::mem::take(&mut current));
            }
        }
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// train.py's `norm`.
pub fn norm(name: &str) -> String {
    name.chars().filter(|&c| is_alnum(c)).flat_map(|c| c.to_lowercase()).collect()
}

/// train.py's `singular`.
pub fn singular(word: &str) -> String {
    if let Some(s) = word.strip_suffix("ies") {
        format!("{s}y")
    } else if word.ends_with("ses") || word.ends_with("xes") {
        word[..word.len() - 2].to_string()
    } else if let Some(s) = word.strip_suffix('s') {
        s.to_string()
    } else {
        word.to_string()
    }
}

/// keys.py's `grams`, as a sorted set.
fn grams(s: &str) -> Vec<String> {
    let chars: Vec<char> = std::iter::once('^').chain(s.chars()).chain(std::iter::once('$')).collect();
    let mut out: Vec<String> = chars.windows(3).map(|w| w.iter().collect()).collect();
    out.sort();
    out.dedup();
    out
}

fn union(a: &[String], b: &[String]) -> Vec<String> {
    let mut out: Vec<String> = a.iter().chain(b).cloned().collect();
    out.sort();
    out.dedup();
    out
}

/// keys.py's `jaccard` on two sorted sets.
fn jaccard(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let (mut i, mut j, mut both) = (0, 0, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                both += 1;
                i += 1;
                j += 1;
            }
        }
    }
    both as f64 / (a.len() + b.len() - both) as f64
}

/// keys.py's `similarity`: 1 less the edit distance over the longer length.
fn similarity(a: &[char], b: &[char]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let v = (previous[j + 1] + 1).min(current[j] + 1).min(previous[j] + usize::from(ca != cb));
            current.push(v);
        }
        previous = current;
    }
    1.0 - *previous.last().unwrap() as f64 / a.len().max(b.len()) as f64
}

fn prefix(a: &[char], b: &[char]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// keys.py's `stem_of`: a name less a trailing key word, and its tokens.
fn stem_of(name: &str) -> (String, Vec<String>) {
    let tk = tokens(name);
    let st = if tk.len() > 1 && KEY_WORDS.contains(&tk[tk.len() - 1].as_str()) { &tk[..tk.len() - 1] } else { &tk[..] };
    (st.concat(), tk)
}

// --- hashing, as scikit-learn's FeatureHasher --------------------------------

/// MurmurHash3 x86 32-bit, as scikit-learn's `murmurhash3_32`, signed.
pub fn murmur3(data: &[u8], seed: u32) -> i32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mut h = seed;
    let chunks = data.chunks_exact(4);
    let tail = chunks.remainder();
    for c in chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    let mut k = 0u32;
    for (i, &b) in tail.iter().enumerate() {
        k |= (b as u32) << (8 * i);
    }
    if !tail.is_empty() {
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h as i32
}

/// A feature's column in the hashed space, as `_hashing_fast.transform`.
pub fn index_of(name: &str) -> u16 {
    index_of_bytes(name.as_bytes())
}

pub fn index_of_bytes(name: &[u8]) -> u16 {
    let h = murmur3(name, 0);
    let n = HASHED as i64;
    let i = if h == i32::MIN { (2_147_483_647 - (n - 1)) % n } else { (h as i64).abs() % n };
    i as u16
}

/// Features in the order a Python dict holds them: a later value of a name
/// replaces the earlier one in its place.
#[derive(Default)]
pub struct Named {
    pub items: Vec<(String, f64)>,
}

impl Named {
    pub fn set(&mut self, name: String, v: f64) {
        if let Some(e) = self.items.iter_mut().find(|e| e.0 == name) {
            e.1 = v;
        } else {
            self.items.push((name, v));
        }
    }
}

/// Hashed (index, value) of a group of named features, in insertion order,
/// zero values dropped as FeatureHasher drops them.
fn hashed(named: &Named) -> Vec<(u16, f64)> {
    named.items.iter().filter(|(_, v)| *v != 0.0).map(|(n, v)| (index_of(n), *v)).collect()
}

/// A pair's sparse row: values summed by index in f64, then made f32, as
/// FeatureHasher's matrix `astype(np.float32)`; sorted by index.
fn row(parts: &[&[(u16, f64)]]) -> Vec<(u16, f32)> {
    let mut all: Vec<(u16, f64)> = parts.iter().flat_map(|p| p.iter().copied()).collect();
    all.sort_by_key(|e| e.0); // stable: values of one index keep their order
    let mut out: Vec<(u16, f32)> = Vec::with_capacity(all.len());
    let mut i = 0;
    while i < all.len() {
        let mut sum = all[i].1;
        let mut j = i + 1;
        while j < all.len() && all[j].0 == all[i].0 {
            sum += all[j].1;
            j += 1;
        }
        out.push((all[i].0, sum as f32));
        i = j;
    }
    out
}

// --- candidates and features, as widen.py's `pairs` with both switches -------

struct Target<'a> {
    table: usize,
    kind: &'static str,
    name: String,
    singular: String,
    singular_chars: Vec<char>,
    grams: Vec<String>,
    pk: String, // the key's name, normalised
    pk_type: Option<&'a str>,
    key: &'a str,
    pseudo: bool,
    hashed: Vec<(u16, f64)>, // g:<token>
    last_tokens: Vec<String>,
    tokens: Vec<String>,
    columns: usize,
}

/// widen.py's `key_of` with pseudo keys: a target's key column and whether
/// it is a pseudo key.
fn key_of(t: &Table) -> Option<(&Column, bool)> {
    if t.pk.len() == 1 {
        let c = t.columns.iter().find(|c| c.name == t.pk[0])?;
        return kind(&c.family).map(|_| (c, false));
    }
    if !t.pk.is_empty() {
        return None;
    }
    let unique: Vec<&Column> = t.columns.iter().filter(|c| c.unique() && kind(&c.family).is_some()).collect();
    if unique.len() == 1 {
        return Some((unique[0], true));
    }
    let g = norm(&t.name);
    let names = ["id".to_string(), format!("{g}id"), format!("{}id", singular(&g))];
    t.columns.iter().find(|c| names.contains(&norm(&c.name)) && kind(&c.family).is_some()).map(|c| (c, true))
}

/// One candidate pair: what the graph stage keeps of it, and its features.
pub struct Pair {
    pub table: usize,
    pub column: usize,
    pub target: usize,
    pub base: f64,
    pub same_type: f64,
    pub named_key: f64,
    pub ends_id: f64,
    pub bare: f64,
    pub features: Vec<(u16, f32)>,
}

fn py_bool(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "True",
        Some(false) => "False",
        None => "None",
    }
}

fn first_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Every candidate pair of a schema, in widen.py's order, and optionally the
/// named features of each: the reference the fast `pairs` is checked
/// against, feature by feature by name against Python's, slow.
pub fn pairs_named(s: &Schema, keep_names: bool, with_cues: bool) -> (Vec<Pair>, Vec<Named>) {
    let n_tables = s.tables.len();
    let cue_info = if with_cues { Some(cues::info(s)) } else { None };
    // the targets
    let mut targets: Vec<Target> = Vec::new();
    for (ti, t) in s.tables.iter().enumerate() {
        let Some((key, pseudo)) = key_of(t) else { continue };
        let g = norm(&t.name);
        let sg = singular(&g);
        let gt = tokens(&t.name);
        let mut named = Named::default();
        for tok in &gt {
            named.set(format!("g:{tok}"), 1.0);
        }
        targets.push(Target {
            table: ti,
            kind: kind(&key.family).unwrap(),
            grams: union(&grams(&g), &grams(&sg)),
            singular_chars: sg.chars().collect(),
            name: g,
            singular: sg,
            pk: norm(&key.name),
            pk_type: key.ty.as_deref(),
            key: &key.name,
            pseudo,
            hashed: hashed(&named),
            last_tokens: gt[gt.len().saturating_sub(2)..].to_vec(),
            tokens: gt,
            columns: t.columns.len(),
        });
    }
    // what the schema says of every column
    let stems: Vec<String> = s.tables.iter().flat_map(|t| t.columns.iter().map(|c| stem_of(&c.name).0)).collect();
    let named_by: Vec<usize> = targets
        .iter()
        .map(|g| stems.iter().filter(|st| !st.is_empty() && (**st == g.name || **st == g.singular)).count())
        .collect();
    let all_tokens: Vec<Vec<String>> = s.tables.iter().flat_map(|t| t.columns.iter().map(|c| tokens(&c.name))).collect();
    let id_convention = all_tokens.iter().filter(|tk| tk.last().is_some_and(|l| l == "id")).count() as f64
        / all_tokens.len().max(1) as f64;

    let mut out = Vec::new();
    let mut names_out = Vec::new();
    for (ti, table) in s.tables.iter().enumerate() {
        let table_tokens = tokens(&table.name);
        for (ci, column) in table.columns.iter().enumerate() {
            let Some(ck) = kind(&column.family) else { continue };
            let own = column.primary() && table.pk.len() == 1;
            let (stem, tk) = stem_of(&column.name);
            let stem_chars: Vec<char> = stem.chars().collect();
            let sg = if stem.is_empty() { Vec::new() } else { grams(&stem) };
            let mine: Vec<usize> = (0..targets.len())
                .filter(|&k| targets[k].kind == ck && !(targets[k].table == ti && column.name == targets[k].key))
                .collect();
            if mine.is_empty() {
                continue;
            }
            let cname = norm(&column.name);
            let base: Vec<f64> = mine
                .iter()
                .map(|&k| {
                    let g = &targets[k];
                    let exact = stem == g.name || stem == g.singular || format!("{stem}s") == g.name;
                    let p = prefix(&stem_chars, &g.singular_chars);
                    let pre = if p >= 3 { 0.8 * p as f64 / 1usize.max(stem_chars.len().min(g.singular_chars.len())) as f64 } else { 0.0 };
                    f64::from(u8::from(exact)).max(jaccard(&sg, &g.grams)).max(f64::from(u8::from(cname == g.pk))).max(pre)
                })
                .collect();
            let best = base.iter().copied().fold(f64::NEG_INFINITY, f64::max);

            // train.py's column_features, the same for every target
            let mut col = Named::default();
            for t in &tk {
                col.set(format!("c:{t}"), 1.0);
            }
            if let (Some(first), Some(last)) = (tk.first(), tk.last()) {
                col.set(format!("last:{last}"), 1.0);
                col.set(format!("first:{first}"), 1.0);
            }
            for t in &table_tokens {
                col.set(format!("t:{t}"), 1.0);
            }
            col.set(format!("family:{}", column.family), 1.0);
            col.set(format!("nullable:{}", py_bool(column.nullable)), 1.0);
            if column.unique() {
                col.set("unique".into(), 1.0);
            }
            if column.primary() {
                col.set(if table.pk.len() == 1 { "pk_single" } else { "pk_part" }.into(), 1.0);
            }
            if column.index.unwrap_or(false) {
                col.set("index".into(), 1.0);
            }
            col.set("tokens".into(), tk.len().min(6) as f64 / 6.0);
            col.set("position".into(), column.position.min(30) as f64 / 30.0);
            col.set("columns".into(), table.columns.len().min(60) as f64 / 60.0);
            col.set("tables".into(), n_tables.min(100) as f64 / 100.0);
            let col_hashed = hashed(&col);
            // the trigrams, sorted, twelve at most
            let mut letters = Named::default();
            for gram in sg.iter().take(12) {
                letters.set(format!("cg:{gram}"), 1.0);
            }
            let letters_hashed = hashed(&letters);
            // pair_features' stem: the name less a trailing `id` only
            let stem_tokens: &[String] = if tk.last().is_some_and(|l| l == "id") { &tk[..tk.len() - 1] } else { &tk[..] };
            let stem2 = stem_tokens.concat();
            let stem_set: std::collections::BTreeSet<&str> = stem_tokens.iter().map(String::as_str).collect();
            let last_stem = &stem_tokens[stem_tokens.len().saturating_sub(2)..];
            let ends_id = f64::from(u8::from(tk.last().is_some_and(|l| l == "id")));
            let key_word = tk.last().cloned().unwrap_or_default();

            for (j, &k) in mine.iter().enumerate() {
                let g = &targets[k];
                let b = base[j];
                let others: Vec<f64> = base.iter().enumerate().filter(|(o, _)| *o != j).map(|(_, &x)| x).collect();
                let max_others = if others.is_empty() { 0.0 } else { others.iter().copied().fold(f64::NEG_INFINITY, f64::max) };
                let mut p = Named::default();
                // the rest of train.py's pair_features
                for a in last_stem {
                    for bb in &g.last_tokens {
                        p.set(format!("x:{a}|{bb}"), 1.0);
                    }
                }
                let sgl = singular(&g.name);
                p.set("stem=target".into(), f64::from(u8::from(stem2 == g.name)));
                p.set("stem=target singular".into(), f64::from(u8::from(stem2 == sgl)));
                p.set("stem in target".into(), f64::from(u8::from(!stem2.is_empty() && g.name.contains(&stem2))));
                p.set("target in stem".into(), f64::from(u8::from(!sgl.is_empty() && stem2.contains(&sgl))));
                // the view's key: the primary key, or the pseudo key
                p.set("named as target's key".into(), f64::from(u8::from(cname == g.pk)));
                let shared = stem_set.iter().filter(|t| **t != "id" && g.tokens.iter().any(|x| x == *t)).count();
                p.set("shared tokens".into(), shared as f64 / 3.0);
                p.set("ends in id".into(), ends_id);
                p.set("self".into(), f64::from(u8::from(g.table == ti)));
                p.set("target columns".into(), g.columns.min(60) as f64 / 60.0);
                // widen.py's
                p.set("v2 letters".into(), jaccard(&sg, &g.grams));
                p.set("v2 edit".into(), similarity(&stem_chars, &g.singular_chars));
                p.set("v2 prefix".into(), prefix(&stem_chars, &g.singular_chars).min(8) as f64 / 8.0);
                p.set("v2 base".into(), b);
                p.set("v2 best of the column".into(), f64::from(u8::from(b >= best)));
                p.set("v2 margin".into(), b - max_others);
                p.set("v2 close rivals".into(), others.iter().filter(|&&x| x >= 0.5).count().min(5) as f64 / 5.0);
                p.set("v2 named as the key".into(), f64::from(u8::from(cname == g.pk)));
                p.set("v2 same type".into(), f64::from(u8::from(column.ty.as_deref() == g.pk_type)));
                p.set("v2 named by".into(), named_by[k].min(10) as f64 / 10.0);
                p.set("v2 id convention".into(), id_convention);
                p.set("v2 bare".into(), f64::from(u8::from(stem.is_empty())));
                p.set(format!("v2 key word:{key_word}"), 1.0);
                let pair_hashed_head = hashed(&p);
                let mut tail = Named::default();
                tail.set(format!("p3:{}|{}", first_chars(&stem, 3), first_chars(&g.singular, 3)), 1.0);
                tail.set("v3 own key".into(), f64::from(u8::from(own)));
                tail.set("v3 pseudo target".into(), f64::from(u8::from(g.pseudo)));
                let tail_hashed = hashed(&tail);
                // round 1t's version 5, after every other feature
                let mut five = Named::default();
                if let Some(info) = &cue_info {
                    let v = cues::values(info, column, &s.tables[g.table].name, g.key);
                    for (n, x) in cues::NAMES.iter().zip(v) {
                        five.set((*n).into(), x);
                    }
                }
                let five_hashed = hashed(&five);
                out.push(Pair {
                    table: ti,
                    column: ci,
                    target: g.table,
                    base: b,
                    same_type: f64::from(u8::from(column.ty.as_deref() == g.pk_type)),
                    named_key: f64::from(u8::from(cname == g.pk)),
                    ends_id,
                    bare: f64::from(u8::from(stem.is_empty())),
                    features: row(&[&col_hashed, &g.hashed, &pair_hashed_head, &letters_hashed, &tail_hashed, &five_hashed]),
                });
                if keep_names {
                    let mut all = Named::default();
                    for part in [&col, &p, &letters, &tail, &five] {
                        for (n, v) in &part.items {
                            all.set(n.clone(), *v);
                        }
                    }
                    for tok in &g.tokens {
                        all.set(format!("g:{tok}"), 1.0);
                    }
                    names_out.push(all);
                }
            }
        }
    }
    (out, names_out)
}

// --- the trees, as pack.py writes them ---------------------------------------

#[derive(Clone, Copy)]
struct Node {
    feature: u16,
    threshold: f32,
    left: u16,
    right: u16,
    flags: u8,
}

struct Tree {
    nodes: Vec<Node>,
    leaves: Vec<f32>,
}

struct Stage {
    base: f32,
    trees: Vec<Tree>,
}

pub struct Model {
    first: Stage,
    second: Stage,
    pub bytes: usize,
    /// round 1t: a keys.bin of version 2 reads version 5's cues too
    pub cues: bool,
    /// round 1t: version 3 reads six.rs's candidates; its flags say which
    /// cues (1 version 7, 2 version 8, 8 version 9) and whether the graph stage reads its
    /// schema relatively (4)
    pub version: u8,
    pub flags: u8,
}

impl Model {
    pub fn read(data: &[u8]) -> Result<Model, String> {
        if data.len() < 5 || &data[..4] != b"KEYS" || !matches!(data[4], 1 | 2 | 3) {
            return Err("not a keys.bin of version 1, 2 or 3".into());
        }
        let version = data[4];
        let flags = if version == 3 { *data.get(5).ok_or("short file")? } else { 0 };
        let mut at = if version == 3 { 6 } else { 5 };
        let u16_at = |at: &mut usize| -> Result<u16, String> {
            let b = data.get(*at..*at + 2).ok_or("short file")?;
            *at += 2;
            Ok(u16::from_le_bytes([b[0], b[1]]))
        };
        let f32_at = |at: &mut usize| -> Result<f32, String> {
            let b = data.get(*at..*at + 4).ok_or("short file")?;
            *at += 4;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let mut stages = Vec::new();
        for _ in 0..2 {
            let n = u16_at(&mut at)?;
            let base = f32_at(&mut at)?;
            let mut trees = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let ni = u16_at(&mut at)?;
                let nl = u16_at(&mut at)?;
                let mut nodes = Vec::with_capacity(ni as usize);
                for _ in 0..ni {
                    let feature = u16_at(&mut at)?;
                    let threshold = f32_at(&mut at)?;
                    let left = u16_at(&mut at)?;
                    let right = u16_at(&mut at)?;
                    let flags = *data.get(at).ok_or("short file")?;
                    at += 1;
                    nodes.push(Node { feature, threshold, left, right, flags });
                }
                let mut leaves = Vec::with_capacity(nl as usize);
                for _ in 0..nl {
                    leaves.push(f32_at(&mut at)?);
                }
                trees.push(Tree { nodes, leaves });
            }
            stages.push(Stage { base, trees });
        }
        if at != data.len() {
            return Err("trailing bytes".into());
        }
        let second = stages.pop().unwrap();
        let first = stages.pop().unwrap();
        Ok(Model { first, second, bytes: data.len(), cues: data[4] == 2, version, flags })
    }
}

/// A dense row of the first stage's features with the pair's present ones
/// marked by a generation, so no row is cleared between pairs.
pub struct Scratch {
    values: Vec<f32>,
    stamp: Vec<u32>,
    generation: u32,
}

impl Default for Scratch {
    fn default() -> Self {
        Scratch { values: vec![0.0; HASHED], stamp: vec![0; HASHED], generation: 0 }
    }
}

/// XGBoost's margin: its base margin, each tree's leaf added in f32 in tree
/// order; a feature absent from the sparse row is missing and goes the
/// node's default way, a present one goes left below the threshold.
fn first_margin(stage: &Stage, row: &[(u16, f32)], scratch: &mut Scratch) -> f32 {
    scratch.generation = scratch.generation.wrapping_add(1);
    if scratch.generation == 0 {
        scratch.stamp.iter_mut().for_each(|s| *s = 0);
        scratch.generation = 1;
    }
    let generation = scratch.generation;
    for &(i, v) in row {
        scratch.values[i as usize] = v;
        scratch.stamp[i as usize] = generation;
    }
    let mut m = stage.base;
    for t in &stage.trees {
        if t.nodes.is_empty() {
            m += t.leaves[0];
            continue;
        }
        let mut n = 0usize;
        loop {
            let node = t.nodes[n];
            let f = node.feature as usize;
            let go_left = if scratch.stamp[f] == generation { scratch.values[f] < node.threshold } else { node.flags & 1 != 0 };
            let next = if go_left { node.left } else { node.right };
            if next & LEAF != 0 {
                m += t.leaves[(next & !LEAF) as usize];
                break;
            }
            n = next as usize;
        }
    }
    m
}

/// LightGBM's raw score over dense f32 features: at or below a threshold
/// goes left; a missing value (by its node's kind: none, zero, NaN) goes the
/// node's default way; leaves summed in f64.
fn second_margin(stage: &Stage, x: &[f32]) -> f64 {
    let mut m = stage.base as f64;
    for t in &stage.trees {
        if t.nodes.is_empty() {
            m += t.leaves[0] as f64;
            continue;
        }
        let mut n = 0usize;
        loop {
            let node = t.nodes[n];
            let v = x[node.feature as usize];
            let missing = match node.flags >> 1 {
                2 => v.abs() <= 1e-35,
                3 => v.is_nan(),
                _ => false,
            };
            let go_left = if missing { node.flags & 1 != 0 } else { v <= node.threshold };
            let next = if go_left { node.left } else { node.right };
            if next & LEAF != 0 {
                m += t.leaves[(next & !LEAF) as usize] as f64;
                break;
            }
            n = next as usize;
        }
    }
    m
}

fn sigmoid32(m: f32) -> f32 {
    1.0 / (1.0 + (-m).exp())
}

/// graph.py's `graph_features` for one schema's pairs, in NumPy's
/// precision there: the first stage's scores are f32, and sums of them
/// started from Python's 0.0 stay f32 under NumPy 2's promotion.
fn graph_features(s: &Schema, pairs: &[Pair], scores: &[f32]) -> Vec<[f32; 19]> {
    let name = |t: usize| s.tables[t].name.as_str();
    let col = |p: &Pair| (name(p.table), s.tables[p.table].columns[p.column].name.as_str());
    let mut by_column: HashMap<(&str, &str), Vec<f32>> = HashMap::new();
    let mut order: Vec<(&str, &str)> = Vec::new();
    let mut back_to: HashMap<(&str, &str), f32> = HashMap::new();
    let mut into_sum: HashMap<&str, f32> = HashMap::new();
    let mut into_confident: HashMap<&str, i64> = HashMap::new();
    let mut into_top: HashMap<&str, Vec<f32>> = HashMap::new();
    let mut sibling_top: HashMap<(&str, &str), Vec<f32>> = HashMap::new();
    let keep_two = |v: &mut Vec<f32>, sc: f32| {
        v.push(sc);
        if v.len() > 2 {
            v.sort_by(|a, b| b.partial_cmp(a).unwrap());
            v.truncate(2);
        }
    };
    for (p, &sc) in pairs.iter().zip(scores) {
        let key = col(p);
        let e = by_column.entry(key).or_insert_with(|| {
            order.push(key);
            Vec::new()
        });
        e.push(sc);
        let tg = name(p.target);
        let b = back_to.entry((key.0, tg)).or_insert(0.0);
        if sc > *b {
            *b = sc;
        }
        *into_sum.entry(tg).or_insert(0.0) += sc;
        *into_confident.entry(tg).or_insert(0) += i64::from(sc >= CONFIDENT);
        keep_two(into_top.entry(tg).or_default(), sc);
        keep_two(sibling_top.entry((key.0, tg)).or_default(), sc);
    }
    let mine_of: HashMap<(&str, &str), Vec<f32>> = by_column
        .iter()
        .map(|(k, v)| {
            let mut v = v.clone();
            v.sort_by(|a, b| b.partial_cmp(a).unwrap());
            (*k, v)
        })
        .collect();
    let confident: std::collections::HashSet<(&str, &str)> =
        mine_of.iter().filter(|(_, v)| v[0] >= CONFIDENT).map(|(k, _)| *k).collect();
    let mut confident_in_table: HashMap<&str, i64> = HashMap::new();
    for k in &confident {
        *confident_in_table.entry(k.0).or_insert(0) += 1;
    }
    let last_word: HashMap<(&str, &str), String> =
        order.iter().map(|k| (*k, tokens(k.1).pop().unwrap_or_default())).collect();
    let mut word_sum: HashMap<&str, f32> = HashMap::new();
    let mut word_count: HashMap<&str, i64> = HashMap::new();
    for k in &order {
        let w = last_word[k].as_str();
        *word_sum.entry(w).or_insert(0.0) += mine_of[k][0];
        *word_count.entry(w).or_insert(0) += 1;
    }
    let without = |top: &Vec<f32>, sc: f32| -> f32 {
        let mut t = top.clone();
        t.sort_by(|a, b| b.partial_cmp(a).unwrap());
        if !t.is_empty() && t[0] == sc {
            if t.len() > 1 { t[1] } else { 0.0 }
        } else if !t.is_empty() {
            t[0]
        } else {
            0.0
        }
    };
    let columns = by_column.len().max(1) as f64;
    let mut out = Vec::with_capacity(pairs.len());
    for (p, &sc) in pairs.iter().zip(scores) {
        let key = col(p);
        let tg = name(p.target);
        let mine = &mine_of[&key];
        let rank = 1 + mine.iter().filter(|&&x| x > sc).count();
        let next_best = if rank == 1 && mine.len() > 1 { mine[1] } else if rank > 1 { mine[0] } else { 0.0 };
        let w = last_word[&key].as_str();
        let mates = word_count[w] - 1;
        let is_confident = i64::from(confident.contains(&key));
        let is_self = p.table == p.target;
        out.push([
            sc,
            rank as f32,
            sc - next_best,
            mine[0],
            mine.iter().filter(|&&x| x >= CONFIDENT).count() as f32,
            into_sum[tg] - sc,
            without(&into_top[tg], sc),
            (into_confident[tg] - i64::from(sc >= CONFIDENT)) as f32,
            without(&sibling_top[&(key.0, tg)], sc),
            (confident_in_table.get(key.0).copied().unwrap_or(0) - is_confident) as f32,
            ((confident.len() as i64 - is_confident) as f64 / columns) as f32,
            f32::from(u8::from(is_self)),
            if is_self { 0.0 } else { back_to.get(&(tg, key.0)).copied().unwrap_or(0.0) },
            if mates != 0 { (word_sum[w] - mine[0]) / mates as f32 } else { -1.0 },
            p.base as f32,
            p.same_type as f32,
            p.named_key as f32,
            p.ends_id as f32,
            p.bare as f32,
        ]);
    }
    out
}

/// What the reader says of a schema: each pair's two margins, and the keys
/// it draws.
pub struct Scored {
    pub pairs: Vec<Pair>,
    pub first: Vec<f32>,
    pub second: Vec<f64>,
    pub drawn: Vec<usize>, // indices into pairs
}

impl Model {
    /// The reader: a schema's candidates, both stages, one target a column.
    pub fn score(&self, s: &Schema, threshold: f64, scratch: &mut Scratch) -> Scored {
        if self.version == 3 {
            return self.score_six(s, threshold, scratch).0;
        }
        let pairs = fast::pairs(s, self.cues);
        let first: Vec<f32> = pairs.iter().map(|p| first_margin(&self.first, &p.features, scratch)).collect();
        let probs: Vec<f32> = first.iter().map(|&m| sigmoid32(m)).collect();
        let (g, ck) = fast::graph(s, &pairs, &probs);
        let second: Vec<f64> = g.iter().map(|x| second_margin(&self.second, x)).collect();
        // keys.py's one_target: each column's best target, ties to the first
        let prob = |i: usize| 1.0 / (1.0 + (-second[i]).exp());
        let mut best: Vec<Option<usize>> = vec![None; ck.iter().max().map_or(0, |m| m + 1)];
        for i in 0..pairs.len() {
            match best[ck[i]] {
                Some(j) if prob(i) <= prob(j) => {}
                _ => best[ck[i]] = Some(i),
            }
        }
        let mut drawn: Vec<usize> = best.into_iter().flatten().filter(|&i| prob(i) >= threshold).collect();
        drawn.sort_unstable();
        Scored { pairs, first, second, drawn }
    }

    /// Seconds spent in each part of `score`, for finding where time goes.
    pub fn profile(&self, s: &Schema, scratch: &mut Scratch) -> [f64; 4] {
        let t = std::time::Instant::now();
        let pairs = fast::pairs(s, self.cues);
        let a = t.elapsed().as_secs_f64();
        let first: Vec<f32> = pairs.iter().map(|p| first_margin(&self.first, &p.features, scratch)).collect();
        let b = t.elapsed().as_secs_f64();
        let probs: Vec<f32> = first.iter().map(|&m| sigmoid32(m)).collect();
        let (g, _) = fast::graph(s, &pairs, &probs);
        let c = t.elapsed().as_secs_f64();
        let second: Vec<f64> = g.iter().map(|x| second_margin(&self.second, x)).collect();
        std::hint::black_box(&second);
        let d = t.elapsed().as_secs_f64();
        [a, b - a, c - b, d - c]
    }

    /// Version 3: six.rs's candidates by name, graph.py's 19 features and,
    /// where the flags say, stack.py's six; one target a column.
    pub fn score_six(&self, s: &Schema, threshold: f64, scratch: &mut Scratch) -> (Scored, Vec<[f32; 19]>) {
        let (pairs, _) = six::pairs(s, self.flags & 3 != 0, self.flags & 2 != 0, self.flags & 8 != 0, false);
        let first: Vec<f32> = pairs.iter().map(|p| first_margin(&self.first, &p.features, scratch)).collect();
        let probs: Vec<f32> = first.iter().map(|&m| sigmoid32(m)).collect();
        let g = graph_features(s, &pairs, &probs);
        let second: Vec<f64> = if self.flags & 4 != 0 {
            six::relative(&pairs, &probs, g.clone()).iter().map(|x| second_margin(&self.second, x)).collect()
        } else {
            g.iter().map(|x| second_margin(&self.second, x)).collect()
        };
        let mut best: HashMap<(&str, &str), usize> = HashMap::new();
        let prob = |i: usize| 1.0 / (1.0 + (-second[i]).exp());
        for i in 0..pairs.len() {
            let k = (s.tables[pairs[i].table].name.as_str(), s.tables[pairs[i].table].columns[pairs[i].column].name.as_str());
            match best.get(&k) {
                Some(&j) if prob(i) <= prob(j) => {}
                _ => {
                    best.insert(k, i);
                }
            }
        }
        let mut drawn: Vec<usize> = best.values().copied().filter(|&i| prob(i) >= threshold).collect();
        drawn.sort_unstable();
        (Scored { pairs, first, second, drawn }, g)
    }

    /// The same through the reference path, for `selfcheck`.
    pub fn score_reference(&self, s: &Schema, threshold: f64, scratch: &mut Scratch) -> (Scored, Vec<[f32; 19]>) {
        if self.version == 3 {
            return self.score_six(s, threshold, scratch);
        }
        let (pairs, _) = pairs_named(s, false, self.cues);
        let first: Vec<f32> = pairs.iter().map(|p| first_margin(&self.first, &p.features, scratch)).collect();
        let probs: Vec<f32> = first.iter().map(|&m| sigmoid32(m)).collect();
        let g = graph_features(s, &pairs, &probs);
        let second: Vec<f64> = g.iter().map(|x| second_margin(&self.second, x)).collect();
        let mut best: HashMap<(&str, &str), usize> = HashMap::new();
        let prob = |i: usize| 1.0 / (1.0 + (-second[i]).exp());
        for i in 0..pairs.len() {
            let k = (s.tables[pairs[i].table].name.as_str(), s.tables[pairs[i].table].columns[pairs[i].column].name.as_str());
            match best.get(&k) {
                Some(&j) if prob(i) <= prob(j) => {}
                _ => {
                    best.insert(k, i);
                }
            }
        }
        let mut drawn: Vec<usize> = best.values().copied().filter(|&i| prob(i) >= threshold).collect();
        drawn.sort_unstable();
        (Scored { pairs, first, second, drawn }, g)
    }

    /// The graph stage's features the fast path computes, for `selfcheck`.
    pub fn graph_of(&self, s: &Schema, scored: &Scored) -> Vec<[f32; 19]> {
        let probs: Vec<f32> = scored.first.iter().map(|&m| sigmoid32(m)).collect();
        if self.version == 3 {
            return graph_features(s, &scored.pairs, &probs);
        }
        fast::graph(s, &scored.pairs, &probs).0
    }
}

/// FNV-1a over a pair's sorted indices and f32 values, for parity.
pub fn digest(row: &[(u16, f32)]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &(i, v) in row {
        for b in i.to_le_bytes().into_iter().chain(v.to_bits().to_le_bytes()) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// FNV-1a over a pair's names, for aligning Rust's pairs with Python's.
pub fn name_digest(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0u8)) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}
