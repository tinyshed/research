//! Round 1o's product charts and its screen, and every panel run as the
//! rounds ran it: round 1c's `see.panel_of` (a bucket by the time's span),
//! round 1m's last 28 days by day and a speed in its unit, round 1o's charts
//! with their shares in percent.

use std::collections::HashMap;

use regex::Regex;

use super::model::Model;
use super::names::has;
use super::numbers::{FAILED, live_rows, statuses, when_of};
use super::question::{Question, identity};
use super::rank::{self, order};
use super::roles::{self, timed_role};
use super::run::{Rows, Runner};
use super::schema::{Column, Schema, Table};
use super::sql::{compile, quote};

const DAYS: i64 = 28;
const WEEK: i64 = 7;
const KINDISH: &[&str] = &["kind", "type", "category", "channel", "method", "plan", "tier", "source", "provider", "model", "class",
    "mode", "segment", "group", "product", "service"];

#[derive(Debug, Clone)]
pub struct Chart {
    pub title: String,
    pub sql: String,
    pub timed: bool,
    pub template: &'static str,
    pub unit: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Q(Question),
    C(Chart),
}

fn last(engine: &str, tb: &Table, col: &Column, days: i64) -> String {
    let e = |n: &str| quote(engine, n);
    let since = if engine == "mysql" { format!("INTERVAL {days} DAY") } else { format!("INTERVAL '{days} days'") };
    format!("b.{} >= (SELECT MAX(m.{}) FROM {} m) - {since}", e(&col.name), e(&col.name), e(&tb.name))
}

fn day(engine: &str, r: &str) -> String {
    if engine == "mysql" { format!("DATE({r})") } else { format!("date_trunc('day', {r})::date") }
}

fn busiest(schema: &Schema) -> Vec<&Table> {
    let mut v: Vec<&Table> = schema.tables.iter().filter(|t| t.timed && !t.boilerplate).collect();
    v.sort_by(|a, b| (-a.rows.unwrap_or(0.0)).total_cmp(&-b.rows.unwrap_or(0.0)));
    v
}

pub fn failed_share(schema: &Schema, run: &Runner, q: Option<&Question>) -> Option<Chart> {
    let engine = schema.engine.as_str();
    let e = |n: &str| quote(engine, n);
    let pairs: Vec<(&Table, &Column, &Column)> = match q {
        Some(q) if q.by.as_ref().is_some_and(|b| b.c >= 0 && b.t == q.base) && q.time.is_some() => {
            let (b, t) = (q.by.as_ref().unwrap(), q.time.as_ref().unwrap());
            vec![(&schema.tables[q.base], &schema.tables[q.base].columns[b.c as usize], &schema.tables[t.t].columns[t.c as usize])]
        }
        _ => busiest(schema).into_iter().flat_map(|tb| statuses(tb).into_iter().filter_map(move |st| when_of(tb).map(|w| (tb, st, w)))).collect(),
    };
    for (tb, st, when) in pairs {
        let vals = super::numbers::probe(run, engine, tb, st);
        let bad: Vec<String> = vals.iter().filter(|(v, _)| FAILED.contains(&v.to_lowercase().as_str())).map(|(v, _)| v.clone()).collect();
        if !bad.is_empty() && bad.len() < vals.len() {
            let cond = format!("b.{} IN ({})", e(&st.name), bad.iter().map(|v| format!("'{}'", v.replace('\'', "''"))).collect::<Vec<_>>().join(", "));
            let mut conds = live_rows(tb, engine);
            conds.push(last(engine, tb, when, DAYS));
            let sql = format!("SELECT {} AS t, AVG(CASE WHEN {cond} THEN 1.0 ELSE 0 END) AS v FROM {} b WHERE {} GROUP BY 1 ORDER BY 1",
                day(engine, &format!("b.{}", e(&when.name))), e(&tb.name), conds.join(" AND "));
            let show: Vec<&str> = bad.iter().take(3).map(String::as_str).collect();
            return Some(Chart { title: format!("share of {} {} a day", tb.name, show.join("/")), sql, timed: true, template: "T3", unit: Some("share") });
        }
    }
    None
}

pub fn money_by_kind(schema: &Schema) -> Option<Chart> {
    let engine = schema.engine.as_str();
    let e = |n: &str| quote(engine, n);
    for tb in busiest(schema) {
        let money = tb.columns.iter().find(|c| c.measure == Some("money") && c.fk.is_none());
        let (Some(money), Some(when)) = (money, when_of(tb)) else { continue };
        let kinds: Vec<&Column> = tb.columns.iter().filter(|c| c.fk.is_none() && !c.pk && !["int", "decimal", "float"].contains(&c.fam) && has(&c.tk, KINDISH)
            && c.distinct.is_none_or(|d| (2.0..=30.0).contains(&d.abs()) || (-1.0 < d && d < 0.0))).collect();
        let Some(k) = kinds.first() else { continue };
        let mut conds = live_rows(tb, engine);
        conds.push(last(engine, tb, when, WEEK));
        let sql = format!("SELECT b.{} AS g, SUM(b.{}) AS v FROM {} b WHERE {} GROUP BY 1 ORDER BY 2 DESC LIMIT 12", e(&k.name), e(&money.name), e(&tb.name), conds.join(" AND "));
        return Some(Chart { title: format!("sum of {}.{} by {}, the last week", tb.name, money.name, k.name), sql, timed: false, template: "T4", unit: None });
    }
    None
}

const STARTISH: &[&str] = &["start", "started", "begin", "began", "created", "requested", "submitted", "opened", "sent", "placed"];
const ENDISH: &[&str] = &["end", "ended", "completed", "finished", "closed", "resolved", "delivered", "returned", "done", "answered", "approved", "shipped", "paid"];

pub fn p95_delay(schema: &Schema, q: Option<&Question>) -> Option<Chart> {
    let engine = schema.engine.as_str();
    if engine == "mysql" {
        return None;
    }
    let e = |n: &str| quote(engine, n);
    let cands: Vec<(&Table, &Column, &Column)> = match q {
        Some(q) if q.measure.kind == "gap" => {
            let ((t0, c0), (t1, c1)) = (q.measure.cols[0], q.measure.cols[1]);
            vec![(&schema.tables[t0], &schema.tables[t0].columns[c0], &schema.tables[t1].columns[c1])]
        }
        _ => busiest(schema).into_iter().filter_map(|tb| {
            let times: Vec<&Column> = tb.columns.iter().filter(|c| (c.fam == "timestamp" || c.fam == "date") && matches!(c.time, Some("created") | Some("event") | None)).collect();
            let start = times.iter().find(|c| has(&c.tk, STARTISH)).copied()?;
            let end = times.iter().find(|c| has(&c.tk, ENDISH) && c.c != start.c).copied()?;
            Some((tb, start, end))
        }).collect(),
    };
    let (tb, start, end) = cands.into_iter().next()?;
    let (x, y) = (format!("b.{}", e(&start.name)), format!("b.{}", e(&end.name)));
    let mut conds = live_rows(tb, engine);
    conds.extend([format!("{y} IS NOT NULL"), format!("{y} >= {x}"), last(engine, tb, start, DAYS)]);
    let sql = format!("SELECT {} AS t, percentile_cont(0.95) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM ({y}::timestamp - {x}::timestamp))) AS v FROM {} b WHERE {} GROUP BY 1 ORDER BY 1",
        day(engine, &x), e(&tb.name), conds.join(" AND "));
    Some(Chart { title: format!("95th percentile of seconds from {}.{} to {} a day", tb.name, start.name, end.name), sql, timed: true, template: "T3", unit: Some("seconds") })
}

/// The ranked questions of a schema, with their scores and slots.
pub struct Ranked {
    pub qs: Vec<Question>,
    pub scores: Vec<f64>,
    pub ids: Vec<super::question::Identity>,
    pub slots: Vec<HashMap<&'static str, Vec<String>>>,
}

pub fn ranked(schema: &Schema, model: &Model) -> Ranked {
    let qs: Vec<Question> = super::generate::generate(schema).into_iter().filter(|q| rank::veto(schema, q).is_none()).collect();
    let ctx = rank::context(schema);
    let scores: Vec<f64> = qs.iter().map(|q| model.score(&rank::features(schema, &ctx, q)) + rank::stats_points(schema, q)).collect();
    let ids = qs.iter().map(identity).collect();
    let slots = qs.iter().map(|q| rank::slots(schema, q)).collect();
    Ranked { qs, scores, ids, slots }
}

/// Round 1o's screen, and round 1m's it was made from.
pub fn screen(schema: &Schema, r: &Ranked, run: &Runner) -> (Vec<Item>, Vec<Question>) {
    let mut base: Vec<Question> = roles::screen(schema, &r.qs, &r.ids, &r.scores, &r.slots, 5).into_iter().map(|i| r.qs[i].clone()).collect();
    let round_1m = base.clone();
    let mut role_of: Vec<Option<&str>> = base.iter().map(|q| timed_role(&identity(q), &rank::slots(schema, q))).collect();
    let score_of = |q: &Question| r.qs.iter().position(|x| x == q).map(|i| r.scores[i]).unwrap_or(f64::MIN);
    for k in 0..base.len() {
        let q = base[k].clone();
        if role_of[k] != Some("pulse") || q.time.is_none() {
            continue;
        }
        let splits: Vec<&Question> = r.qs.iter().filter(|x| x.base == q.base && x.time == q.time && x.shape == "split" && x.measure.kind == "rows"
            && x.by.as_ref().is_some_and(|b| b.c >= 0 && has(&schema.tables[b.t].columns[b.c as usize].tk, KINDISH))).collect();
        if !splits.is_empty() {
            // Python's max: the first of the largest (another table's kind first, then the score)
            let mut best = splits[0];
            for &x in &splits[1..] {
                let (a, b) = ((x.by.as_ref().unwrap().t != q.base, score_of(x)), (best.by.as_ref().unwrap().t != q.base, score_of(best)));
                if a.0 > b.0 || (a.0 == b.0 && a.1 > b.1) {
                    best = x;
                }
            }
            base[k] = best.clone();
            role_of[k] = Some("pulse");
        }
        break;
    }
    let mut kinds = money_by_kind(schema);
    let mut out: Vec<(Item, Option<&str>)> = Vec::new();
    for (q, role) in base.into_iter().zip(role_of) {
        let item = match role {
            Some("health") => failed_share(schema, run, Some(&q)).map(Item::C),
            Some("speed") => p95_delay(schema, Some(&q)).map(Item::C),
            Some("people") => kinds.take().map(Item::C),
            _ => None,
        };
        out.push((item.unwrap_or(Item::Q(q)), role));
    }
    if let Some(k) = kinds {
        if let Some(slot) = out.iter_mut().rev().find(|(i, r)| matches!(i, Item::Q(_)) && !matches!(r, Some("pulse") | Some("money") | Some("health") | Some("speed"))) {
            slot.0 = Item::C(k);
        }
    }
    (out.into_iter().map(|x| x.0).collect(), round_1m)
}

/// A panel, run: its rows as text, its title, bucket and template.
#[derive(Debug, Clone)]
pub struct Ran {
    pub title: String,
    pub template: &'static str,
    pub bucket: Option<String>,
    pub rows: Rows,
    pub error: Option<String>,
    pub sql: String,
}

pub fn template_of(q: &Question) -> &'static str {
    match q.shape {
        "series" => if q.measure.kind == "rows" { "T1" } else { "T3" },
        "split" => "T6",
        "distribution" => "T5",
        "top" => "T9",
        _ => if q.measure.kind == "rows" { "T2" } else { "T4" },
    }
}

fn date_days(v: &str) -> Option<f64> {
    if v.len() < 10 {
        return None;
    }
    let (y, m, d) = (v.get(0..4)?.parse::<f64>().ok()?, v.get(5..7)?.parse::<f64>().ok()?, v.get(8..10)?.parse::<f64>().ok()?);
    Some(y * 365.25 + m * 30.4 + d)
}

fn bucket_for(span: Option<f64>) -> &'static str {
    match span {
        None => "month",
        Some(s) if s <= 90.0 => "day",
        Some(s) if s <= 420.0 => "week",
        Some(s) if s <= 6.0 * 366.0 => "month",
        _ => "year",
    }
}

fn ten(r: Vec<Option<String>>) -> Vec<Option<String>> {
    let mut r = r;
    if let Some(first) = r.first_mut() {
        *first = Some(first.clone().unwrap_or_default().chars().take(10).collect());
    }
    r
}

/// Round 1c's `see.panel_of`: the bucket by the time's span.
fn standard(schema: &Schema, q: &Question, run: &Runner) -> Ran {
    let engine = schema.engine.as_str();
    let mut bucket: Option<String> = None;
    if let Some(t) = &q.time {
        let col = &schema.tables[t.t].columns[t.c as usize];
        if col.fam == "timestamp" || col.fam == "date" {
            let e = |n: &str| quote(engine, n);
            let (rows, _) = run.run(&format!("SELECT min({}), max({}) FROM {}", e(&col.name), e(&col.name), e(&schema.tables[t.t].name)));
            let (lo, hi) = rows.first().map(|r| (r.first().cloned().flatten(), r.get(1).cloned().flatten())).unwrap_or((None, None));
            let span = match (lo.as_deref().and_then(date_days), hi.as_deref().and_then(date_days)) {
                (Some(a), Some(b)) => Some(b - a),
                _ => None,
            };
            bucket = Some(bucket_for(span).to_string());
        } else {
            bucket = Some(if col.tk.iter().any(|t| t.contains("year")) { "year" } else { "month" }.to_string());
        }
    }
    let sql = compile(schema, q, engine, bucket.as_deref());
    let (rows, err) = run.run(&sql);
    let rows: Rows = match &q.time {
        Some(t) if !matches!(schema.tables[t.t].columns[t.c as usize].fam, "timestamp" | "date") => rows.into_iter().map(|mut r| {
            let v = r.first().cloned().flatten().unwrap_or_default();
            if v.len() == 4 && v.chars().all(|c| c.is_ascii_digit()) {
                r[0] = Some(format!("{v}-01-01"));
            } else if v.len() == 6 && v.chars().all(|c| c.is_ascii_digit()) {
                r[0] = Some(format!("{}-{}-01", &v[..4], &v[4..]));
            }
            r
        }).collect(),
        Some(_) if !rows.is_empty() => rows.into_iter().map(ten).collect(),
        _ => rows,
    };
    Ran { title: q.title(schema), template: template_of(q), bucket, rows: rows.into_iter().take(500).collect(), error: err, sql }
}

/// Round 1m's last DAYS days of the time a panel is drawn over.
fn recent(sql: &str, schema: &Schema, q: &Question) -> String {
    let engine = schema.engine.as_str();
    let t = q.time.as_ref().unwrap();
    let tb = &schema.tables[t.t];
    let col = &tb.columns[t.c as usize];
    let re = Regex::new(&format!(r"(\w+\.{}) IS NOT NULL", regex::escape(&quote(engine, &col.name)))).unwrap();
    let Some(m) = re.captures(sql) else { return sql.to_string() };
    let whole = m.get(0).unwrap().as_str();
    let r = m.get(1).unwrap().as_str();
    let since = if engine == "mysql" { format!("INTERVAL {DAYS} DAY") } else { format!("INTERVAL '{DAYS} days'") };
    let add = format!("{whole} AND {r} >= (SELECT MAX({}) FROM {}) - {since}", quote(engine, &col.name), quote(engine, &tb.name));
    sql.replacen(whole, &add, 1)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n == 0 { 0.0 } else if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

fn values(rows: &Rows) -> Vec<f64> {
    rows.iter().filter_map(|r| r.last().cloned().flatten().filter(|s| !s.is_empty()).and_then(|s| s.parse::<f64>().ok())).map(f64::abs).collect()
}

/// A question as round 1m's screen ran it on a living database.
pub fn run_question(schema: &Schema, q: &Question, run: &Runner) -> Ran {
    let engine = schema.engine.as_str();
    let mut p = match &q.time {
        Some(t) if matches!(schema.tables[t.t].columns[t.c as usize].fam, "timestamp" | "date") => {
            let sql = recent(&compile(schema, q, engine, Some("day")), schema, q);
            let (rows, err) = run.run(&sql);
            Ran { title: q.title(schema), template: template_of(q), bucket: Some("day".into()), rows: rows.into_iter().map(ten).take(500).collect(), error: err, sql }
        }
        _ => standard(schema, q, run),
    };
    if q.measure.kind == "gap" {
        // in the unit its values need, by their median
        let vals = values(&p.rows);
        if !vals.is_empty() {
            let med = median(vals);
            let (name, k) = [("seconds", 86400.0), ("minutes", 1440.0), ("hours", 24.0), ("days", 1.0)].into_iter().find(|(_, k)| med * k < 120.0).unwrap_or(("days", 1.0));
            if k != 1.0 {
                p.rows = p.rows.into_iter().map(|mut r| {
                    let n = r.len();
                    if let Some(v) = r[n - 1].clone().filter(|s| !s.is_empty()).and_then(|s| s.parse::<f64>().ok()) {
                        r[n - 1] = Some(format!("{:.2}", v * k));
                    }
                    r
                }).collect();
                p.title = p.title.replace("average days from", &format!("average {name} from"));
            }
        }
    }
    p
}

pub fn run_item(schema: &Schema, item: &Item, run: &Runner) -> Ran {
    match item {
        Item::Q(q) => run_question(schema, q, run),
        Item::C(c) => {
            let (rows, err) = run.run(&c.sql);
            let mut rows: Rows = if c.timed { rows.into_iter().map(ten).collect() } else { rows };
            let mut title = c.title.clone();
            match c.unit {
                Some("share") => {
                    rows = rows.into_iter().map(|mut r| {
                        let n = r.len();
                        if let Some(v) = r[n - 1].clone().filter(|s| !s.is_empty()).and_then(|s| s.parse::<f64>().ok()) {
                            r[n - 1] = Some(format!("{:.1}", 100.0 * v));
                        }
                        r
                    }).collect();
                }
                Some("seconds") => {
                    let med = median(values(&rows));
                    let (name, k) = [("seconds", 1.0), ("minutes", 60.0), ("hours", 3600.0)].into_iter().find(|(_, k)| med / k < 120.0).unwrap_or(("days", 86400.0));
                    rows = rows.into_iter().map(|mut r| {
                        let n = r.len();
                        if let Some(v) = r[n - 1].clone().filter(|s| !s.is_empty()).and_then(|s| s.parse::<f64>().ok()) {
                            r[n - 1] = Some(format!("{:.2}", v / k));
                        }
                        r
                    }).collect();
                    title = title.replace("of seconds", &format!("of {name}"));
                }
                _ => {}
            }
            Ran { title, template: c.template, bucket: if c.timed { Some("day".into()) } else { None }, rows: rows.into_iter().take(500).collect(), error: err, sql: c.sql.clone() }
        }
    }
}

pub fn ord(r: &Ranked) -> Vec<usize> {
    order(&r.scores)
}
