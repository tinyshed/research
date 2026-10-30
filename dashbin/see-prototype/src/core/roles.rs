//! The five as a weekly screen, as round 1m's `roles.py`: each question's role
//! by what it reads — pulse, money, health, speed, people — and the screen,
//! the best over a time of each role there is, then the best of the rest with
//! one pulse.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use super::question::{By, Identity, Question};
use super::rank::{MONEYWORD, order};
use super::schema::Schema;

pub const ORDER: [&str; 5] = ["pulse", "money", "health", "speed", "people"];
const HEALTH: &[&str] = &["status", "state", "stage", "result", "outcome", "resolution", "approved", "approval", "cancel",
    "cancelled", "canceled", "fail", "failed", "failure", "error", "success", "succeeded", "rejected",
    "refund", "refunded", "returned", "paid", "complete", "completed", "active"];

pub static MONEY_WORDS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut w: HashSet<String> = MONEYWORD.iter().map(|s| s.to_string()).collect();
    w.extend(include_str!("moneywords.txt").lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(str::to_string));
    w
});

fn words(slot: Option<&Vec<String>>) -> HashSet<String> {
    slot.map(|s| {
        s.iter()
            .filter(|w| !w.starts_with('#'))
            .map(|w| w.split_once(':').map(|(_, r)| r).unwrap_or(w).trim_start_matches('@').to_string())
            .collect()
    })
    .unwrap_or_default()
}

pub fn role(i: &Identity, slots: &HashMap<&'static str, Vec<String>>) -> &'static str {
    let (agg, n) = (i.agg, i.cols.len());
    if agg == "late" || (agg == "avg" && n == 2) {
        return "speed";
    }
    if (agg == "sum" || agg == "avg") && words(slots.get("measure")).iter().any(|w| MONEY_WORDS.contains(w)) {
        return "money";
    }
    if agg == "count_if" || (i.by.is_some() && words(slots.get("by")).iter().any(|w| HEALTH.contains(&w.as_str()))) {
        return "health";
    }
    if agg == "distinct" {
        return "people";
    }
    if agg == "count" && i.time.is_some() && i.by.is_none() {
        return "pulse";
    }
    "other"
}

pub fn timed_role(i: &Identity, slots: &HashMap<&'static str, Vec<String>>) -> Option<&'static str> {
    let r = role(i, slots);
    i.time?;
    match r {
        "health" | "pulse" | "speed" => Some(r),
        "money" | "people" if i.by.is_none() => Some(r),
        _ => None,
    }
}

/// Round 1m's screen over a ranker's scores.
pub fn screen(schema: &Schema, qs: &[Question], ids: &[Identity], scores: &[f64], slots: &[HashMap<&'static str, Vec<String>>], k: usize) -> Vec<usize> {
    let ord = order(scores);
    let mut timed: Vec<Option<&str>> = ids.iter().zip(slots).map(|(i, s)| timed_role(i, s)).collect();
    let plain: Vec<&str> = ids.iter().zip(slots).map(|(i, s)| role(i, s)).collect();
    for (i, q) in qs.iter().enumerate() {
        if timed[i] == Some("people") && !q.measure.cols.first().is_some_and(|&c| schema.col(c).fk.is_some()) {
            timed[i] = None;
        }
    }
    let mut chosen: Vec<usize> = Vec::new();
    let mut tables: HashMap<usize, usize> = HashMap::new();
    let (mut bys, mut asked, mut times): (HashSet<By>, HashSet<Identity>, HashSet<(usize, usize)>) = Default::default();
    let fits = |i: usize, strict: bool, tables: &HashMap<usize, usize>, bys: &HashSet<By>, asked: &HashSet<Identity>, times: &HashSet<(usize, usize)>| -> bool {
        let q = &qs[i];
        if asked.contains(&ids[i]) || *tables.get(&q.base).unwrap_or(&0) >= 2 || ids[i].by.as_ref().is_some_and(|b| bys.contains(b)) {
            return false;
        }
        !(strict && q.time.as_ref().is_some_and(|d| times.contains(&(d.t, d.c as usize))))
    };
    let take = |i: usize, chosen: &mut Vec<usize>, tables: &mut HashMap<usize, usize>, bys: &mut HashSet<By>, asked: &mut HashSet<Identity>, times: &mut HashSet<(usize, usize)>| {
        let q = &qs[i];
        chosen.push(i);
        asked.insert(ids[i].clone());
        *tables.entry(q.base).or_insert(0) += 1;
        if let Some(b) = &ids[i].by {
            bys.insert(b.clone());
        }
        if let Some(d) = &q.time {
            times.insert((d.t, d.c as usize));
        }
    };
    for r in ORDER {
        if let Some(&i) = ord.iter().find(|&&i| timed[i] == Some(r) && fits(i, false, &tables, &bys, &asked, &times)) {
            take(i, &mut chosen, &mut tables, &mut bys, &mut asked, &mut times);
        }
    }
    for &i in &ord {
        if chosen.len() >= k {
            break;
        }
        if fits(i, true, &tables, &bys, &asked, &times) && !(plain[i] == "pulse" && chosen.iter().any(|&j| plain[j] == "pulse")) {
            take(i, &mut chosen, &mut tables, &mut bys, &mut asked, &mut times);
        }
    }
    chosen.truncate(k);
    chosen.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    chosen
}
