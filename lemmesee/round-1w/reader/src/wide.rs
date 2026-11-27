//! Round 1v's wide second stage, as `wide.py`'s `wide_features` computes it
//! for one schema's pairs: four groups of a column's like (the same name, an
//! actor's, the same lead word, the same last word) with seven figures each,
//! then fifteen of the pair's own. Every figure is computed in f64 from the
//! first stage's f32 scores and stored as f32, as NumPy stores Python's
//! floats in its float32 array; sums run in the order Python's dicts hold
//! their keys.

use crate::six::{KEY_WORDS7, carries, pairwise_sum, roles, split};
use crate::{Pair, Schema, norm, tokens};
use std::collections::{HashMap, HashSet};

pub const GROUPS: usize = 4;
pub const FIGURES: usize = 7;
pub const WIDE: usize = GROUPS * FIGURES + 15;
const CONFIDENT: f64 = 0.5;

/// `group_keys`: a column's name as letters and digits, `@` for an actor's,
/// its lead word (style.py's `lead_and_last` with no lead for a name of one
/// word) and its last word; an empty one is no group.
fn group_keys(name: &str) -> [String; GROUPS] {
    let pieces: Vec<String> = tokens(name).iter().flat_map(|t| split(t)).collect();
    let words: Vec<&String> = pieces.iter().filter(|w| !KEY_WORDS7.contains(&w.as_str()) && w.chars().count() >= 2).collect();
    let lead = if words.len() > 1 { words[0].clone() } else { String::new() };
    let actor = if carries(name, &roles().actor) { "@" } else { "" };
    [norm(name), actor.to_string(), lead, pieces.last().cloned().unwrap_or_default()]
}

/// `logit`, with the natural logarithm of the platform's libm as Python's
/// `math.log` takes it.
fn logit(p: f64) -> f64 {
    ((p + 1e-4) / (1.0 - p + 1e-4)).ln()
}

/// `valley`: Otsu's cut of the columns' best scores as logits: the cut, the
/// share of the variance it explains, the share of columns above it and the
/// gap it sits in. NumPy's order of operations: `var` is the pairwise sum of
/// the squared distances from the pairwise mean, `cumsum` adds in order.
fn valley(bests: &[f64]) -> [f64; 4] {
    let n = bests.len();
    if n < 4 {
        return [0.0; 4];
    }
    let mut v: Vec<f64> = bests.iter().map(|&b| logit(b)).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let nf = n as f64;
    let mean = pairwise_sum(&v) / nf;
    let squares: Vec<f64> = v.iter().map(|x| (x - mean) * (x - mean)).collect();
    let var = pairwise_sum(&squares) / nf;
    if var <= 1e-9 {
        return [v[0], 0.0, 0.0, 0.0];
    }
    let mut cs = Vec::with_capacity(n);
    let mut sum = 0.0;
    for (i, x) in v.iter().enumerate() {
        sum = if i == 0 { *x } else { sum + x };
        cs.push(sum);
    }
    let (mut j, mut most) = (0, f64::NEG_INFINITY);
    for i in 0..n - 1 {
        let k = i + 1;
        let m0 = cs[i] / k as f64;
        let m1 = (cs[n - 1] - cs[i]) / (n - k) as f64;
        let between = (k * (n - k)) as f64 * ((m0 - m1) * (m0 - m1)) / (n * n) as f64;
        if between > most {
            (j, most) = (i, between);
        }
    }
    [(v[j] + v[j + 1]) / 2.0, most / var, (n - j - 1) as f64 / nf, v[j + 1] - v[j]]
}

/// One way of grouping the schema's columns, as `wide_features` holds it.
#[derive(Default)]
struct Group<'a> {
    /// a column's group, or none
    of: Vec<Option<usize>>,
    members: Vec<i64>,
    confident: Vec<i64>,
    best_sum: Vec<f64>,
    best_is: HashMap<(usize, &'a str), i64>,
    /// (group, target): where its sum and two best scores are
    at: HashMap<(usize, &'a str), usize>,
    into: Vec<f64>,
    top: Vec<[f64; 2]>,
    /// a group's two best targets by summed score
    lead: Vec<[(f64, Option<&'a str>); 2]>,
}

pub fn features(s: &Schema, pairs: &[Pair], scores: &[f32]) -> Vec<[f32; WIDE]> {
    let tables = s.tables.len().max(1) as f64;
    let target = |p: &Pair| s.tables[p.target].name.as_str();
    // the columns, by their table's and their own name, in the pairs' order
    let mut ids: HashMap<(&str, &str), usize> = HashMap::new();
    let mut names: Vec<&str> = Vec::new();
    let mut col_of = Vec::with_capacity(pairs.len());
    for p in pairs {
        let t = &s.tables[p.table];
        let key = (t.name.as_str(), t.columns[p.column].name.as_str());
        let next = ids.len();
        let c = *ids.entry(key).or_insert(next);
        if c == names.len() {
            names.push(key.1);
        }
        col_of.push(c);
    }
    let n = names.len();
    let (mut rows, mut best, mut total) = (vec![0usize; n], vec![-1.0f64; n], vec![0.0f64; n]);
    let mut best_target: Vec<Option<&str>> = vec![None; n];
    for (i, p) in pairs.iter().enumerate() {
        let (c, sc) = (col_of[i], scores[i] as f64);
        rows[c] += 1;
        total[c] += sc;
        if sc > best[c] {
            best[c] = sc;
            best_target[c] = Some(target(p));
        }
    }
    let gk: Vec<[String; GROUPS]> = names.iter().map(|name| group_keys(name)).collect();
    let mut groups: Vec<Group> = Vec::with_capacity(GROUPS);
    for gi in 0..GROUPS {
        let mut g = Group::default();
        let mut known: HashMap<&str, usize> = HashMap::new();
        for c in 0..n {
            let key = gk[c][gi].as_str();
            if key.is_empty() {
                g.of.push(None);
                continue;
            }
            let next = known.len();
            let id = *known.entry(key).or_insert(next);
            if id == g.members.len() {
                g.members.push(0);
                g.confident.push(0);
                g.best_sum.push(0.0);
            }
            g.of.push(Some(id));
            g.members[id] += 1;
            g.confident[id] += i64::from(best[c] >= CONFIDENT);
            g.best_sum[id] += best[c];
            if let Some(t) = best_target[c] {
                *g.best_is.entry((id, t)).or_insert(0) += 1;
            }
        }
        for (i, p) in pairs.iter().enumerate() {
            let Some(id) = g.of[col_of[i]] else { continue };
            if g.members[id] < 2 {
                continue;
            }
            let sc = scores[i] as f64;
            let next = g.into.len();
            let at = *g.at.entry((id, target(p))).or_insert(next);
            if at == next {
                g.into.push(0.0);
                g.top.push([0.0, 0.0]);
            }
            g.into[at] += sc;
            let t2 = &mut g.top[at];
            if sc > t2[0] {
                *t2 = [sc, t2[0]];
            } else if sc > t2[1] {
                t2[1] = sc;
            }
        }
        g.lead = vec![[(0.0, None), (0.0, None)]; g.members.len()];
        // in the order the sums were first met, as Python's dict gives them
        let mut order: Vec<(usize, (usize, &str))> = g.at.iter().map(|(k, &at)| (at, *k)).collect();
        order.sort_unstable_by_key(|e| e.0);
        for (at, (id, t)) in order {
            let (v, l2) = (g.into[at], &mut g.lead[id]);
            if v > l2[0].0 {
                *l2 = [(v, Some(t)), l2[0]];
            } else if v > l2[1].0 {
                l2[1] = (v, Some(t));
            }
        }
        groups.push(g);
    }
    let [cut, apart, above, width] = valley(&best);
    let mut names_into: HashMap<&str, HashSet<&str>> = HashMap::new();
    for c in 0..n {
        if best[c] >= CONFIDENT {
            if let Some(t) = best_target[c] {
                names_into.entry(t).or_default().insert(gk[c][0].as_str());
            }
        }
    }
    let mut out = Vec::with_capacity(pairs.len());
    for (i, p) in pairs.iter().enumerate() {
        let (c, t, sc) = (col_of[i], target(p), scores[i] as f64);
        let mut row = [0.0f32; WIDE];
        let b = best[c];
        let is_best = best_target[c] == Some(t);
        for (gi, g) in groups.iter().enumerate() {
            let figures = &mut row[gi * FIGURES..(gi + 1) * FIGURES];
            let others = g.of[c].map_or(0, |id| g.members[id] - 1);
            if others <= 0 {
                figures.fill(-1.0);
                continue;
            }
            let id = g.of[c].unwrap();
            let (nf, at) = (others as f64, g.at[&(id, t)]);
            let t2 = g.top[at];
            let lead = g.lead[id];
            let other = if lead[0].1 == Some(t) { lead[1].0 } else { lead[0].0 };
            let best_is = g.best_is.get(&(id, t)).copied().unwrap_or(0);
            let values = [
                nf / tables,
                (g.into[at] - sc) / nf,
                if sc >= t2[0] { t2[1] } else { t2[0] },
                (best_is - i64::from(is_best)) as f64 / nf,
                (g.into[at] - other) / (others + 1) as f64,
                (g.confident[id] - i64::from(b >= CONFIDENT)) as f64 / nf,
                (g.best_sum[id] - b) / nf,
            ];
            for (x, v) in figures.iter_mut().zip(values) {
                *x = v as f32;
            }
        }
        let confident_names = names_into.get(t).map_or(0, |set| set.len() - usize::from(set.contains(gk[c][0].as_str())));
        let own = [
            tables.log2() / 8.0,
            (rows[c] as f64).log2() / 8.0,
            if total[c] > 0.0 { b / total[c] } else { 0.0 },
            total[c].min(5.0),
            confident_names as f64 / tables,
        ];
        let o = GROUPS * FIGURES;
        for (x, v) in row[o..].iter_mut().zip(own) {
            *x = v as f32;
        }
        row[o + 5..o + 10].copy_from_slice(&p.ten);
        let rest = [logit(sc) - cut, logit(b) - cut, apart, above, width];
        for (x, v) in row[o + 10..].iter_mut().zip(rest) {
            *x = v as f32;
        }
        out.push(row);
    }
    out
}
