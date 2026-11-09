//! Round 1t's version 6 to 8 candidates and cues, as `six.py`'s `pairs`,
//! `words7.py` and `stack.py` compute them, by name, in the order Python's
//! dicts hold them. The three lexicons are compiled into the binary.

use crate::*;
use std::collections::{BTreeSet, HashMap};

const KIND6: [(&str, &str); 5] = [("int", "int"), ("decimal", "int"), ("uuid", "uuid"), ("text", "text"), ("longtext", "text")];
const KEY_WORDS7: [&str; 12] = ["id", "key", "code", "no", "num", "nr", "fk", "ref", "pk", "uid", "link", "ptr"];

fn kind6(family: &str) -> Option<&'static str> {
    KIND6.iter().find(|(f, _)| *f == family).map(|(_, k)| *k)
}

fn view_order(how: &str) -> u8 {
    match how {
        "pk" => 0,
        "pseudo" => 1,
        "part" => 2,
        _ => 3,
    }
}

// --- the lexicons (words7.py) ------------------------------------------------

pub struct Lex {
    cost: HashMap<String, f64>,
    unknown: f64,
    longest: usize,
    abbrev: HashMap<String, Vec<String>>,
    related: HashMap<String, Vec<String>>,
}

pub fn lex() -> &'static Lex {
    static LEX: std::sync::OnceLock<Lex> = std::sync::OnceLock::new();
    LEX.get_or_init(|| {
        let mut counts: Vec<(String, u64)> = Vec::new();
        for line in include_str!("../../vocab.txt").lines() {
            let mut it = line.split_whitespace();
            if let (Some(w), Some(n)) = (it.next(), it.next()) {
                counts.push((w.to_string(), n.parse().unwrap()));
            }
        }
        let total: u64 = counts.iter().map(|(_, n)| n).sum();
        let cost = counts.iter().map(|(w, n)| (w.clone(), -((*n as f64) / (total as f64)).ln())).collect();
        let longest = counts.iter().map(|(w, _)| w.chars().count()).max().unwrap_or(1);
        let abbrev: HashMap<String, Vec<String>> = serde_json::from_str(include_str!("../../abbrev.json")).unwrap();
        let related: HashMap<String, Vec<String>> = serde_json::from_str(include_str!("../../related.json")).unwrap();
        let related = related.into_iter().map(|(k, v)| {
            let set: BTreeSet<String> = v.iter().map(|x| singular(x)).collect();
            (k, set.into_iter().collect())
        }).collect();
        Lex { cost, unknown: -(1.0 / total as f64).ln(), longest, abbrev, related }
    })
}

// --- version 9's roles (roles.py) -------------------------------------------

pub struct Roles {
    selfw: std::collections::HashSet<String>,
    actor: std::collections::HashSet<String>,
    person: std::collections::HashSet<String>,
}

pub fn roles() -> &'static Roles {
    static ROLES: std::sync::OnceLock<Roles> = std::sync::OnceLock::new();
    ROLES.get_or_init(|| {
        let m: HashMap<String, Vec<String>> = serde_json::from_str(include_str!("../../roles.json")).unwrap();
        let set = |k: &str| m.get(k).map(|v| v.iter().cloned().collect()).unwrap_or_default();
        Roles { selfw: set("self"), actor: set("actor"), person: set("person") }
    })
}

/// roles.py's `_tokens`: camel case split as its two substitutions do, then
/// the ASCII letters' runs, lower-cased.
fn role_tokens(name: &str) -> Vec<String> {
    let c: Vec<char> = name.chars().collect();
    let mut s1: Vec<char> = Vec::with_capacity(c.len() + 8);
    for (i, &ch) in c.iter().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() && (c[i - 1].is_ascii_lowercase() || c[i - 1].is_ascii_digit()) {
            s1.push(' ');
        }
        s1.push(ch);
    }
    let mut s2: Vec<char> = Vec::with_capacity(s1.len() + 8);
    for (i, &ch) in s1.iter().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() && s1[i - 1].is_ascii_uppercase()
            && s1.get(i + 1).is_some_and(|n| n.is_ascii_lowercase()) {
            s2.push(' ');
        }
        s2.push(ch);
    }
    let lower: String = s2.iter().collect::<String>().to_lowercase();
    lower.split(|ch: char| !ch.is_ascii_alphabetic()).filter(|t| !t.is_empty()).map(String::from).collect()
}

/// roles.py's `carries`: a token among the words, or, four letters or more, the
/// name run together beginning or ending with one.
fn carries(name: &str, words: &std::collections::HashSet<String>) -> bool {
    let toks = role_tokens(name);
    if toks.iter().any(|t| words.contains(t)) {
        return true;
    }
    let run = toks.concat();
    words.iter().any(|w| w.chars().count() >= 4 && (run.starts_with(w.as_str()) || run.ends_with(w.as_str())))
}

/// `split`: a token as vocabulary words, or itself.
fn split(token: &str) -> Vec<String> {
    let lx = lex();
    let t: Vec<char> = token.chars().collect();
    let n = t.len();
    if n < 5 || lx.cost.contains_key(token) {
        return vec![token.to_string()];
    }
    let mut best = vec![f64::INFINITY; n + 1];
    best[0] = 0.0;
    let mut back = vec![0usize; n + 1];
    for i in 1..=n {
        let c = best[i - 1] + lx.unknown;
        if c < best[i] {
            best[i] = c;
            back[i] = i - 1;
        }
        let lo = i.saturating_sub(lx.longest);
        for j in lo..i.saturating_sub(1) {
            let w: String = t[j..i].iter().collect();
            if let Some(&cw) = lx.cost.get(&w) {
                if best[j] + cw < best[i] {
                    best[i] = best[j] + cw;
                    back[i] = j;
                }
            }
        }
    }
    let mut parts = Vec::new();
    let mut i = n;
    while i > 0 {
        parts.push(t[back[i]..i].iter().collect::<String>());
        i = back[i];
    }
    parts.reverse();
    if parts.iter().any(|p| p.chars().count() == 1) {
        return vec![token.to_string()];
    }
    parts
}

/// words7.py's `words`.
fn words7(name: &str) -> BTreeSet<String> {
    let lx = lex();
    let mut out = BTreeSet::new();
    for tok in tokens(name) {
        for w in split(&tok) {
            if KEY_WORDS7.contains(&w.as_str()) {
                continue;
            }
            out.insert(singular(&w));
            if let Some(es) = lx.abbrev.get(&w) {
                for e in es {
                    for x in e.split_whitespace() {
                        if x.chars().count() >= 2 && !KEY_WORDS7.contains(&x) {
                            out.insert(singular(x));
                        }
                    }
                }
            }
        }
    }
    out
}

/// six.py's `words`.
fn words6(name: &str) -> BTreeSet<String> {
    tokens(name).into_iter().filter(|w| !KEY_WORDS.contains(&w.as_str()) && w.chars().count() >= 2).map(|w| singular(&w)).collect()
}

fn views(t: &Table) -> Vec<(&Column, &'static str)> {
    let col = |name: &str| t.columns.iter().find(|c| c.name == name);
    let mut out: Vec<(&Column, &'static str)> = Vec::new();
    if t.pk.len() == 1 {
        if let Some(c) = col(&t.pk[0]) {
            if kind6(&c.family).is_some() {
                out.push((c, "pk"));
            }
        }
    } else if t.pk.is_empty() {
        let unique: Vec<&Column> = t.columns.iter().filter(|c| c.unique() && kind6(&c.family).is_some()).collect();
        if unique.len() == 1 {
            out.push((unique[0], "pseudo"));
        } else {
            let g = norm(&t.name);
            let names = ["id".to_string(), format!("{g}id"), format!("{}id", singular(&g))];
            if let Some(c) = t.columns.iter().find(|c| names.contains(&norm(&c.name)) && kind6(&c.family).is_some()) {
                out.push((c, "pseudo"));
            }
        }
    } else {
        for p in &t.pk {
            if let Some(c) = col(p) {
                if kind6(&c.family).is_some() {
                    out.push((c, "part"));
                }
            }
        }
    }
    let taken: Vec<*const Column> = out.iter().map(|(c, _)| *c as *const Column).collect();
    for c in &t.columns {
        if c.unique() && !taken.contains(&(c as *const Column)) && kind6(&c.family).is_some() {
            out.push((c, "unique"));
        }
    }
    out
}

struct View<'a> {
    table: usize,
    key: &'a Column,
    kind: &'static str,
    how: &'static str,
    name: String,
    singular: String,
    singular_chars: Vec<char>,
    grams: Vec<String>,
    pk: String,
    words: BTreeSet<String>,
    key_words: BTreeSet<String>,
    tokens: Vec<String>,
    last_tokens: Vec<String>,
    columns: usize,
}

fn strip_digits(s: &str) -> &str {
    s.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// six.py's `pairs`, its version 7 and 8 cues as asked, each pair's named
/// features kept when `keep_names`.
pub fn pairs(s: &Schema, seven: bool, eight: bool, nine: bool, keep_names: bool) -> (Vec<Pair>, Vec<Named>) {
    let n_tables = s.tables.len();
    let mut targets: Vec<View> = Vec::new();
    for (ti, t) in s.tables.iter().enumerate() {
        let g = norm(&t.name);
        let sg = singular(&g);
        let gt = tokens(&t.name);
        for (key, how) in views(t) {
            targets.push(View {
                table: ti, key, kind: kind6(&key.family).unwrap(), how,
                grams: union(&grams(&g), &grams(&sg)), singular_chars: sg.chars().collect(), name: g.clone(),
                singular: sg.clone(), pk: norm(&key.name), words: words6(&t.name), key_words: words6(&key.name),
                last_tokens: gt[gt.len().saturating_sub(2)..].to_vec(), tokens: gt.clone(), columns: t.columns.len(),
            });
        }
    }
    let stems: Vec<String> = s.tables.iter().flat_map(|t| t.columns.iter().map(|c| stem_of(&c.name).0)).collect();
    let named_by: Vec<usize> = targets
        .iter()
        .map(|g| stems.iter().filter(|st| !st.is_empty() && (**st == g.name || **st == g.singular)).count())
        .collect();
    let all_tokens: Vec<Vec<String>> = s.tables.iter().flat_map(|t| t.columns.iter().map(|c| tokens(&c.name))).collect();
    let id_convention = all_tokens.iter().filter(|tk| tk.last().is_some_and(|l| l == "id")).count() as f64
        / all_tokens.len().max(1) as f64;
    let mut key_count: HashMap<&str, usize> = HashMap::new();
    for g in &targets {
        *key_count.entry(g.pk.as_str()).or_insert(0) += 1;
    }
    let bare: Vec<String> =
        s.tables.iter().flat_map(|t| t.columns.iter().map(|c| strip_digits(&norm(&c.name)).to_string())).collect();
    let carrying: HashMap<&str, usize> = key_count
        .keys()
        .filter(|k| **k != "id" && k.chars().count() >= 3)
        .map(|k| (*k, bare.iter().filter(|b| b.contains(*k)).count()))
        .collect();
    let keyed_share: Vec<f64> = s
        .tables
        .iter()
        .map(|t| {
            let n = t.columns.iter().filter(|c| key_count.contains_key(strip_digits(&norm(&c.name)))).count();
            n as f64 / t.columns.len().max(1) as f64
        })
        .collect();
    let info = cues::info(s);
    let b = |x: bool| f64::from(u8::from(x));

    let mut out = Vec::new();
    let mut names_out = Vec::new();
    for (ti, table) in s.tables.iter().enumerate() {
        let table_tokens = tokens(&table.name);
        let no_key = b(table.pk.is_empty());
        for (ci, column) in table.columns.iter().enumerate() {
            let Some(ck) = kind6(&column.family) else { continue };
            let own = column.primary() && table.pk.len() == 1;
            let (stem, tk) = stem_of(&column.name);
            let stem_chars: Vec<char> = stem.chars().collect();
            let sg = if stem.is_empty() { Vec::new() } else { grams(&stem) };
            let cname = norm(&column.name);
            let cwords = words6(&column.name);
            // each table once, through the view the names fit best, in the
            // order its first view comes
            let mut order: Vec<usize> = Vec::new();
            let mut chosen: HashMap<usize, (usize, f64, bool)> = HashMap::new();
            for (k, g) in targets.iter().enumerate() {
                if g.table == ti && column.name == g.key.name {
                    continue;
                }
                let cross = g.kind != ck;
                if cross && !(cname == g.pk || (!stem.is_empty() && (stem == g.name || stem == g.singular))) {
                    continue;
                }
                let exact = stem == g.name || stem == g.singular || format!("{stem}s") == g.name;
                let p = prefix(&stem_chars, &g.singular_chars);
                let pre = if p >= 3 { 0.8 * p as f64 / 1usize.max(stem_chars.len().min(g.singular_chars.len())) as f64 } else { 0.0 };
                let bb = b(exact).max(jaccard(&sg, &g.grams)).max(b(cname == g.pk)).max(pre);
                match chosen.get(&g.table) {
                    None => {
                        order.push(g.table);
                        chosen.insert(g.table, (k, bb, cross));
                    }
                    Some(&(k0, b0, _)) => {
                        if bb > b0 || (bb == b0 && view_order(g.how) < view_order(targets[k0].how)) {
                            chosen.insert(g.table, (k, bb, cross));
                        }
                    }
                }
            }
            if order.is_empty() {
                continue;
            }
            let mine: Vec<(usize, f64, bool)> = order.iter().map(|t| chosen[t]).collect();
            let base: Vec<f64> = mine.iter().map(|m| m.1).collect();
            let best = base.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let less = strip_digits(&cname).to_string();

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
            let mut letters = Named::default();
            for gram in sg.iter().take(12) {
                letters.set(format!("cg:{gram}"), 1.0);
            }
            let letters_hashed = hashed(&letters);
            let stem_tokens: &[String] = if tk.last().is_some_and(|l| l == "id") { &tk[..tk.len() - 1] } else { &tk[..] };
            let stem2 = stem_tokens.concat();
            let stem_set: std::collections::BTreeSet<&str> = stem_tokens.iter().map(String::as_str).collect();
            let last_stem = &stem_tokens[stem_tokens.len().saturating_sub(2)..];
            let ends_id = b(tk.last().is_some_and(|l| l == "id"));
            let key_word = tk.last().cloned().unwrap_or_default();
            let cw7 = if seven || eight { words7(&column.name) } else { BTreeSet::new() };

            for (j, &(k, bb, cross)) in mine.iter().enumerate() {
                let g = &targets[k];
                let target = &s.tables[g.table];
                let others: Vec<f64> = base.iter().enumerate().filter(|(o, _)| *o != j).map(|(_, &x)| x).collect();
                let max_others = if others.is_empty() { 0.0 } else { others.iter().copied().fold(f64::NEG_INFINITY, f64::max) };
                let mut gnamed = Named::default();
                for tok in &g.tokens {
                    gnamed.set(format!("g:{tok}"), 1.0);
                }
                let g_hashed = hashed(&gnamed);
                let mut p = Named::default();
                for a in last_stem {
                    for bt in &g.last_tokens {
                        p.set(format!("x:{a}|{bt}"), 1.0);
                    }
                }
                let sgl = singular(&g.name);
                p.set("stem=target".into(), b(stem2 == g.name));
                p.set("stem=target singular".into(), b(stem2 == sgl));
                p.set("stem in target".into(), b(!stem2.is_empty() && g.name.contains(&stem2)));
                p.set("target in stem".into(), b(!sgl.is_empty() && stem2.contains(&sgl)));
                p.set("named as target's key".into(), b(cname == g.pk));
                let shared = stem_set.iter().filter(|t| **t != "id" && g.tokens.iter().any(|x| x == *t)).count();
                p.set("shared tokens".into(), shared as f64 / 3.0);
                p.set("ends in id".into(), ends_id);
                p.set("self".into(), b(g.table == ti));
                p.set("target columns".into(), g.columns.min(60) as f64 / 60.0);
                p.set("v2 letters".into(), jaccard(&sg, &g.grams));
                p.set("v2 edit".into(), similarity(&stem_chars, &g.singular_chars));
                p.set("v2 prefix".into(), prefix(&stem_chars, &g.singular_chars).min(8) as f64 / 8.0);
                p.set("v2 base".into(), bb);
                p.set("v2 best of the column".into(), b(bb >= best));
                p.set("v2 margin".into(), bb - max_others);
                p.set("v2 close rivals".into(), others.iter().filter(|&&x| x >= 0.5).count().min(5) as f64 / 5.0);
                p.set("v2 named as the key".into(), b(cname == g.pk));
                p.set("v2 same type".into(), b(column.ty.as_deref() == g.key.ty.as_deref()));
                p.set("v2 named by".into(), named_by[k].min(10) as f64 / 10.0);
                p.set("v2 id convention".into(), id_convention);
                p.set("v2 bare".into(), b(stem.is_empty()));
                p.set(format!("v2 key word:{key_word}"), 1.0);
                let head = hashed(&p);
                let mut tail = Named::default();
                tail.set(format!("p3:{}|{}", first_chars(&stem, 3), first_chars(&g.singular, 3)), 1.0);
                tail.set("v3 own key".into(), b(own));
                tail.set("v3 pseudo target".into(), b(g.how == "pseudo"));
                let kk = g.pk.as_str();
                let kn = kk.chars().count();
                tail.set("v4 less digits is the key".into(), b(less == kk && less != cname));
                tail.set("v4 key inside".into(), b(kk != "id" && kn >= 3 && less.contains(kk)));
                tail.set("v4 ends with the key".into(), b(kn >= 3 && less.ends_with(kk) && less != kk));
                tail.set("v4 targets keyed so".into(), 1.0 / key_count[kk] as f64);
                tail.set("v4 columns carrying the key".into(), carrying.get(kk).copied().unwrap_or(0).min(20) as f64 / 20.0);
                for (n, x) in cues::NAMES.iter().zip(cues::values(&info, column, &target.name, &g.key.name)) {
                    tail.set((*n).into(), x);
                }
                tail.set(format!("v6 view:{}", g.how), 1.0);
                tail.set("v6 families differ".into(), b(cross));
                tail.set("v6 source declares no key".into(), no_key);
                tail.set("v6 source columns keyed".into(), keyed_share[ti]);
                tail.set("v6 words shared".into(), cwords.intersection(&g.words).count().min(3) as f64 / 3.0);
                tail.set("v6 words shared with the key".into(), cwords.intersection(&g.key_words).count().min(3) as f64 / 3.0);
                tail.set("v6 decimal".into(), b(column.family == "decimal"));
                if seven || eight {
                    let tw = words7(&target.name);
                    if seven {
                        let kw = words7(&g.key.name);
                        tail.set("v7 words shared with the table".into(), cw7.intersection(&tw).count().min(3) as f64 / 3.0);
                        tail.set("v7 words shared with the key".into(), cw7.intersection(&kw).count().min(3) as f64 / 3.0);
                        tail.set("v7 table's words all in the column".into(), b(!tw.is_empty() && tw.is_subset(&cw7)));
                        tail.set("v7 key's words all in the column".into(), b(!kw.is_empty() && kw.is_subset(&cw7)));
                        tail.set("v7 column words".into(), cw7.len().min(6) as f64 / 6.0);
                    }
                    if eight {
                        let rel = &lex().related;
                        let hits: usize = cw7.iter().map(|w| rel.get(w).map_or(0, |rs| rs.iter().filter(|r| tw.contains(*r)).count())).sum();
                        tail.set("v8 the column's entity names the table".into(), b(hits > 0));
                        tail.set("v8 entity matches".into(), hits.min(3) as f64 / 3.0);
                        tail.set("v8 the table's entity names the column".into(),
                                 b(tw.iter().any(|w| rel.get(w).is_some_and(|rs| rs.iter().any(|r| cw7.contains(r))))));
                    }
                }
                if nine {
                    let r = roles();
                    let own = target.name == table.name;
                    if carries(&column.name, &r.selfw) {
                        tail.set("v9 self word, own table".into(), b(own));
                        tail.set("v9 self word, another table".into(), b(!own));
                    }
                    if carries(&column.name, &r.actor) {
                        let person = carries(&target.name, &r.person);
                        tail.set("v9 actor word, a table of people".into(), b(person));
                        tail.set("v9 actor word, another table".into(), b(!person));
                    }
                }
                let tail_hashed = hashed(&tail);
                out.push(Pair {
                    table: ti, column: ci, target: g.table, base: bb,
                    same_type: b(column.ty.as_deref() == g.key.ty.as_deref()), named_key: b(cname == g.pk), ends_id,
                    bare: b(stem.is_empty()),
                    features: row(&[&col_hashed, &g_hashed, &head, &letters_hashed, &tail_hashed]),
                });
                if keep_names {
                    let mut all = Named::default();
                    for part in [&col, &gnamed, &p, &letters, &tail] {
                        for (n, v) in &part.items {
                            all.set(n.clone(), *v);
                        }
                    }
                    names_out.push(all);
                }
            }
        }
    }
    (out, names_out)
}

/// stack.py's six features read against the schema, after graph.py's 19.
pub fn relative(pairs: &[Pair], scores: &[f32], base: Vec<[f32; 19]>) -> Vec<Vec<f32>> {
    let mut bests: HashMap<(usize, usize), f64> = HashMap::new();
    let mut lists: HashMap<(usize, usize), Vec<f64>> = HashMap::new();
    for (i, p) in pairs.iter().enumerate() {
        lists.entry((p.table, p.column)).or_default().push(scores[i] as f64);
    }
    let mut nexts: HashMap<(usize, usize), f64> = HashMap::new();
    for (k, v) in &lists {
        let mut v = v.clone();
        v.sort_by(|a, b| b.partial_cmp(a).unwrap());
        bests.insert(*k, v[0]);
        nexts.insert(*k, if v.len() > 1 { v[1] } else { 0.0 });
    }
    let mut ordered: Vec<f64> = bests.values().copied().collect();
    ordered.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = ordered.len();
    let top = ordered.last().copied().unwrap_or(0.0);
    let mean = if n > 0 { pairwise_sum(&ordered) / n as f64 } else { 0.0 };
    base.into_iter()
        .enumerate()
        .map(|(i, row)| {
            let p = &pairs[i];
            let k = (p.table, p.column);
            let sc = scores[i] as f64;
            let mine = bests[&k];
            let other = if sc >= mine { nexts[&k] } else { mine };
            let place = if n > 1 { ordered.partition_point(|&x| x < sc) as f64 / (n - 1) as f64 } else { 1.0 };
            let mut v = row.to_vec();
            v.extend_from_slice(&[
                (if top > 0.0 { sc / top } else { 0.0 }) as f32,
                (if mean > 0.0 { mine / mean } else { 0.0 }) as f32,
                place.min(1.0) as f32,
                ((sc + 1e-3) / (other + 1e-3)).ln() as f32,
                top as f32,
                mean as f32,
            ]);
            v
        })
        .collect()
}

/// NumPy's pairwise summation of a float64 array, as `np.mean` adds.
fn pairwise_sum(a: &[f64]) -> f64 {
    if a.len() < 8 {
        return a.iter().fold(0.0, |s, &x| s + x);
    }
    if a.len() <= 128 {
        let mut r = [0.0f64; 8];
        r.copy_from_slice(&a[..8]);
        let mut i = 8;
        while i + 8 <= a.len() {
            for k in 0..8 {
                r[k] += a[i + k];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < a.len() {
            res += a[i];
            i += 1;
        }
        return res;
    }
    let half = (a.len() / 2) - (a.len() / 2) % 8;
    pairwise_sum(&a[..half]) + pairwise_sum(&a[half..])
}
