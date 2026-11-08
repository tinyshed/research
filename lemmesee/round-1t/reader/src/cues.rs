//! Round 1t's version 5 cues, as `cues.py`'s `schema_info`, `stem5` and
//! `add_cues` compute them: appended after every other feature of a pair, in
//! `add_cues`'s order, a zero dropped as FeatureHasher drops it.

use crate::{norm, tokens, Column, Schema};
use std::collections::HashMap;

pub const NAMES: [&str; 14] = [
    "v5 prefix with key", "v5 prefix with key, of key", "v5 prefix with table", "v5 stems equal",
    "v5 stem starts key", "v5 key stem in column", "v5 shortens table", "v5 shortens key",
    "v5 table word starts stem", "v5 tables carrying the name", "v5 columns ending so", "v5 ends as keys end",
    "v5 last word ends keys", "v5 column end",
];

const KEY_WORDS: [&str; 8] = ["id", "key", "code", "no", "num", "nr", "fk", "ref"];

pub fn index(k: usize) -> u16 {
    static TABLE: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| NAMES.iter().map(|n| crate::index_of(n)).collect())[k]
}

/// What `schema_info` reads off a schema once.
pub struct Info {
    pk_end: Vec<char>,
    col_end: Vec<char>,
    carried: HashMap<String, usize>,
    last: HashMap<String, usize>,
    pk_last: HashMap<String, usize>,
    tables: usize,
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// `common_end`: the longest ending at least `share` of the names carry,
/// each name longer than it; Counter.most_common(1) keeps the first counted
/// of equal counts.
fn common_end(names: &[Vec<char>], share: f64, shortest: usize) -> Vec<char> {
    let names: Vec<&Vec<char>> = names.iter().filter(|n| !n.is_empty()).collect();
    if names.len() < 3 {
        return Vec::new();
    }
    let mut best = Vec::new();
    for k in shortest..12 {
        let mut order: Vec<&[char]> = Vec::new();
        let mut count: HashMap<&[char], usize> = HashMap::new();
        for n in &names {
            if n.len() > k {
                let end = &n[n.len() - k..];
                let e = count.entry(end).or_insert(0);
                if *e == 0 {
                    order.push(end);
                }
                *e += 1;
            }
        }
        if order.is_empty() {
            break;
        }
        let mut top: (&[char], usize) = (order[0], count[order[0]]);
        for &e in &order[1..] {
            if count[e] > top.1 {
                top = (e, count[e]);
            }
        }
        if top.1 as f64 >= share * names.len() as f64 {
            best = top.0.to_vec();
        } else {
            break;
        }
    }
    best
}

fn ends_with(a: &[char], e: &[char]) -> bool {
    a.len() >= e.len() && &a[a.len() - e.len()..] == e
}

fn starts_with(a: &[char], p: &[char]) -> bool {
    a.len() >= p.len() && &a[..p.len()] == p
}

fn contains(a: &[char], b: &[char]) -> bool {
    b.is_empty() || a.windows(b.len()).any(|w| w == b)
}

fn lcp(a: &[char], b: &[char]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

fn last_token(name: &str) -> String {
    tokens(name).pop().unwrap_or_default()
}

pub fn info(s: &Schema) -> Info {
    let pk_names: Vec<Vec<char>> =
        s.tables.iter().filter(|t| t.pk.len() == 1).map(|t| chars(&norm(&t.pk[0]))).collect();
    let cols: Vec<Vec<char>> = s
        .tables
        .iter()
        .flat_map(|t| t.columns.iter().filter(move |c| !(c.primary() && t.pk.len() == 1)).map(|c| chars(&norm(&c.name))))
        .collect();
    let mut carried: HashMap<String, usize> = HashMap::new();
    for t in &s.tables {
        let mut seen: Vec<String> = t.columns.iter().map(|c| norm(&c.name)).collect();
        seen.sort_unstable();
        seen.dedup();
        for n in seen {
            *carried.entry(n).or_insert(0) += 1;
        }
    }
    let mut last: HashMap<String, usize> = HashMap::new();
    for t in &s.tables {
        for c in &t.columns {
            *last.entry(last_token(&c.name)).or_insert(0) += 1;
        }
    }
    let mut pk_last: HashMap<String, usize> = HashMap::new();
    for t in s.tables.iter().filter(|t| t.pk.len() == 1) {
        *pk_last.entry(last_token(&t.pk[0])).or_insert(0) += 1;
    }
    Info {
        pk_end: common_end(&pk_names, 0.5, 2),
        col_end: common_end(&cols, 0.15, 2),
        carried,
        last,
        pk_last,
        tables: s.tables.len(),
    }
}

fn stem5(name: &str, info: &Info) -> Vec<char> {
    let n = chars(&norm(name));
    for end in [&info.col_end, &info.pk_end] {
        if !end.is_empty() && ends_with(&n, end) && n.len() - end.len() >= 2 {
            return n[..n.len() - end.len()].to_vec();
        }
    }
    let tk = tokens(name);
    if tk.len() > 1 && KEY_WORDS.contains(&tk[tk.len() - 1].as_str()) {
        return chars(&tk[..tk.len() - 1].concat());
    }
    n
}

/// `abbreviates`: `a` is `b` shortened, `b`'s first letter, then `a`'s
/// letters in order.
fn abbreviates(a: &[char], b: &[char]) -> bool {
    if a.len() < 2 || b.is_empty() || a[0] != b[0] || a.len() >= b.len() {
        return false;
    }
    let mut j = 1;
    for &c in &a[1..] {
        match b[j.min(b.len())..].iter().position(|&x| x == c) {
            Some(p) => j += p + 1,
            None => return false,
        }
    }
    true
}

/// The fourteen values in `add_cues`'s order.
pub fn values(info: &Info, column: &Column, target_name: &str, key: &str) -> [f64; 14] {
    let c = chars(&norm(&column.name));
    let k = chars(&norm(key));
    let g = chars(&norm(target_name));
    let cs = stem5(&column.name, info);
    let ks = if !info.pk_end.is_empty() && ends_with(&k, &info.pk_end) && k.len() > info.pk_end.len() {
        k[..k.len() - info.pk_end.len()].to_vec()
    } else {
        k.clone()
    };
    let gw: Vec<Vec<char>> = tokens(target_name).into_iter().filter(|w| w.chars().count() >= 3).map(|w| chars(&w)).collect();
    let b = |x: bool| f64::from(u8::from(x));
    let last = last_token(&column.name);
    let tables = info.tables as f64;
    [
        lcp(&c, &k) as f64 / c.len().max(1) as f64,
        lcp(&c, &k) as f64 / k.len().max(1) as f64,
        lcp(&c, &g) as f64 / c.len().max(1) as f64,
        b(!cs.is_empty() && cs == ks),
        b(cs.len() >= 2 && starts_with(&ks, &cs)),
        b(ks.len() >= 3 && contains(&c, &ks) && ks != c),
        b(abbreviates(&cs, &g) || gw.iter().any(|w| abbreviates(&cs, w) || cs == *w)),
        b(abbreviates(&cs, &ks)),
        b(gw.iter().any(|w| w.len() >= 3 && starts_with(&cs, &w[..3.max(w.len() / 2)]))),
        info.carried.get(&norm(&column.name)).copied().unwrap_or(0).min(10) as f64 / 10.0,
        (info.last.get(&last).copied().unwrap_or(0) as f64 / tables).min(3.0) / 3.0,
        b(!info.pk_end.is_empty() && ends_with(&c, &info.pk_end) && c != k),
        info.pk_last.get(&last).copied().unwrap_or(0) as f64 / info.tables.max(1) as f64,
        b(!info.col_end.is_empty() && ends_with(&c, &info.col_end)),
    ]
}
