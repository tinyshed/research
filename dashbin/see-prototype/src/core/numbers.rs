//! The row of numbers, as round 1n's `kpi.py` in round 1o's version: who uses
//! the product, the thing it is for, what counts as done, who did it and did
//! it twice, the shares, the funnel, the new, a month's money, a week's
//! failures — read from the schema, a status's values read from the database.

use std::collections::{HashMap, HashSet};

use super::names::{has, singular};
use super::question::alive;
use super::run::Runner;
use super::schema::{Column, Schema, Table};
use super::sql::quote;

pub const DONE: &[&str] = &["published", "complete", "completed", "done", "paid", "success", "successful", "succeeded", "active",
    "approved", "confirmed", "delivered", "closed", "finished", "applied", "committed", "fulfilled",
    "settled", "accepted", "resolved", "shipped", "sent", "ok"];
pub const FAILED: &[&str] = &["failed", "failure", "error", "errored", "cancelled", "canceled", "rejected", "refunded", "declined",
    "expired", "timeout", "timedout", "lost", "aborted", "bounced", "retracted", "denied", "void", "voided",
    "overdue", "defaulted", "default", "late", "unpaid", "missed", "noshow", "no-show", "no_show", "disputed",
    "chargeback", "returned"];
const STATUSY: &[&str] = &["status", "state", "stage", "result", "outcome", "resolution"];
const REPLACED: &[&str] = &["merged", "replaced", "duplicate", "superseded"];
const WINDOWS: [i64; 2] = [7, 30];
const MONEYTAB: &[&str] = &["payment", "order", "invoice", "transaction", "sale", "booking", "rental", "purchase", "charge", "bill",
    "billing", "subscription", "reimbursement", "claim", "loan", "expense", "donation", "deposit"];
const PEOPLE: &[&str] = &["user", "account", "member", "customer", "client", "person", "people", "player", "student", "author",
    "owner", "subscriber", "patient", "employee", "participant", "guest", "contact", "profile", "buyer",
    "seller", "driver", "rider", "tenant", "learner", "teacher", "candidate", "applicant", "visitor"];
const MONEYNAME: [&[&str]; 2] = [
    &["amount", "total", "price", "paid", "revenue", "value", "payment", "sales", "income", "award", "cost"],
    &["fee", "charge", "fare", "salary", "wage", "budget", "balance"],
];
const NOTMONEY: &[&str] = &["tax", "discount", "rate", "percent", "qty", "quantity", "count"];

#[derive(Debug, Clone, Default)]
pub struct Tile {
    pub title: String,
    pub sql: Option<String>,
    pub of: Option<(String, String)>,
    pub value: Option<f64>,
    pub error: Option<String>,
    pub kind: &'static str,
    pub parts: Vec<String>,
    pub what: &'static str,
    pub table: String,
    pub column: Option<String>,
    pub days: Option<i64>,
    /// what the value is when it is not the whole's count: a sample, the catalog
    pub estimate: Option<String>,
}

fn tile(title: String, sql: String, what: &'static str, table: &str) -> Tile {
    Tile { title, sql: Some(sql), kind: "number", what, table: table.to_string(), ..Default::default() }
}

/// A table's live rows: soft deletion, and a row merged into another.
pub fn live_rows(tb: &Table, engine: &str) -> Vec<String> {
    let e = |n: &str| quote(engine, n);
    let mut out = Vec::new();
    for (c, how) in alive(tb) {
        let r = format!("b.{}", e(&tb.columns[c].name));
        out.push(if how == "null" {
            format!("{r} IS NULL")
        } else if engine == "mysql" {
            format!("COALESCE({r}, 0) = 0")
        } else {
            format!("NOT COALESCE({r}::int::boolean, false)")
        });
    }
    for &(c, p) in &tb.out {
        let col = &tb.columns[c];
        if p == tb.t && has(&col.tk, REPLACED) {
            out.push(format!("b.{} IS NULL", e(&col.name)));
        }
    }
    out
}

pub fn money_column(tb: &Table) -> Option<&Column> {
    let mut best: Option<(usize, usize, &Column)> = None;
    for c in &tb.columns {
        if c.fk.is_some() || c.pk || !["decimal", "float", "int"].contains(&c.fam) {
            continue;
        }
        let name: String = c.tk.concat();
        if NOTMONEY.iter().any(|w| name.contains(w)) {
            continue;
        }
        let mut tier = MONEYNAME.iter().position(|words| words.iter().any(|w| name.contains(w)));
        if c.measure == Some("money") {
            tier = Some(tier.unwrap_or(0));
        }
        if let Some(t) = tier {
            if !(c.fam == "int" && c.measure != Some("money") && t > 0) && best.is_none_or(|b| (t, c.c) < (b.0, b.1)) {
                best = Some((t, c.c, c));
            }
        }
    }
    best.map(|b| b.2)
}

fn since(engine: &str, days: i64) -> String {
    if engine == "mysql" { format!("NOW() - INTERVAL {days} DAY") } else { format!("NOW() - INTERVAL '{days} days'") }
}

pub fn created(tb: &Table) -> Option<&Column> {
    tb.columns.iter().find(|c| c.time == Some("created"))
}

pub fn when_of(tb: &Table) -> Option<&Column> {
    created(tb).or_else(|| tb.columns.iter().find(|c| c.time == Some("event")))
}

pub fn statuses(tb: &Table) -> Vec<&Column> {
    tb.columns
        .iter()
        .filter(|c| c.fk.is_none() && !c.pk && !["int", "decimal", "float"].contains(&c.fam) && matches!(c.category, Some("typed") | Some("named") | None) && has(&c.tk, STATUSY))
        .collect()
}

fn rstrip_s(w: &str) -> String {
    w.trim_end_matches('s').to_string()
}

pub fn named_people(tb: &Table) -> bool {
    has(&tb.tk, PEOPLE) || tb.tk.iter().any(|w| has(&[rstrip_s(w)], PEOPLE))
}

pub fn people(tb: &Table) -> bool {
    tb.person || named_people(tb)
}

/// Keys round 1c's reader left unguessed, by a column named for a table.
pub fn guess_links(schema: &mut Schema) {
    let mut names: HashMap<String, usize> = HashMap::new();
    for tb in &schema.tables {
        names.insert(singular(&tb.tk.concat()), tb.t);
    }
    let mut more: HashMap<String, usize> = HashMap::new();
    for tb in &schema.tables {
        if let Some(last) = tb.tk.last() {
            let k = singular(last);
            if !names.contains_key(&k) {
                more.insert(k, tb.t);
            }
        }
    }
    names.extend(more);
    let mut links = Vec::new();
    for tb in &schema.tables {
        for c in &tb.columns {
            if c.pk || c.fk.is_some() || !(c.fam == "int" || c.fam == "text") || c.tk.len() < 2 || !(c.tk[0] == "id" || c.tk[c.tk.len() - 1] == "id") {
                continue;
            }
            let rest = if c.tk[0] == "id" { &c.tk[1..] } else { &c.tk[..c.tk.len() - 1] };
            if let Some(&p) = names.get(&singular(&rest.concat())) {
                if p != tb.t {
                    links.push((tb.t, c.c, p));
                }
            }
        }
    }
    for (t, c, p) in links {
        let col = &mut schema.tables[t].columns[c];
        col.fk = Some(p);
        col.fk_guessed = true;
        schema.tables[t].out.push((c, p));
        schema.tables[p].referenced_by.push((t, c));
    }
}

pub struct Found {
    pub actor: usize,
    pub object: Option<usize>,
    pub via: Option<usize>,
    pub link: Option<usize>,
}

pub fn actor_and_object(schema: &Schema) -> Option<Found> {
    let tabs: Vec<&Table> = schema.tables.iter().filter(|tb| !tb.boilerplate && tb.role != "lookup" && tb.role != "link").collect();
    let pointing: Vec<HashSet<usize>> = schema.tables.iter().map(|tb| tb.referenced_by.iter().map(|x| x.0).filter(|&c| c != tb.t).collect()).collect();
    let timed_pointing: Vec<usize> = pointing.iter().map(|kids| kids.iter().filter(|&&c| schema.tables[c].timed).count()).collect();
    let rank = |tb: &Table| (timed_pointing[tb.t], pointing[tb.t].len(), tb.rows.unwrap_or(0.0));
    let folk: Vec<&Table> = tabs.iter().copied().filter(|tb| people(tb) && !pointing[tb.t].is_empty()).collect();
    let pool: Vec<&Table> = if !folk.is_empty() {
        folk
    } else {
        tabs.iter().copied().filter(|tb| tb.rows.is_none_or(|r| r >= 10.0) && !pointing[tb.t].is_empty()).collect()
    };
    // Python's max: the first of the largest
    let mut actor: Option<&Table> = None;
    for tb in pool {
        if actor.is_none_or(|a| greater(rank(tb), rank(a))) {
            actor = Some(tb);
        }
    }
    let actor = actor?;
    let mut best: Option<((usize, f64), usize, usize)> = None;
    for tb in &tabs {
        if tb.t == actor.t || named_people(tb) || !tb.timed {
            continue;
        }
        let mut via: Vec<usize> = tb.out.iter().filter(|x| x.1 == actor.t).map(|x| x.0).collect();
        if via.len() > 1 {
            let mut words: Vec<&str> = vec!["owner", "creator", "created", "author", "user", "customer", "client", "submitted", "by", "from", "member"];
            words.extend(actor.tk.iter().map(String::as_str));
            let maker: Vec<usize> = via.iter().copied().filter(|&c| has(&tb.columns[c].tk, &words)).collect();
            via = if !maker.is_empty() { vec![maker[0]] } else { vec![via[0]] };
        }
        if via.len() == 1 {
            let key = (pointing[tb.t].len(), tb.rows.unwrap_or(0.0));
            if best.is_none_or(|b| greater2(key, b.0)) {
                best = Some((key, tb.t, via[0]));
            }
        }
    }
    if let Some((_, o, v)) = best {
        return Some(Found { actor: actor.t, object: Some(o), via: Some(v), link: None });
    }
    let mut two: Option<((usize, f64), usize, usize, usize)> = None;
    for tb in &schema.tables {
        if tb.timed || tb.boilerplate {
            continue;
        }
        let mine: Vec<usize> = tb.out.iter().filter(|x| x.1 == actor.t).map(|x| x.0).collect();
        let theirs: Vec<usize> = tb.out.iter().filter(|x| x.1 != actor.t && schema.tables[x.1].timed && !named_people(&schema.tables[x.1])).map(|x| x.1).collect();
        if !mine.is_empty() && !theirs.is_empty() {
            let mut target = theirs[0];
            for &p in &theirs[1..] {
                if greater2((pointing[p].len(), schema.tables[p].rows.unwrap_or(0.0)), (pointing[target].len(), schema.tables[target].rows.unwrap_or(0.0))) {
                    target = p;
                }
            }
            let key = (pointing[target].len(), tb.rows.unwrap_or(0.0));
            if two.is_none_or(|t| greater2(key, t.0)) {
                two = Some((key, target, mine[0], tb.t));
            }
        }
    }
    Some(match two {
        Some((_, target, via, link)) => Found { actor: actor.t, object: Some(target), via: Some(via), link: Some(link) },
        None => Found { actor: actor.t, object: None, via: None, link: None },
    })
}

fn greater(a: (usize, usize, f64), b: (usize, usize, f64)) -> bool {
    (a.0, a.1) > (b.0, b.1) || ((a.0, a.1) == (b.0, b.1) && a.2 > b.2)
}

fn greater2(a: (usize, f64), b: (usize, f64)) -> bool {
    a.0 > b.0 || (a.0 == b.0 && a.1 > b.1)
}

/// A status's values, most rows first.
pub fn probe(run: &Runner, engine: &str, tb: &Table, col: &Column) -> Vec<(String, f64)> {
    let e = |n: &str| quote(engine, n);
    let (rows, err) = run.run(&format!("SELECT b.{}, COUNT(*) FROM {} b GROUP BY 1 ORDER BY 2 DESC LIMIT 30", e(&col.name), e(&tb.name)));
    if err.is_some() {
        return vec![];
    }
    rows.into_iter().filter_map(|r| match (r.first().cloned().flatten(), r.get(1).cloned().flatten()) {
        (Some(v), Some(n)) => Some((v, n.parse().unwrap_or(0.0))),
        _ => None,
    }).collect()
}

fn in_list(engine: &str, col: &Column, vals: &[String]) -> String {
    format!("b.{} IN ({})", quote(engine, &col.name), vals.iter().map(|v| format!("'{}'", v.replace('\'', "''"))).collect::<Vec<_>>().join(", "))
}

fn done_of(run: &Runner, engine: &str, tb: &Table) -> Vec<String> {
    for st in statuses(tb) {
        let vals = probe(run, engine, tb, st);
        let hit: Vec<String> = vals.iter().filter(|(v, _)| DONE.contains(&v.to_lowercase().as_str())).map(|(v, _)| v.clone()).collect();
        if !hit.is_empty() && hit.len() < vals.len() {
            return vec![in_list(engine, st, &hit)];
        }
    }
    vec![]
}

fn wh(conds: &[String]) -> String {
    if conds.is_empty() { String::new() } else { format!(" WHERE {}", conds.join(" AND ")) }
}

pub fn row(schema: &Schema, run: &Runner) -> Vec<Tile> {
    let mut schema = schema.clone();
    guess_links(&mut schema);
    let schema = &schema;
    let engine = schema.engine.as_str();
    let e = |n: &str| quote(engine, n);
    let mut tiles: Vec<Tile> = Vec::new();
    let Some(found) = actor_and_object(schema) else { return tiles };
    let actor = &schema.tables[found.actor];
    let a = e(&actor.name);
    let a_live = live_rows(actor, engine);
    let total_a = format!("{} in all", actor.name);
    tiles.push(tile(total_a.clone(), format!("SELECT COUNT(*) FROM {a} b{}", wh(&a_live)), "count", &actor.name));
    if let Some(o) = found.object {
        let obj = &schema.tables[o];
        let oq = e(&obj.name);
        let o_live = live_rows(obj, engine);
        let done = done_of(run, engine, obj);
        let label = format!("{}{}", obj.name, if done.is_empty() { "" } else { " done" });
        let lw = |extra: &[String]| -> Vec<String> { o_live.iter().chain(done.iter()).cloned().chain(extra.iter().cloned()).collect() };
        tiles.push(tile(format!("{label} in all"), format!("SELECT COUNT(*) FROM {oq} b{}", wh(&lw(&[]))), "count", &obj.name));
        let o_when = when_of(obj);
        if let Some(w) = o_when {
            for d in WINDOWS {
                let mut t = tile(format!("{label}, {d} days"), format!("SELECT COUNT(*) FROM {oq} b{}", wh(&lw(&[format!("b.{} >= {}", e(&w.name), since(engine, d))]))), "count", &obj.name);
                t.days = Some(d);
                tiles.push(t);
            }
        }
        let actors = format!("{} with {label}", actor.name);
        let again = format!("{} with two {label} or more", actor.name);
        match found.link {
            None => {
                let fname = obj.columns[found.via.unwrap()].name.clone();
                let f = e(&fname);
                let mut t = tile(actors.clone(), format!("SELECT COUNT(DISTINCT b.{f}) FROM {oq} b{}", wh(&lw(&[]))), "count distinct", &obj.name);
                t.column = Some(fname.clone());
                tiles.push(t);
                let mut t = tile(again.clone(), format!("SELECT COUNT(*) FROM (SELECT b.{f} FROM {oq} b{} GROUP BY b.{f} HAVING COUNT(*) >= 2) x", wh(&lw(&[format!("b.{f} IS NOT NULL")]))), "two or more", &obj.name);
                t.column = Some(fname.clone());
                tiles.push(t);
                if let Some(w) = o_when {
                    let mut t = tile(format!("{} active, 30 days", actor.name), format!("SELECT COUNT(DISTINCT b.{f}) FROM {oq} b{}", wh(&lw(&[format!("b.{} >= {}", e(&w.name), since(engine, 30))]))), "count distinct", &obj.name);
                    t.column = Some(fname);
                    t.days = Some(30);
                    tiles.push(t);
                }
            }
            Some(l) => {
                let link = &schema.tables[l];
                let fname = link.columns[found.via.unwrap()].name.clone();
                let f = e(&fname);
                let lq = e(&link.name);
                let mut t = tile(actors.clone(), format!("SELECT COUNT(DISTINCT b.{f}) FROM {lq} b"), "count distinct", &link.name);
                t.column = Some(fname.clone());
                tiles.push(t);
                let mut t = tile(again.clone(), format!("SELECT COUNT(*) FROM (SELECT b.{f} FROM {lq} b WHERE b.{f} IS NOT NULL GROUP BY b.{f} HAVING COUNT(*) >= 2) x"), "two or more", &link.name);
                t.column = Some(fname);
                tiles.push(t);
            }
        }
        let prev = tiles[tiles.len() - 2].clone();
        tiles.push(Tile { title: format!("share of {} with {label}", actor.name), of: Some((actors.clone(), total_a.clone())), kind: "share", what: "share", table: prev.table, column: prev.column, ..Default::default() });
        let prev = tiles[tiles.len() - 2].clone();
        tiles.push(Tile { title: "share of them with two or more".into(), of: Some((again.clone(), actors.clone())), kind: "share", what: "share", table: prev.table, column: prev.column, ..Default::default() });
        tiles.push(Tile { title: format!("{} → with {label} → with two or more", actor.name), kind: "funnel", parts: vec![total_a.clone(), actors, again], what: "funnel", table: actor.name.clone(), ..Default::default() });
    }
    if let Some(made) = created(actor) {
        for d in WINDOWS {
            let conds: Vec<String> = a_live.iter().cloned().chain([format!("b.{} >= {}", e(&made.name), since(engine, d))]).collect();
            let mut t = tile(format!("new {}, {d} days", actor.name), format!("SELECT COUNT(*) FROM {a} b{}", wh(&conds)), "count", &actor.name);
            t.days = Some(d);
            tiles.push(t);
        }
    }
    // what it earned, the last month: the thing's own money, else a table of payments or orders, else the busiest
    let earns = |t: &Table| -> (u8, f64) {
        let tier = if found.object == Some(t.t) { 0 } else if has(&t.tk, MONEYTAB) || t.tk.iter().any(|w| has(&[rstrip_s(w)], MONEYTAB)) { 1 } else { 2 };
        (tier, -t.rows.unwrap_or(0.0))
    };
    let mut timed: Vec<&Table> = schema.tables.iter().filter(|t| t.timed && !t.boilerplate).collect();
    timed.sort_by(|a, b| { let (x, y) = (earns(a), earns(b)); x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)) });
    for tb in timed {
        let (Some(money), Some(when)) = (money_column(tb), when_of(tb)) else { continue };
        let done = done_of(run, engine, tb);
        let conds: Vec<String> = live_rows(tb, engine).into_iter().chain(done).chain([format!("b.{} >= {}", e(&when.name), since(engine, 30))]).collect();
        let mut t = tile(format!("{}.{}, 30 days", tb.name, money.name), format!("SELECT SUM(b.{}) FROM {} b{}", e(&money.name), e(&tb.name), wh(&conds)), "sum", &tb.name);
        t.column = Some(money.name.clone());
        t.days = Some(30);
        tiles.push(t);
        if found.object != Some(tb.t) || tiles.iter().filter(|t| t.what == "sum").count() >= 2 {
            break;
        }
    }
    // what failed, the last week
    let mut busiest: Vec<&Table> = schema.tables.iter().filter(|t| t.timed && !t.boilerplate).collect();
    busiest.sort_by(|a, b| (-a.rows.unwrap_or(0.0)).total_cmp(&-b.rows.unwrap_or(0.0)));
    'tables: for tb in busiest {
        let when = when_of(tb);
        for st in statuses(tb) {
            let vals = probe(run, engine, tb, st);
            let bad: Vec<String> = vals.iter().filter(|(v, _)| FAILED.contains(&v.to_lowercase().as_str())).map(|(v, _)| v.clone()).collect();
            if !bad.is_empty() {
                if let Some(w) = when {
                    let cond = in_list(engine, st, &bad);
                    let conds: Vec<String> = live_rows(tb, engine).into_iter().chain([format!("b.{} >= {}", e(&w.name), since(engine, 7))]).collect();
                    let show: Vec<&str> = bad.iter().take(3).map(String::as_str).collect();
                    let mut t = tile(format!("share of {} {}, 7 days", tb.name, show.join("/")), format!("SELECT AVG(CASE WHEN {cond} THEN 1.0 ELSE 0 END) FROM {} b{}", e(&tb.name), wh(&conds)), "share", &tb.name);
                    t.kind = "share";
                    t.column = Some(st.name.clone());
                    t.days = Some(7);
                    tiles.push(t);
                    break 'tables;
                }
            }
        }
    }
    // run them, then the shares of what ran
    for t in tiles.iter_mut() {
        if let Some(sql) = &t.sql {
            let (rows, err) = run.run(sql);
            t.value = match rows.first().and_then(|r| r.first()).cloned().flatten().filter(|v| !v.is_empty()) {
                Some(v) => v.parse().ok(),
                None => if err.is_some() { None } else { Some(0.0) },
            };
            t.error = err;
            t.estimate = run.note(sql);
        }
    }
    let values: HashMap<String, Option<f64>> = tiles.iter().map(|t| (t.title.clone(), t.value)).collect();
    let estimated: HashMap<String, bool> = tiles.iter().map(|t| (t.title.clone(), t.estimate.is_some())).collect();
    for t in tiles.iter_mut() {
        // a share of an estimate, or a funnel through one, is an estimate too
        let parts: Vec<&String> = match &t.of {
            Some((top, bottom)) => vec![top, bottom],
            None => t.parts.iter().collect(),
        };
        if t.estimate.is_none() && t.kind != "number" && parts.iter().any(|p| estimated.get(*p).copied().unwrap_or(false)) {
            t.estimate = Some("from estimates".into());
        }
        if let Some((top, bottom)) = &t.of {
            t.value = match (values.get(top).copied().flatten(), values.get(bottom).copied().flatten()) {
                (Some(a), Some(b)) if b != 0.0 => Some(a / b),
                _ => None,
            };
        }
        if t.kind == "funnel" {
            t.value = None;
        }
        // a share left without a value is not an estimate of one
        if t.value.is_none() && t.kind == "share" {
            t.estimate = None;
        }
    }
    tiles
}

fn commas(x: f64) -> String {
    let s = format!("{x:.0}");
    let (sign, digits) = if let Some(d) = s.strip_prefix('-') { ("-", d) } else { ("", s.as_str()) };
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{sign}{out}")
}

/// A tile as `kpi.shown` writes it.
pub fn shown(t: &Tile, tiles: &[Tile]) -> String {
    let value_of = |title: &str| tiles.iter().find(|x| x.title == title).and_then(|x| x.value);
    if t.kind == "funnel" {
        return t.parts.iter().map(|p| value_of(p).map(|v| format!("{v:.0}")).unwrap_or_else(|| "?".into())).collect::<Vec<_>>().join(" → ");
    }
    match t.value {
        None => match &t.error {
            None => "—".into(),
            // the bound's refusals say "not run" themselves
            Some(e) => format!("not run: {}", e.strip_prefix("not run: ").unwrap_or(e).chars().take(60).collect::<String>()),
        },
        Some(v) if t.kind == "share" => format!("{:.1}%", 100.0 * v),
        Some(v) if v.abs() >= 10.0 => commas(v),
        Some(v) => format!("{v:.2}"),
    }
}
