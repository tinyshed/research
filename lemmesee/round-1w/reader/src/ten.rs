//! Round 1v's version 10 and 11 cues, as `ten.py`'s `schema_info`,
//! `add_cues` and `add_people` compute them: set after version 9's, in the
//! order Python's dict holds them, a zero dropped as FeatureHasher drops it.
//! A length is a count of characters, as Python's `len` of a `str`.

use crate::cues::{self, abbreviates, chars, contains, lcp};
use crate::six::{KEY_WORDS7, carries, role_tokens, roles, split};
use crate::{Column, Named, Schema, norm, singular, tokens};
use std::collections::{HashMap, HashSet};

/// ten.py's `BY`: the words for "by" that end a column named for someone.
pub const BY: [&str; 9] = ["by", "por", "par", "oleh", "von", "door", "da", "przez", "tarafindan"];

fn len(s: &str) -> usize {
    s.chars().count()
}

/// `common_start`: the longest start at least `share` of the names carry
/// (three of them at least), each name three letters longer than it;
/// Counter.most_common(1) keeps the first counted of equal counts.
fn common_start(names: &[Vec<char>], share: f64) -> String {
    let names: Vec<&Vec<char>> = names.iter().filter(|n| !n.is_empty()).collect();
    if names.len() < 4 {
        return String::new();
    }
    let mut best: &[char] = &[];
    for k in 2..=8 {
        let mut order: Vec<&[char]> = Vec::new();
        let mut count: HashMap<&[char], usize> = HashMap::new();
        for n in &names {
            if n.len() >= k + 3 {
                let start = &n[..k];
                let e = count.entry(start).or_insert(0);
                if *e == 0 {
                    order.push(start);
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
        if top.1 as f64 >= 3f64.max(share * names.len() as f64) {
            best = top.0;
        } else {
            break;
        }
    }
    best.iter().collect()
}

/// `people_head`: a table of people by the last word of its name.
fn people_head(name: &str) -> bool {
    let toks = role_tokens(name);
    let Some(last) = toks.last() else { return false };
    let person = &roles().person;
    if person.contains(last) || person.contains(&singular(last)) || person.contains(last.strip_suffix("es").unwrap_or(last)) {
        return true;
    }
    let run = toks.concat();
    person.iter().any(|w| len(w) >= 5 && (run.ends_with(w.as_str()) || run.strip_suffix('s').is_some_and(|r| r.ends_with(w.as_str()))))
}

/// `someone`: (named for an actor, carries a person word) of a column's name.
fn someone(name: &str) -> (bool, bool) {
    // style.py's `pieces`
    let mut words: Vec<String> = tokens(name).iter().flat_map(|t| split(t)).collect();
    while words.len() > 1 && KEY_WORDS7.contains(&words[words.len() - 1].as_str()) {
        words.pop();
    }
    let by = words.len() > 1 && BY.contains(&words[words.len() - 1].as_str());
    let r = roles();
    let person = words.iter().any(|w| r.person.contains(w) || r.person.contains(&singular(w)));
    (by || carries(name, &r.actor), person)
}

/// What `schema_info` reads off a schema once, and version 11's people.
pub struct Info<'a> {
    homes: HashMap<String, usize>,
    parts: HashMap<String, usize>,
    unkeyed: HashMap<String, HashSet<&'a str>>,
    /// the tables' names, each once, in the schema's order
    names: Vec<&'a str>,
    core: HashMap<&'a str, String>,
    plain: HashMap<&'a str, String>,
    cores: HashMap<String, usize>,
    start: String,
    head: HashMap<&'a str, bool>,
    people: usize,
    people_shortest: usize,
}

pub fn info(s: &Schema) -> Info<'_> {
    let (mut homes, mut parts) = (HashMap::new(), HashMap::new());
    let mut unkeyed: HashMap<String, HashSet<&str>> = HashMap::new();
    for t in &s.tables {
        let pk: Vec<String> = t.pk.iter().map(|p| norm(p)).collect();
        if pk.len() == 1 {
            *homes.entry(pk[0].clone()).or_insert(0) += 1;
        } else {
            for p in &pk {
                *parts.entry(p.clone()).or_insert(0) += 1;
            }
        }
        for c in &t.columns {
            let n = norm(&c.name);
            if !pk.contains(&n) {
                unkeyed.entry(n).or_default().insert(t.name.as_str());
            }
        }
    }
    let mut names: Vec<&str> = Vec::new();
    let mut toks: HashMap<&str, Vec<String>> = HashMap::new();
    for t in &s.tables {
        if !toks.contains_key(t.name.as_str()) {
            names.push(&t.name);
            toks.insert(&t.name, tokens(&t.name));
        }
    }
    let need = 3f64.max(0.1 * names.len() as f64);
    let (mut first, mut last): (HashMap<&str, usize>, HashMap<&str, usize>) = (HashMap::new(), HashMap::new());
    for n in &names {
        let tk = &toks[n];
        if tk.len() > 1 {
            *first.entry(tk[0].as_str()).or_insert(0) += 1;
            *last.entry(tk[tk.len() - 1].as_str()).or_insert(0) += 1;
        }
    }
    let run = common_start(&names.iter().map(|n| chars(&norm(n))).collect::<Vec<_>>(), 0.3);
    let often = |m: &HashMap<&str, usize>, w: &str| m.get(w).is_some_and(|&n| n as f64 >= need);
    let (mut core, mut plain, mut cores) = (HashMap::new(), HashMap::new(), HashMap::new());
    for n in &names {
        let tk = &toks[n];
        let mut t2: &[String] = tk;
        if t2.len() > 1 && often(&first, &t2[0]) {
            t2 = &t2[1..];
        }
        if t2.len() > 1 && often(&last, &t2[t2.len() - 1]) {
            t2 = &t2[..t2.len() - 1];
        }
        let mut c = t2.concat();
        if t2.len() == tk.len() && !run.is_empty() && c.starts_with(&run) && len(&c) - len(&run) >= 3 {
            c = c[run.len()..].to_string();
        }
        let c = singular(&c);
        *cores.entry(c.clone()).or_insert(0) += 1;
        core.insert(*n, c);
        plain.insert(*n, singular(&norm(n)));
    }
    let cols: Vec<Vec<char>> = s.tables.iter().flat_map(|t| t.columns.iter().map(|c| chars(&norm(&c.name)))).collect();
    let head: HashMap<&str, bool> = names.iter().map(|n| (*n, people_head(n))).collect();
    let people: Vec<&str> = s.tables.iter().map(|t| t.name.as_str()).filter(|n| head[n]).collect();
    Info {
        homes,
        parts,
        unkeyed,
        core,
        plain,
        cores,
        start: common_start(&cols, 0.15),
        people: people.len(),
        people_shortest: people.iter().map(|n| len(&norm(n))).min().unwrap_or(0),
        head,
        names,
    }
}

/// What the cues read off a column, the same for every target.
pub struct Col<'a> {
    c: String,
    homes: usize,
    parts: usize,
    /// the stem, singular
    stem: String,
    /// `holding`: the tables whose name, or core, holds the stem
    hold: HashSet<&'a str>,
    /// `inside`: a table, the letters of its name or core inside the column's name
    ins: HashMap<&'a str, usize>,
    longest: usize,
    /// the name less the schema's start and the columns' ending, where it starts so
    rest: Option<Vec<char>>,
    actor: bool,
    person: bool,
}

pub fn column<'a>(info: &Info<'a>, v5: &cues::Info, column: &Column) -> Col<'a> {
    let c = norm(&column.name);
    let stem: String = cues::stem5(&column.name, v5).iter().collect();
    let stem = singular(if stem.is_empty() { &c } else { &stem });
    let hold = if len(&stem) >= 3 {
        info.names.iter().copied().filter(|n| info.plain[n].contains(&stem) || info.core[n].contains(&stem)).collect()
    } else {
        HashSet::new()
    };
    let mut ins = HashMap::new();
    for n in &info.names {
        let (p, k) = (&info.plain[n], &info.core[n]);
        let of = |x: &String| if len(x) >= 3 && c.contains(x.as_str()) { len(x) } else { 0 };
        let m = of(p).max(of(k));
        if m > 0 {
            ins.insert(*n, m);
        }
    }
    let rest = (!info.start.is_empty() && c.starts_with(&info.start) && len(&c) - len(&info.start) >= 2).then(|| {
        // cues.py's `strip_end`
        let mut rest = chars(&c[info.start.len()..]);
        let end = &v5.col_end;
        if !end.is_empty() && cues::ends_with(&rest, end) && rest.len() > end.len() {
            rest.truncate(rest.len() - end.len());
        }
        rest
    });
    let (actor, person) = someone(&column.name);
    Col {
        homes: info.homes.get(&c).copied().unwrap_or(0),
        parts: info.parts.get(&c).copied().unwrap_or(0),
        longest: ins.values().copied().max().unwrap_or(0),
        c,
        stem,
        hold,
        ins,
        rest,
        actor,
        person,
    }
}

/// `add_cues`: version 10's cues of a pair set on its features, and the five
/// fields the wide second stage reads: home, carried, view, core, inside.
pub fn add_cues(f: &mut Named, info: &Info, col: &Col, target: &str, key: &str, how: &str) -> [f32; 5] {
    let b = |x: bool| f64::from(u8::from(x));
    let k = norm(key);
    let home = col.c == k && how == "pk" && col.homes == 1;
    let carried = info.unkeyed.get(&col.c).is_some_and(|t| t.contains(target));
    f.set("v10 home".into(), b(home));
    f.set("v10 homes".into(), col.homes.min(4) as f64 / 4.0);
    f.set("v10 key part elsewhere".into(), col.parts.min(4) as f64 / 4.0);
    f.set("v10 target carries the name unkeyed".into(), b(carried));
    let core = &info.core[target];
    let is_core = len(core) >= 2 && col.stem == *core;
    f.set("v10 stem is the core".into(), b(is_core));
    f.set("v10 tables of this core".into(), 1.0 / info.cores[core] as f64);
    f.set("v10 tables holding the stem".into(), if col.hold.contains(target) { 1.0 / col.hold.len() as f64 } else { 0.0 });
    let mine = col.ins.get(target);
    f.set("v10 table inside the column".into(), if mine.is_some() { 1.0 / col.ins.len() as f64 } else { 0.0 });
    let is_inside = mine.is_some_and(|&m| m == col.longest);
    f.set("v10 the longest table inside the column".into(), b(is_inside));
    if let Some(rs) = &col.rest {
        let p = norm(target);
        let (pc, cc, kc) = (chars(&p), chars(core), chars(&k));
        let short = abbreviates(rs, &pc) || abbreviates(rs, &cc) || *rs == cc || *rs == chars(&singular(&p));
        f.set("v10 less the start: shortens the table".into(), b(short));
        f.set("v10 less the start: starts the table".into(), lcp(rs, &cc) as f64 / rs.len().max(1) as f64);
        f.set("v10 less the start: starts the key".into(), lcp(rs, &kc) as f64 / rs.len().max(1) as f64);
        f.set("v10 less the start: inside the key".into(), b(rs.len() >= 3 && contains(&kc, rs)));
    }
    let view = match how {
        "pk" => 0.0,
        "pseudo" => 1.0,
        "part" => 2.0,
        _ => 3.0,
    };
    [f32::from(u8::from(home)), f32::from(u8::from(carried)), view, f32::from(u8::from(is_core)), f32::from(u8::from(is_inside))]
}

/// `add_people`: version 11's cues of a pair whose column is someone's.
pub fn add_people(f: &mut Named, info: &Info, col: &Col, target: &str) {
    if !(col.actor || col.person) {
        return;
    }
    let b = |x: bool| f64::from(u8::from(x));
    let head = info.head[target];
    let n = info.people;
    f.set("v11 tables of people".into(), n.min(4) as f64 / 4.0);
    if col.actor {
        f.set(if head { "v11 actor into a table of people" } else { "v11 actor into another table" }.into(), 1.0);
    }
    if col.person {
        f.set(if head { "v11 person word into a table of people" } else { "v11 person word into another table" }.into(), 1.0);
    }
    if head {
        f.set("v11 the only table of people".into(), b(n == 1));
        f.set("v11 the shortest table of people".into(), b(len(&norm(target)) == info.people_shortest));
    } else if n > 0 {
        f.set("v11 a table of people stands elsewhere".into(), 1.0);
    }
}
