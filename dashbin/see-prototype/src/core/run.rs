//! Running a statement, as lemmesee's rounds ran them: `psql` in round 1's
//! container on this machine (round 1c's `see.run_sql`), or on a database's
//! own host through ssh in a read-only session with half a second a
//! statement (round 1k's `live.remote`). A row is its cells as text, a cell
//! the database left empty is `None`.
//!
//! Bounded, as `see` runs by default, a statement goes through chapter 14's
//! plan (`bound`): in a read-only transaction, its time cut to what is left
//! of its tables' 2 s, joins kept to indexes, and a guard in the database
//! that refuses it before it reads a big table whole.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use super::bound::{self, BUDGET_MS, Catalog, Fewest, Plan, Try};

pub type Rows = Vec<Vec<Option<String>>>;

const NULL: &str = "<<null>>";

pub enum Conn {
    /// `docker exec <container> psql -U <user> -d <database>` on this machine
    Docker { container: String, user: String, database: String },
    /// `ssh <host> docker exec <container> psql`, read only
    Ssh { host: String, key: Option<String>, container: String, user: String, database: String },
}

pub struct Runner {
    pub conn: Conn,
    bound: Option<Bounded>,
}

struct Bounded {
    cat: Catalog,
    /// server time spent on each table
    spent: Mutex<HashMap<String, f64>>,
    /// what an answer is, when it is not the whole's count
    notes: Mutex<HashMap<String, String>>,
    /// no statement longer than this, whatever is left of a table's time
    cap_ms: f64,
    /// a window's end, read once a look for every statement that ends there: its value and what it is
    ends: Mutex<HashMap<(String, String), Option<(String, Option<String>)>>>,
}

fn rows_of(out: &str) -> Rows {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|v| if v == NULL || v == "NULL" { None } else { Some(v.to_string()) }).collect())
        .collect()
}

/// What psql said went wrong: its last ERROR line, not the CONTEXT after it.
fn last_line(err: &str) -> String {
    let lines: Vec<&str> = err.trim().lines().collect();
    lines.iter().rev().find(|l| l.contains("ERROR:")).or(lines.last()).copied().unwrap_or("error").chars().take(300).collect()
}

/// The rows of a script's output, and the server time its `\timing` wrote.
fn timed(out: &str) -> (Rows, f64) {
    let mut ms = 0.0;
    let mut kept = String::new();
    for l in out.lines() {
        if let Some(t) = l.strip_prefix("Time: ") {
            ms += t.split_whitespace().next().and_then(|n| n.parse::<f64>().ok()).unwrap_or(0.0);
        } else if !l.starts_with("Timing is") {
            kept.push_str(l);
            kept.push('\n');
        }
    }
    (rows_of(&kept), ms)
}

fn trace(line: serde_json::Value) {
    // every statement and what it took, a line of JSON each, when SEE_TRACE names a file
    if let Some(path) = std::env::var_os("SEE_TRACE") {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

enum Failed {
    /// the guard found more rows than the try may read: the next try
    Over,
    /// too little to show, and why: the next try
    Thin(String),
    Error(String),
}

/// An answer held to its floor: a sample's own count of rows taken off its
/// last column, the groups too thin to say left out, or why it says too little.
fn floor(mut rows: Rows, t: &Try) -> Result<(Rows, f64), String> {
    let mut seen = 0.0;
    let mut counts = Vec::new();
    if t.counted {
        for r in rows.iter_mut() {
            let n = r.pop().flatten().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            counts.push(n);
            seen += n;
        }
    }
    let what = t.note.clone().unwrap_or_else(|| t.tables.join(", "));
    match t.fewest {
        Some(Fewest::Buckets(k)) if rows.len() < k => Err(format!("not run: {what} hold fewer than {k} days")),
        Some(Fewest::PerBucket(m)) => {
            let mut c = counts.clone();
            c.sort_by(|a, b| a.total_cmp(b));
            let median = c.get(c.len() / 2).copied().unwrap_or(0.0);
            if median < m { Err(format!("not run: {what} holds about {median:.0} rows a day, too few to say one")) } else { Ok((rows, seen)) }
        }
        Some(Fewest::PerGroup(m)) => {
            let kept: Rows = rows.into_iter().zip(&counts).filter(|(_, n)| **n >= m).map(|(r, _)| r).collect();
            if kept.is_empty() { Err(format!("not run: {what} holds too few rows of any one value")) } else { Ok((kept, seen)) }
        }
        Some(Fewest::Whole(m)) if seen < m => Err(format!("not run: {what} holds {seen:.0} rows of it, too few to say")),
        _ => Ok((rows, seen)),
    }
}

// joins in the order written, each a lookup by key through an index: the
// table named first is read, the others only at the keys it holds
const JOINS_BY_INDEX: [&str; 5] = [
    "SET LOCAL enable_seqscan = off",
    "SET LOCAL enable_hashjoin = off",
    "SET LOCAL enable_mergejoin = off",
    "SET LOCAL enable_material = off",
    "SET LOCAL join_collapse_limit = 1",
];

impl Runner {
    pub fn new(conn: Conn) -> Runner {
        Runner { conn, bound: None }
    }

    /// Every statement held to chapter 14's bound, by the catalog the snapshot read.
    pub fn bounded(mut self, cat: Catalog) -> Runner {
        let cap_ms = match self.conn {
            Conn::Docker { .. } => BUDGET_MS,
            Conn::Ssh { .. } => 500.0,
        };
        self.bound = Some(Bounded { cat, spent: Mutex::new(HashMap::new()), notes: Mutex::new(HashMap::new()), cap_ms, ends: Mutex::new(HashMap::new()) });
        self
    }

    /// What a statement's answer is when it is not the whole's: a sample, the catalog.
    pub fn note(&self, sql: &str) -> Option<String> {
        self.bound.as_ref().and_then(|b| b.notes.lock().unwrap().get(sql).cloned())
    }

    pub fn run(&self, sql: &str) -> (Rows, Option<String>) {
        let started = std::time::Instant::now();
        let Some(b) = &self.bound else {
            let (rows, error) = match self.psql(sql, "statement_timeout=5000") {
                Ok(out) => (rows_of(&out), None),
                Err(e) => (vec![], Some(e)),
            };
            trace(serde_json::json!({"sql": sql, "ms": started.elapsed().as_secs_f64() * 1000.0, "rows": rows.len(), "error": error}));
            return (rows, error);
        };
        if let Some(rows) = self.empty_window(b, sql) {
            trace(serde_json::json!({"sql": sql, "known": "its window ends after its table's last row", "ms": 0.0, "rows": rows.len(), "error": null}));
            return (rows, None);
        }
        let (planned, end_note) = self.with_ends(b, sql);
        let tries = match bound::plan(&planned, &b.cat) {
            Plan::Known { rows, note } => {
                trace(serde_json::json!({"sql": sql, "known": note, "ms": 0.0, "rows": rows.len(), "error": null}));
                b.notes.lock().unwrap().insert(sql.to_string(), note);
                return (rows, None);
            }
            Plan::Refuse(why) => {
                trace(serde_json::json!({"sql": sql, "ms": 0.0, "rows": 0, "error": why}));
                return (vec![], Some(why));
            }
            Plan::Exact { sql: s } => {
                let tables = bound::uses(&s).into_iter().map(|u| u.table).collect();
                vec![Try { sql: s, big: vec![], most: None, scale: None, note: None, tables, drop_first: false, counted: false, fewest: None }]
            }
            Plan::Tries(t) => t,
        };
        let n = tries.len();
        for (i, t) in tries.iter().enumerate() {
            match self.attempt(b, t, started) {
                Ok((rows, seen)) => {
                    let note = t.note.as_ref().map(|n| if t.counted { format!("{n}, {seen:.0} of its rows") } else { n.clone() });
                    let ends = end_note.as_ref().map(|e| format!("its window ending where {e} ends"));
                    let note = match (note, ends) {
                        (Some(n), Some(e)) => Some(format!("{n}; {e}")),
                        (n, e) => n.or(e),
                    };
                    if let Some(note) = note {
                        b.notes.lock().unwrap().insert(sql.to_string(), note);
                    }
                    return (rows, None);
                }
                Err(Failed::Over) | Err(Failed::Thin(_)) if i + 1 < n => continue,
                Err(Failed::Thin(why)) => return (vec![], Some(why)),
                Err(Failed::Over) => return (vec![], Some(format!("not run: its window holds more than {} rows of {}", bound::ROWS, t.big.join(", ")))),
                Err(Failed::Error(e)) => return (vec![], Some(e)),
            }
        }
        (vec![], Some("not run".into()))
    }

    /// A statement with each window's end, `(SELECT MAX("c") FROM "T")`, as the
    /// literal the look read once for it: a small table read once a statement,
    /// a big one's end through its index, its newest pages or a sample, and the
    /// same end for every panel of the look. And what the end is, when not exact.
    fn with_ends(&self, b: &Bounded, sql: &str) -> (String, Option<String>) {
        let re = regex::Regex::new(r#"\(SELECT MAX\((?:m\.)?"([^"]+)"\) FROM "([^"]+)"(?: m)?\)"#).unwrap();
        let mut out = sql.to_string();
        let mut note = None;
        for c in re.captures_iter(sql) {
            let (whole, col, table) = (c[0].to_string(), c[1].to_string(), c[2].to_string());
            let Some(ty) = b.cat.time_type(&table, &col) else { continue };
            if let Some((v, n)) = self.end_of(b, &table, &col) {
                out = out.replace(&whole, &format!("'{}'::{ty}", v.replace('\'', "''")));
                note = note.or(n);
            }
        }
        (out, note)
    }

    /// A table's last time, read once a look: through its index, off its newest
    /// pages, from a sample, or exactly for a small table; and what it is when
    /// not exact.
    fn end_of(&self, b: &Bounded, table: &str, col: &str) -> Option<(String, Option<String>)> {
        let key = (table.to_string(), col.to_string());
        if let Some(e) = b.ends.lock().unwrap().get(&key).cloned() {
            return e;
        }
        let q = format!("SELECT MAX(\"{}\") FROM \"{}\"", col.replace('"', "\"\""), table.replace('"', "\"\""));
        let (rows, err) = self.run(&q);
        let e = if err.is_none() { rows.first().and_then(|r| r.first()).cloned().flatten().filter(|v| !v.is_empty()).map(|v| (v, self.note(&q))) } else { None };
        b.ends.lock().unwrap().insert(key, e.clone());
        e
    }

    /// A number over the last days of a big table whose last row is older than
    /// them: no row, so its count and its sum are nothing, said without a
    /// sample, which cannot tell none from a few. Only when that last row was
    /// read through an index, off the newest pages, or exactly.
    fn empty_window(&self, b: &Bounded, sql: &str) -> Option<Rows> {
        let w = regex::Regex::new(r#"\bb\."([^"]+)" >= NOW\(\) - INTERVAL '(\d+) days'"#).unwrap().captures(sql)?;
        if sql.contains(" GROUP BY ") {
            return None;
        }
        let base = bound::uses(sql).into_iter().find(|u| !u.joined && u.alias.as_deref() == Some("b"))?;
        if !b.cat.big(&base.table) {
            return None;
        }
        b.cat.time_type(&base.table, &w[1])?;
        let (last, note) = self.end_of(b, &base.table, &w[1])?;
        if note.as_deref().is_some_and(|n| !n.starts_with("the newest pages")) {
            return None;
        }
        let days: i64 = w[2].parse().ok()?;
        let last_day = civil_days(last.get(..10)?)?;
        let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64 / 86_400;
        if last_day >= today - days {
            return None;
        }
        Some(vec![bound::outputs(sql).into_iter().map(|o| if o == bound::Out::Keep { None } else { Some("0".to_string()) }).collect()])
    }

    fn attempt(&self, b: &Bounded, t: &Try, started: std::time::Instant) -> Result<(Rows, f64), Failed> {
        let left = {
            let spent = b.spent.lock().unwrap();
            t.tables.iter().map(|x| BUDGET_MS - spent.get(x).copied().unwrap_or(0.0)).fold(BUDGET_MS, f64::min)
        };
        if left < 50.0 {
            let why = format!("not run: its {:.0} s on {} are spent", BUDGET_MS / 1000.0, t.tables.join(", "));
            trace(serde_json::json!({"sql": t.sql, "ms": 0.0, "rows": 0, "error": why}));
            return Err(Failed::Error(why));
        }
        let sets: Vec<&str> = if t.big.is_empty() { vec![] } else { JOINS_BY_INDEX.to_vec() };
        let mut script = format!("\\timing on\nBEGIN READ ONLY;\nSET LOCAL statement_timeout = {:.0};\n", left.min(b.cap_ms));
        for s in &sets {
            script += &format!("{s};\n");
        }
        if !t.big.is_empty() {
            script += &format!("{};\n", bound::guard(t));
        }
        script += &format!("{};\nCOMMIT;\n", t.sql);
        let out = self.psql(&script, "");
        let (rows, ms, error) = match &out {
            Ok(o) => {
                let (rows, ms) = timed(o);
                (rows, ms, None)
            }
            Err(e) => (vec![], 0.0, Some(e.clone())),
        };
        {
            let mut spent = b.spent.lock().unwrap();
            for x in &t.tables {
                *spent.entry(x.clone()).or_default() += ms;
            }
        }
        let gave_way = error.as_deref().is_some_and(|e| e.contains("see: over"));
        trace(serde_json::json!({"sql": t.sql, "sets": sets, "ms": started.elapsed().as_secs_f64() * 1000.0, "server_ms": ms, "rows": rows.len(),
            "note": t.note, "gave_way": gave_way, "error": if gave_way { None } else { error.clone() }}));
        match error {
            None => {
                let mut rows = rows;
                if t.drop_first && rows.len() > 1 {
                    rows.remove(0);
                }
                let (rows, seen) = floor(rows, t).map_err(Failed::Thin)?;
                Ok((match &t.scale {
                    Some((outs, k)) => bound::scale(rows, outs, *k),
                    None => rows,
                }, seen))
            }
            Some(_) if gave_way => Err(Failed::Over),
            Some(e) if e.contains("see: whole") => Err(Failed::Error(format!("not run: it would read all of {}", e.rsplit("see: whole ").next().unwrap_or("a big table").trim()))),
            Some(e) if e.contains("statement timeout") => Err(Failed::Error(format!("not run: over the time left on {}", t.tables.join(", ")))),
            Some(e) => Err(Failed::Error(e)),
        }
    }

    /// A script through `psql -f -`, its output, or the last line it wrote of an error.
    fn psql(&self, script: &str, options: &str) -> Result<String, String> {
        let quiet = if options.is_empty() { "-q" } else { "-A" };
        let out = match &self.conn {
            Conn::Docker { container, user, database } => {
                let mut cmd = Command::new("docker");
                cmd.args(["exec", "-i"]);
                if !options.is_empty() {
                    cmd.args(["-e", &format!("PGOPTIONS=-c {options}")]);
                }
                cmd.args([container, "psql", "-U", user, "-d", database, "-At", quiet, "-F", "\t", "-P", &format!("null={NULL}"), "-v", "ON_ERROR_STOP=1", "-f", "-"]);
                feed(cmd, script)
            }
            Conn::Ssh { host, key, container, user, database } => {
                let opts = "-c default_transaction_read_only=on -c statement_timeout=500 -c lock_timeout=1000";
                // every word quoted for the remote shell: an unquoted <<null>> is a here-document there
                let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
                let line = format!(
                    "docker exec -i -e {} {} psql -U {} -d {} -At {quiet} -F $'\\t' -P {} -v ON_ERROR_STOP=1 -f -",
                    q(&format!("PGOPTIONS={opts}")), q(container), q(user), q(database), q(&format!("null={NULL}"))
                );
                let mut cmd = Command::new(if cfg!(windows) { "ssh.exe" } else { "ssh" });
                if let Some(k) = key {
                    cmd.args(["-i", k]);
                }
                cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", host, &line]);
                feed(cmd, script)
            }
        };
        match out {
            Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
            Ok(o) => Err(last_line(&String::from_utf8_lossy(&o.stderr))),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// A day, `YYYY-MM-DD`, as days since 1970, after Howard Hinnant.
fn civil_days(s: &str) -> Option<i64> {
    let (y, m, d): (i64, i64, i64) = (s.get(..4)?.parse().ok()?, s.get(5..7)?.parse().ok()?, s.get(8..10)?.parse().ok()?);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn feed(mut cmd: Command, script: &str) -> std::io::Result<std::process::Output> {
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(script.as_bytes());
    }
    child.wait_with_output()
}
