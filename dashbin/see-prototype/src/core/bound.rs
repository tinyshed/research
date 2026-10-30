//! What looking may cost a source, chapter 14's bound: its catalog and
//! statistics, at most 10 000 rows a table, inside 2 s a table, never a full
//! scan. Every statement the core writes is planned here before it runs: as
//! it is when it reads no big table; from the catalog when the catalog
//! already knows the answer; over an indexed window when the window is
//! small; else over a sample of the big table's pages, the same pages for
//! every statement, its counts and sums scaled; and not at all when a sample
//! cannot say it, a distinct count above all. PostgreSQL only, as the
//! runners are.

use std::collections::{HashMap, HashSet};

use regex::Regex;
use serde_json::Value;

use super::run::Rows;

/// Rows a statement may read of a table.
pub const ROWS: f64 = 10_000.0;
/// Server time a look may spend on a table.
pub const BUDGET_MS: f64 = 2_000.0;
/// Every sample of a table reads the same pages.
pub const SEED: u32 = 17;
/// What a sample aims at: pages come whole, so below ROWS, and below it
/// again with a window's end read off a sample of its own, or the keys a
/// sample looks up in a big table and that table's own window end.
pub const SAMPLE_ROWS: f64 = 7_000.0;
pub const END_ROWS: f64 = 800.0;
/// A sample whose rows each look a key up in a big table aims lower, so the
/// lookups and that table's window end stay below ROWS too.
pub const JOIN_ROWS: f64 = 5_500.0;

#[derive(Debug, Clone, Default)]
pub struct Info {
    pub rows: f64,
    /// the first column of every index
    pub indexed: HashSet<String>,
    /// a column's distinct values as the statistics count them
    pub distinct: HashMap<String, f64>,
    pub pages: f64,
    /// how far a column's order is the order of the rows on disk, -1 to 1
    pub correlation: HashMap<String, f64>,
    pub schema: String,
    /// a column's type, as a literal of it is cast
    pub types: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub tables: HashMap<String, Info>,
    /// a range of a table's pages can be read alone: PostgreSQL 14 and later
    pub tid_range: bool,
}

impl Catalog {
    /// The catalog as the snapshot read it: rows, indexes, distinct values.
    pub fn from_snapshot(snap: &Value) -> Catalog {
        let feature = |f: &Value, k: &str| f.get("features").and_then(|x| x.get(k)).and_then(|x| x.get("value")).and_then(Value::as_f64);
        let mut tables = HashMap::new();
        for t in snap.get("tables").and_then(Value::as_array).into_iter().flatten() {
            let Some(name) = t.get("name").and_then(Value::as_str) else { continue };
            let rows = feature(t, "rows_estimate").unwrap_or(0.0).max(0.0);
            let indexed = t.get("indexes").and_then(Value::as_array).into_iter().flatten()
                .filter_map(|i| i.get("columns").and_then(Value::as_array).and_then(|c| c.first()).and_then(Value::as_str).map(str::to_string))
                .collect();
            let distinct = t.get("columns").and_then(Value::as_array).into_iter().flatten().filter_map(|c| {
                let n = feature(c, "n_distinct")?;
                // the statistics write a share of the rows as a negative number
                Some((c.get("name")?.as_str()?.to_string(), if n < 0.0 { -n * rows } else { n }))
            }).collect();
            let correlation = t.get("columns").and_then(Value::as_array).into_iter().flatten()
                .filter_map(|c| Some((c.get("name")?.as_str()?.to_string(), feature(c, "correlation")?)))
                .collect();
            let pages = feature(t, "pages").unwrap_or(0.0).max(0.0);
            let schema = t.get("schema").and_then(Value::as_str).unwrap_or("public").to_string();
            let types = t.get("columns").and_then(Value::as_array).into_iter().flatten()
                .filter_map(|c| Some((c.get("name")?.as_str()?.to_string(), c.get("type_name")?.as_str()?.to_string())))
                .collect();
            tables.insert(name.to_string(), Info { rows, indexed, distinct, pages, correlation, schema, types });
        }
        let major = snap.get("server_version").and_then(Value::as_str).and_then(|v| v.split(|c: char| !c.is_ascii_digit()).next()).and_then(|m| m.parse::<u32>().ok()).unwrap_or(0);
        Catalog { tables, tid_range: major >= 14 }
    }

    pub fn rows(&self, t: &str) -> f64 {
        self.tables.get(t).map_or(0.0, |i| i.rows)
    }

    pub fn big(&self, t: &str) -> bool {
        self.rows(t) > ROWS
    }

    fn indexed(&self, t: &str, c: &str) -> bool {
        self.tables.get(t).is_some_and(|i| i.indexed.contains(c))
    }

    /// The type a time column's literal is cast to, when it is a time.
    pub fn time_type(&self, t: &str, c: &str) -> Option<String> {
        let ty = self.tables.get(t)?.types.get(c)?;
        (ty.starts_with("timestamp") || ty == "date").then(|| ty.clone())
    }
}

/// A table a statement names, where, under which alias.
#[derive(Debug, Clone)]
pub struct Use {
    pub table: String,
    pub alias: Option<String>,
    /// where the name and its alias end, for a sample to be written after
    pub end: usize,
    pub joined: bool,
}

pub fn uses(sql: &str) -> Vec<Use> {
    let re = Regex::new(r#"\b(FROM|JOIN)\s+"([^"]+)"(?:\s+([a-z][a-z0-9_]*))?"#).unwrap();
    re.captures_iter(sql).map(|c| Use {
        table: c[2].to_string(),
        alias: c.get(3).map(|a| a.as_str().to_string()),
        end: c.get(0).unwrap().end(),
        joined: &c[1] == "JOIN",
    }).collect()
}

/// What an output column holds, so a sample's answer can be scaled to the whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    /// a key, an average, a percentile: the same in a sample
    Keep,
    /// a count or a sum: a sample's, times the whole over the sample
    Scale,
    /// a count of distinct values: no sample says it
    Distinct,
}

/// The items of a statement's own SELECT list, split at its top-level commas.
pub fn outputs(sql: &str) -> Vec<Out> {
    let body = sql.trim_start();
    let Some(rest) = body.strip_prefix("SELECT ") else { return vec![] };
    let (mut depth, mut items, mut start) = (0i32, Vec::new(), 0usize);
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                items.push(&rest[start..i]);
                start = i + 1;
            }
            b' ' if depth == 0 && rest[i..].starts_with(" FROM ") => break,
            _ => {}
        }
        i += 1;
    }
    items.push(&rest[start..i.min(rest.len())]);
    items.iter().map(|it| {
        let it = it.trim().to_uppercase();
        if it.starts_with("COUNT(DISTINCT") {
            Out::Distinct
        } else if it.starts_with("COUNT(") || it.starts_with("SUM(") {
            Out::Scale
        } else {
            Out::Keep
        }
    }).collect()
}

/// How a statement is to be run.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// as written: it reads no big table
    Exact { sql: String },
    /// answered without reading a row
    Known { rows: Rows, note: String },
    /// the ways to try, in order, each guarded in the database
    Tries(Vec<Try>),
    /// not run, and why
    Refuse(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Try {
    pub sql: String,
    /// the big tables the guard keeps from being read whole
    pub big: Vec<String>,
    /// at most this many rows of a big table, or the try gives way to the next
    pub most: Option<f64>,
    /// what each output column of a sample is multiplied by
    pub scale: Option<(Vec<Out>, f64)>,
    pub note: Option<String>,
    /// every table read, for its time
    pub tables: Vec<String>,
    /// its first bucket is cut short, and not shown
    pub drop_first: bool,
    /// its last column is the count of a sample's own rows behind each answer, not shown
    pub counted: bool,
    /// what the answer must hold to be shown, or the try gives way
    pub fewest: Option<Fewest>,
}

/// Too little to show: a chart of a few days, a sample too thin to say a day,
/// a group or a number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fewest {
    /// at least this many buckets
    Buckets(usize),
    /// a sample's rows a bucket, at the median
    PerBucket(f64),
    /// a sample's rows a group, each group below it left out
    PerGroup(f64),
    /// a sample's rows behind a single number
    Whole(f64),
}

/// Where a statement's own SELECT list ends: its first FROM outside brackets.
pub fn select_end(sql: &str) -> Option<usize> {
    let bytes = sql.as_bytes();
    let mut depth = 0i32;
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b' ' if depth == 0 && sql[i..].starts_with(" FROM ") => return Some(i),
            _ => {}
        }
    }
    None
}

fn pct(p: f64) -> String {
    let s = format!("{p:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The share of a table's pages a sample of about `aim` rows reads.
pub fn share(rows: f64, aim: f64) -> f64 {
    (100.0 * aim / rows.max(1.0)).clamp(0.0001, 100.0)
}

/// A statement with a sample written after each table named where `at`
/// says, each aiming at its own number of rows.
fn sampled(sql: &str, at: &[(usize, f64)], cat: &Catalog, us: &[Use]) -> String {
    let mut out = sql.to_string();
    let mut ends: Vec<(usize, f64)> = at.to_vec();
    ends.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    for (end, aim) in ends {
        let u = us.iter().find(|u| u.end == end).unwrap();
        out.insert_str(end, &format!(" TABLESAMPLE SYSTEM ({}) REPEATABLE ({SEED})", pct(share(cat.rows(&u.table), aim))));
    }
    out
}

pub fn plan(sql: &str, cat: &Catalog) -> Plan {
    let us = uses(sql);
    let tables: Vec<String> = us.iter().map(|u| u.table.clone()).collect::<HashSet<_>>().into_iter().collect();
    let big: Vec<String> = tables.iter().filter(|t| cat.big(t)).cloned().collect();
    if big.is_empty() {
        return Plan::Exact { sql: sql.to_string() };
    }
    let whole_count = Regex::new(r#"^SELECT COUNT\(\*\) FROM "([^"]+)" b$"#).unwrap();
    if let Some(c) = whole_count.captures(sql) {
        let rows = cat.rows(&c[1]);
        return Plan::Known { rows: vec![vec![Some(format!("{rows:.0}"))]], note: format!("the catalog's estimate of {}", &c[1]) };
    }
    let whole_distinct = Regex::new(r#"^SELECT COUNT\(DISTINCT b\."([^"]+)"\) FROM "([^"]+)" b$"#).unwrap();
    if let Some(c) = whole_distinct.captures(sql) {
        if let Some(n) = cat.tables.get(&c[2]).and_then(|i| i.distinct.get(&c[1])) {
            return Plan::Known { rows: vec![vec![Some(format!("{n:.0}"))]], note: format!("the statistics' distinct values of {}.{}", &c[2], &c[1]) };
        }
    }
    let by_value = Regex::new(r#"^SELECT b\."([^"]+)"(?: AS g)?, COUNT\(\*\)(?: AS v)? FROM "([^"]+)" b GROUP BY (?:1|b\."[^"]+") ORDER BY (?:2|v) DESC LIMIT (\d+)$"#).unwrap();
    let common = by_value.captures(sql).filter(|c| cat.big(&c[2])).map(|c| {
        let lit = |s: &str| format!("'{}'", s.replace('\'', "''"));
        let info = cat.tables.get(&c[2]).cloned().unwrap_or_default();
        Try {
            sql: format!(
                "SELECT s.v, round(s.f * {:.0}) FROM (SELECT unnest(most_common_vals::text::text[]) AS v, unnest(most_common_freqs) AS f FROM pg_stats WHERE schemaname = {} AND tablename = {} AND attname = {}) s ORDER BY 2 DESC LIMIT {}",
                info.rows, lit(&info.schema), lit(&c[2]), lit(&c[1]), &c[3]
            ),
            big: vec![],
            most: None,
            scale: None,
            note: Some(format!("the statistics' most common values of {}.{}", &c[2], &c[1])),
            tables: vec![],
            drop_first: false,
            counted: false,
            fewest: Some(Fewest::Buckets(1)),
        }
    });
    // a count over a count, as "two or more": every row of the inner table
    if sql.starts_with("SELECT COUNT(*) FROM (SELECT") {
        return Plan::Refuse(format!("not run: two or more needs every row of {}", big.join(", ")));
    }
    let base = us.iter().find(|u| !u.joined && u.alias.as_deref() == Some("b"));
    // a join into a big table is a lookup by its key, and needs an index on it
    let on = Regex::new(r#"JOIN "([^"]+)" (\w+) ON (\w+)\."([^"]+)" = (\w+)\."([^"]+)""#).unwrap();
    for c in on.captures_iter(sql) {
        if cat.big(&c[1]) && &c[2] == &c[3] && !cat.indexed(&c[1], &c[4]) {
            return Plan::Refuse(format!("not run: {} is too big to read and has no index on {}", &c[1], &c[4]));
        }
    }
    // a window's end, or a range, read off a big table: through an index, or from its sample
    let anchors: Vec<(usize, f64)> = {
        let mx = Regex::new(r#"(?i)(MAX|MIN)\((?:\w+\.)?"([^"]+)"\)"#).unwrap();
        us.iter().filter(|u| Some(u.end) != base.map(|b| b.end) && !u.joined && cat.big(&u.table)).filter(|u| {
            // the column the subquery or range reads, in the text just before its FROM
            let before = &sql[..u.end];
            let at = before.rfind("SELECT").unwrap_or(0);
            mx.captures_iter(&before[at..]).any(|c| !cat.indexed(&u.table, &c[2]))
        }).map(|u| (u.end, END_ROWS)).collect()
    };
    let Some(base) = base else {
        // a window's end alone, `SELECT MAX("c") FROM "T"`, read once a look: off a log's newest pages, or a sample's
        let end = Regex::new(r#"^SELECT MAX\("([^"]+)"\) FROM "([^"]+)"$"#).unwrap();
        if let Some(c) = end.captures(sql) {
            if cat.big(&c[2]) && cat.indexed(&c[2], &c[1]) {
                // one row off the end of its index; the guard, which sees no Limit above a scan, would call it a whole read
                return Plan::Tries(vec![Try { sql: sql.to_string(), big: vec![], most: None, scale: None, note: None, tables, drop_first: false, counted: false, fewest: None }]);
            }
            if cat.big(&c[2]) && !cat.indexed(&c[2], &c[1]) {
                let info = cat.tables.get(&c[2]).cloned().unwrap_or_default();
                let cor = info.correlation.get(&c[1]).copied().unwrap_or(0.0);
                let (sql, note) = if cat.tid_range && cor.abs() >= 0.95 && info.pages >= 1.0 {
                    let k = (info.pages * END_ROWS / info.rows.max(1.0)).ceil().max(1.0);
                    let range = if cor > 0.0 { format!("ctid >= '({:.0},0)'::tid", (info.pages - k).max(0.0)) } else { format!("ctid < '({k:.0},0)'::tid") };
                    (format!("{sql} WHERE {range}"), format!("the newest pages of {}", &c[2]))
                } else {
                    (sampled(sql, &[(us[0].end, END_ROWS)], cat, &us), format!("a sample of {}", &c[2]))
                };
                return Plan::Tries(vec![Try { sql, big: big.clone(), most: None, scale: None, note: Some(note), tables, drop_first: false, counted: false, fewest: None }]);
            }
        }
        // a range of a column, `SELECT min(c), max(c) FROM "T"`: a sample's when no index has it
        let range = Regex::new(r#"^SELECT min\("([^"]+)"\), max\("[^"]+"\) FROM "([^"]+)"$"#).unwrap();
        if let Some(c) = range.captures(sql) {
            if cat.big(&c[2]) && cat.indexed(&c[2], &c[1]) {
                // a row off each end of its index
                return Plan::Tries(vec![Try { sql: sql.to_string(), big: vec![], most: None, scale: None, note: None, tables, drop_first: false, counted: false, fewest: None }]);
            }
            if cat.big(&c[2]) && !cat.indexed(&c[2], &c[1]) {
                let end = us[0].end;
                return Plan::Tries(vec![Try {
                    sql: sampled(sql, &[(end, SAMPLE_ROWS)], cat, &us),
                    big: big.clone(),
                    most: None,
                    scale: None,
                    note: Some(format!("the range of a {}% sample of {}", pct(share(cat.rows(&c[2]), SAMPLE_ROWS)), &c[2])),
                    tables,
                    drop_first: false,
                    counted: false,
                    fewest: None,
                }]);
            }
        }
        return Plan::Tries(vec![Try { sql: sampled(sql, &anchors, cat, &us), big: big.clone(), most: Some(ROWS), scale: None, note: None, tables, drop_first: false, counted: false, fewest: None }]);
    };
    let outs = outputs(sql);
    let mut tries: Vec<Try> = common.into_iter().collect();
    let window = Regex::new(r#"\bb\."([^"]+)" >= (NOW\(\)|\(SELECT MAX|')"#).unwrap();
    let anchored = if anchors.is_empty() { sql.to_string() } else { sampled(sql, &anchors, cat, &us) };
    let anchor_note = (!anchors.is_empty()).then(|| "its window ending where a sample ends".to_string());
    if !cat.big(&base.table) {
        // a small table, looking up a big one by its key
        return Plan::Tries(vec![Try { sql: anchored, big, most: Some(ROWS), scale: None, note: anchor_note, tables, drop_first: false, counted: false, fewest: None }]);
    }
    if window.captures(sql).is_some_and(|c| cat.indexed(&base.table, &c[1])) {
        // a window through an index, exact while it holds few rows
        tries.push(Try { sql: anchored.clone(), big: big.clone(), most: Some(ROWS), scale: None, note: anchor_note.clone(), tables: tables.clone(), drop_first: false, counted: false, fewest: None });
    }
    // by day over a table kept in its time's order: its newest rows, exact, off its last pages
    let bucketed = sql.starts_with("SELECT date_trunc(") && sql.trim_end().ends_with("ORDER BY 1");
    if let (true, true, Some(c)) = (bucketed, cat.tid_range, window.captures(sql)) {
        if let Some(t) = tail(sql, base, &us, cat, &c[1], &big, &tables) {
            tries.push(t);
        }
    }
    if outs.contains(&Out::Distinct) {
        if tries.is_empty() {
            return Plan::Refuse(format!("not run: a count of distinct values needs every row of {}", base.table));
        }
        return Plan::Tries(tries);
    }
    // a page of a log is a moment: a sample of its pages says nothing of a day
    let logged = window.captures(sql).and_then(|c| cat.tables.get(&base.table).and_then(|i| i.correlation.get(&c[1]).copied())).is_some_and(|r| r.abs() >= 0.95);
    if bucketed && logged {
        if tries.is_empty() {
            return Plan::Refuse(format!("not run: {} keeps its rows in time order, and a sample of its pages says nothing of a day", base.table));
        }
        return Plan::Tries(tries);
    }
    let key = Regex::new(r#"^SELECT b\."([^"]+)""#).unwrap().captures(sql).map(|c| c[1].to_string());
    let clustered = key.as_ref().and_then(|k| cat.tables.get(&base.table).and_then(|i| i.correlation.get(k).copied())).is_some_and(|r| r.abs() >= 0.5);
    if !bucketed && sql.contains(" GROUP BY ") && clustered && share(cat.rows(&base.table), SAMPLE_ROWS) < 10.0 {
        if tries.is_empty() {
            return Plan::Refuse(format!("not run: {} keeps the rows of one {} together, and a sample of its pages counts the few it finds many times over", base.table, key.unwrap_or_default()));
        }
        return Plan::Tries(tries);
    }
    let joins_big = us.iter().any(|u| u.joined && cat.big(&u.table));
    let aim = if joins_big { JOIN_ROWS } else { SAMPLE_ROWS };
    let p = share(cat.rows(&base.table), aim);
    let mut ends = anchors.clone();
    ends.push((base.end, aim));
    let sampled_sql = sampled(sql, &ends, cat, &us);
    let Some(at) = select_end(&sampled_sql) else { return Plan::Refuse("not run: no select list to count a sample by".into()) };
    let mut counted_sql = sampled_sql.clone();
    counted_sql.insert_str(at, ", COUNT(*)");
    let fewest = if bucketed { Fewest::PerBucket(20.0) } else if sql.contains(" GROUP BY ") { Fewest::PerGroup(5.0) } else { Fewest::Whole(30.0) };
    tries.push(Try {
        sql: counted_sql,
        big,
        most: None,
        scale: Some((outs, 100.0 / p)),
        note: Some(format!("a {}% sample of {}", pct(p), base.table)),
        tables,
        drop_first: false,
        counted: true,
        fewest: Some(fewest),
    });
    Plan::Tries(tries)
}

/// The newest rows of a table whose time follows the order of its pages, as
/// an event log's does: the last pages only, a range of `ctid`, and the
/// window's end off fewer pages still.
fn tail(sql: &str, base: &Use, us: &[Use], cat: &Catalog, col: &str, big: &[String], tables: &[String]) -> Option<Try> {
    let info = cat.tables.get(&base.table)?;
    let cor = *info.correlation.get(col)?;
    if cor.abs() < 0.95 || info.pages < 1.0 || info.rows <= 0.0 {
        return None;
    }
    let pages_for = |aim: f64| (info.pages * aim / info.rows).ceil().max(1.0);
    let range = |aim: f64, who: &str| {
        let k = pages_for(aim);
        if cor > 0.0 { format!("{who}ctid >= '({:.0},0)'::tid", (info.pages - k).max(0.0)) } else { format!("{who}ctid < '({k:.0},0)'::tid") }
    };
    let mut out = sql.to_string();
    // the window's end, where the statement reads it off the same table without an index
    let mut ends: Vec<usize> = us.iter().filter(|u| u.end != base.end && !u.joined && u.table == base.table && !cat.indexed(&u.table, col)).map(|u| u.end).collect();
    ends.sort_unstable_by(|a, b| b.cmp(a));
    for end in ends {
        out.insert_str(end, &format!(" WHERE {}", range(END_ROWS, "")));
    }
    let at = out[base.end..].find(" WHERE ")? + base.end + " WHERE ".len();
    out.insert_str(at, &format!("{} AND ", range(SAMPLE_ROWS, "b.")));
    Some(Try {
        sql: out,
        big: big.to_vec(),
        most: Some(ROWS),
        scale: None,
        note: Some(format!("the newest {:.0} rows of {}", SAMPLE_ROWS, base.table)),
        tables: tables.to_vec(),
        drop_first: true,
        counted: false,
        fewest: Some(Fewest::Buckets(7)),
    })
}

/// A sample's answer as the whole's: its counts and sums scaled.
pub fn scale(rows: Rows, outs: &[Out], k: f64) -> Rows {
    rows.into_iter().map(|r| {
        r.into_iter().enumerate().map(|(i, v)| match (outs.get(i), v) {
            (Some(Out::Scale), Some(s)) => match s.trim().parse::<f64>() {
                Ok(x) => Some(if x.fract() == 0.0 { format!("{:.0}", x * k) } else { format!("{:.2}", x * k) }),
                Err(_) => Some(s),
            },
            (_, v) => v,
        }).collect()
    }).collect()
}

/// The guard run before a try, in the same transaction: the try's plan, read
/// by the database itself, refused if it reads a big table whole, in sequence
/// or along an index with nothing to look up in it, or given way if it would
/// read more of one than the try may.
pub fn guard(t: &Try) -> String {
    let lit = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let names = t.big.iter().map(|b| lit(b)).collect::<Vec<_>>().join(", ");
    let over = match t.most {
        Some(n) => format!(
            "IF jsonb_path_exists(p, '$.** ? (@.\"Relation Name\" == $t && @.\"Plan Rows\" > $n)', jsonb_build_object('t', t, 'n', {n})) THEN RAISE EXCEPTION 'see: over %', t; END IF;"
        ),
        None => String::new(),
    };
    format!(
        "DO $see$ DECLARE j json; p jsonb; t text; BEGIN EXECUTE 'EXPLAIN (FORMAT JSON) ' || $q${}$q$ INTO j; p := j::jsonb; \
FOREACH t IN ARRAY ARRAY[{names}]::text[] LOOP \
IF jsonb_path_exists(p, '$.** ? ((@.\"Node Type\" == \"Seq Scan\" || ((@.\"Node Type\" == \"Index Scan\" || @.\"Node Type\" == \"Index Only Scan\") && !(exists(@.\"Index Cond\")))) && @.\"Relation Name\" == $t)', jsonb_build_object('t', t)) THEN RAISE EXCEPTION 'see: whole %', t; END IF; {over} \
END LOOP; END $see$",
        t.sql
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> Catalog {
        let mut c = Catalog::default();
        c.tables.insert("tickets".into(), Info { rows: 3_000_000.0, indexed: ["ticket_no".to_string(), "book_ref".to_string()].into(), distinct: [("book_ref".to_string(), 1_200_000.0)].into(), ..Default::default() });
        c.tables.insert("bookings".into(), Info { rows: 1_300_000.0, indexed: ["book_ref".to_string()].into(), ..Default::default() });
        c.tables.insert("segments".into(), Info { rows: 3_900_000.0, indexed: ["flight_id".to_string()].into(), ..Default::default() });
        c.tables.insert("flights".into(), Info { rows: 5_000.0, ..Default::default() });
        c
    }

    #[test]
    fn small_tables_run_as_written() {
        assert_eq!(plan(r#"SELECT COUNT(*) FROM "flights" b"#, &cat()), Plan::Exact { sql: r#"SELECT COUNT(*) FROM "flights" b"#.into() });
    }

    #[test]
    fn whole_counts_come_from_the_catalog() {
        assert!(matches!(plan(r#"SELECT COUNT(*) FROM "tickets" b"#, &cat()), Plan::Known { .. }));
        match plan(r#"SELECT COUNT(DISTINCT b."book_ref") FROM "tickets" b"#, &cat()) {
            Plan::Known { rows, .. } => assert_eq!(rows[0][0].as_deref(), Some("1200000")),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn a_group_by_is_sampled_and_scaled() {
        let Plan::Tries(t) = plan(r#"SELECT b."book_ref" AS g, SUM(b."amount") AS v FROM "tickets" b GROUP BY b."book_ref" ORDER BY v DESC LIMIT 12"#, &cat()) else { panic!() };
        assert_eq!(t.len(), 1);
        assert!(t[0].sql.contains(r#"SELECT b."book_ref" AS g, SUM(b."amount") AS v, COUNT(*) FROM "tickets" b TABLESAMPLE SYSTEM (0.233333) REPEATABLE (17) GROUP BY"#), "{}", t[0].sql);
        assert!(t[0].counted);
        assert_eq!(t[0].scale.as_ref().unwrap().0, vec![Out::Keep, Out::Scale]);
    }

    #[test]
    fn a_window_on_an_index_is_tried_exact_first() {
        let sql = r#"SELECT COUNT(*) FROM "tickets" b WHERE b."book_ref" >= NOW() - INTERVAL '7 days'"#;
        let Plan::Tries(t) = plan(sql, &cat()) else { panic!() };
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].sql, sql);
        assert_eq!(t[0].most, Some(ROWS));
        assert!(t[1].sql.contains("TABLESAMPLE"));
    }

    #[test]
    fn a_distinct_count_is_refused_without_a_small_window() {
        assert!(matches!(plan(r#"SELECT COUNT(DISTINCT b."flight_id") FROM "segments" b WHERE b."x" IN ('a')"#, &cat()), Plan::Refuse(_)));
        assert!(matches!(plan(r#"SELECT COUNT(*) FROM (SELECT b."x" FROM "segments" b GROUP BY b."x" HAVING COUNT(*) >= 2) x"#, &cat()), Plan::Refuse(_)));
    }

    #[test]
    fn a_join_needs_an_index_on_the_big_side() {
        let ok = r#"SELECT p1."book_ref" AS g, COUNT(*) AS v FROM "segments" b LEFT JOIN "tickets" p1 ON p1."ticket_no" = b."ticket_no" GROUP BY p1."book_ref" ORDER BY v DESC LIMIT 12"#;
        assert!(matches!(plan(ok, &cat()), Plan::Tries(_)));
        let no = r#"SELECT p1."x" AS g, COUNT(*) AS v FROM "segments" b LEFT JOIN "bookings" p1 ON p1."x" = b."x" GROUP BY p1."x""#;
        assert!(matches!(plan(no, &cat()), Plan::Refuse(_)));
    }

    #[test]
    fn a_window_end_without_an_index_is_a_sample_s() {
        let sql = r#"SELECT date_trunc('day', b."flown")::date AS t, COUNT(*) AS v FROM "segments" b WHERE b."flown" IS NOT NULL AND b."flown" >= (SELECT MAX("flown") FROM "segments") - INTERVAL '28 days' GROUP BY date_trunc('day', b."flown")::date ORDER BY 1"#;
        let Plan::Tries(t) = plan(sql, &cat()) else { panic!() };
        let last = t.last().unwrap();
        assert_eq!(last.sql.matches("TABLESAMPLE").count(), 2, "{}", last.sql);
    }

    #[test]
    fn by_day_over_a_log_reads_its_last_pages() {
        let mut c = cat();
        c.tid_range = true;
        let seg = c.tables.get_mut("segments").unwrap();
        seg.pages = 39_000.0;
        seg.correlation.insert("flown".into(), 0.998);
        let sql = r#"SELECT date_trunc('day', b."flown")::date AS t, COUNT(DISTINCT b."who") AS v FROM "segments" b WHERE b."flown" IS NOT NULL AND b."flown" >= (SELECT MAX("flown") FROM "segments") - INTERVAL '28 days' GROUP BY date_trunc('day', b."flown")::date ORDER BY 1"#;
        let Plan::Tries(t) = plan(sql, &c) else { panic!() };
        assert_eq!(t.len(), 1);
        assert!(t[0].sql.contains(r#"FROM "segments" b WHERE b.ctid >= '(38930,0)'::tid AND b."flown" IS NOT NULL"#), "{}", t[0].sql);
        assert!(t[0].sql.contains(r#"FROM "segments" WHERE ctid >= '(38992,0)'::tid)"#), "{}", t[0].sql);
        assert!(t[0].drop_first);
    }

    #[test]
    fn rows_by_a_value_come_from_the_statistics_first() {
        let Plan::Tries(t) = plan(r#"SELECT b."book_ref" AS g, COUNT(*) AS v FROM "tickets" b GROUP BY b."book_ref" ORDER BY v DESC LIMIT 12"#, &cat()) else { panic!() };
        assert!(t[0].sql.contains("FROM pg_stats WHERE schemaname = '' AND tablename = 'tickets' AND attname = 'book_ref'"), "{}", t[0].sql);
        assert!(t[0].tables.is_empty());
        assert!(t[1].sql.contains("TABLESAMPLE"));
    }

    #[test]
    fn scaling_keeps_keys_and_multiplies_counts() {
        let rows = vec![vec![Some("approved".into()), Some("12".into())]];
        assert_eq!(scale(rows, &[Out::Keep, Out::Scale], 300.0), vec![vec![Some("approved".into()), Some("3600".into())]]);
    }
}
