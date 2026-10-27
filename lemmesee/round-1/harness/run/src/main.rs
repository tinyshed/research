//! Round 1's ledger of execution and its page: the candidates the
//! configurations chose, and three more the pool adds, run on the fixture
//! and drawn for people to compare and to judge.
//!
//!   run --snapshot <snapshot.json> --proposal <directory> --url <url> [--admin <url>]
//!       --fixture <name> [--note <text>] --out <directory>
//!
//! `--proposal` is where `propose` wrote its documents; `--admin` is the
//! fixture's owner, used only to reset and read the server's counters.

mod exec;

use exec::{Bucket, Conn};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

const CONFIGS: [&str; 3] = ["O", "A", "B1"];
/// A panel's statement is stopped after this long, as chapter 14's bound on
/// looking would stop it.
const LIMIT_MS: u64 = 2000;
const ROWS: usize = 500;
const EXTRA: usize = 3;
const PAGE: &str = include_str!("page.html");

#[derive(Serialize, Default)]
struct Panel {
    id: String,
    template: String,
    aggregate: String,
    chart: String,
    title: String,
    sql: String,
    bucket: Option<&'static str>,
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
    error: Option<String>,
    full_scan: Option<bool>,
    cost: Option<f64>,
    client_ms: f64,
    server: BTreeMap<String, Value>,
    reasons: Vec<String>,
    flags: Vec<String>,
    abstained: Option<String>,
    /// Its place in each configuration's five, where it is in them.
    top: BTreeMap<String, usize>,
}

fn read(dir: &Path, name: &str) -> Value {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("reading {}: {e}", path.display());
        std::process::exit(1);
    });
    serde_json::from_str(&text).expect("a JSON document")
}

fn ident(mysql: bool, name: &str) -> String {
    if mysql { format!("`{}`", name.replace('`', "``")) } else { format!("\"{}\"", name.replace('"', "\"\"")) }
}

/// FNV-1a, for a seed that follows the snapshot.
fn seed(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (Some(snapshot), Some(proposal), Some(url), Some(fixture), Some(out)) =
        (flag("--snapshot"), flag("--proposal"), flag("--url"), flag("--fixture"), flag("--out"))
    else {
        eprintln!("usage: run --snapshot <snapshot.json> --proposal <dir> --url <url> [--admin <url>] --fixture <name> [--note <text>] --out <dir>");
        std::process::exit(2);
    };
    let snapshot: model::Snapshot =
        serde_json::from_str(&std::fs::read_to_string(&snapshot).expect("the snapshot")).expect("a snapshot");
    let dir = Path::new(&proposal);
    let candidates = read(dir, "candidates.json");
    let mysql = snapshot.engine == "mysql";

    // Each configuration's five, and its scores for reasons and flags
    let mut top: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut scored: BTreeMap<String, Value> = BTreeMap::new();
    for config in CONFIGS {
        let panels = read(dir, &format!("panels-{config}.json"));
        top.insert(config.into(), panels["panels"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap().to_string()).collect());
        scored.insert(config.into(), read(dir, &format!("scores-{config}.json")));
    }
    let score_of = |config: &str, id: &str| -> Value {
        scored[config]["candidates"].as_array().unwrap().iter().find(|s| s["id"] == id).cloned().unwrap_or_default()
    };

    // The pool: the union of the fives, and three more no hard rule of
    // tier S removes, abstained ones included, drawn from a seed
    let mut pool: Vec<String> = Vec::new();
    for ids in top.values() {
        for id in ids {
            if !pool.contains(id) {
                pool.push(id.clone());
            }
        }
    }
    let rest: Vec<String> = scored["A"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["removed_by"].is_null() && !pool.iter().any(|p| s["id"] == p.as_str()))
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();
    let mut state = seed(&snapshot.database) | 1;
    let mut chosen = BTreeSet::new();
    while chosen.len() < EXTRA.min(rest.len()) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        chosen.insert((state % rest.len() as u64) as usize);
    }
    pool.extend(chosen.into_iter().map(|i| rest[i].clone()));

    let mut conn = Conn::connect(&snapshot.engine, &url, Some(LIMIT_MS)).unwrap_or_else(|e| {
        eprintln!("connecting: {e}");
        std::process::exit(1);
    });
    let mut admin = flag("--admin").map(|a| Conn::connect(&snapshot.engine, &a, None).expect("the admin connection"));
    if let Some(a) = admin.as_mut() {
        a.reset_counters().expect("counters reset");
    }

    let by_id: BTreeMap<&str, &Value> =
        candidates["candidates"].as_array().unwrap().iter().map(|c| (c["id"].as_str().unwrap(), c)).collect();
    let mut panels: Vec<Panel> = Vec::new();
    for id in &pool {
        let c = by_id[id.as_str()];
        let best = ["B1", "A"].iter().map(|k| score_of(k, id)).find(|s| !s.is_null()).unwrap_or_default();
        let strings = |v: &Value| -> Vec<String> {
            v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
        };
        let mut panel = Panel {
            id: id.clone(),
            template: c["template"].as_str().unwrap_or_default().into(),
            aggregate: c["aggregate"].as_str().unwrap_or_default().into(),
            chart: c["chart"].as_str().unwrap_or_default().into(),
            title: c["title"].as_str().unwrap_or_default().into(),
            reasons: strings(&best["reasons"]),
            flags: strings(&best["flags"]),
            abstained: best["abstained"].as_str().map(str::to_string),
            top: top.iter().filter_map(|(k, ids)| ids.iter().position(|x| x == id).map(|i| (k.clone(), i + 1))).collect(),
            ..Default::default()
        };

        if panel.template == "T8" {
            // Read from the snapshot: no statement runs
            let mut sizes: Vec<(String, f64)> = snapshot
                .tables
                .iter()
                .filter_map(|t| t.features.get("rows_estimate").and_then(|f| f.value.as_f64()).map(|n| (t.name.clone(), n)))
                .collect();
            sizes.sort_by(|a, b| b.1.total_cmp(&a.1));
            panel.columns = vec!["table".into(), "estimated rows".into()];
            panel.rows = sizes.into_iter().take(10).map(|(t, n)| vec![Some(t), Some(format!("{n}"))]).collect();
            panels.push(panel);
            continue;
        }

        let mut sql = c["sql"][if mysql { "mysql" } else { "postgres" }].as_str().unwrap_or_default().to_string();
        if sql.contains("{bucket(") {
            let time = &c["time"];
            let table = snapshot.tables.iter().find(|t| t.name == time["table"].as_str().unwrap_or_default());
            let range = table.map(|t| {
                let from = if mysql { ident(true, &t.name) } else { format!("{}.{}", ident(false, &t.schema), ident(false, &t.name)) };
                let col = ident(mysql, time["column"].as_str().unwrap_or_default());
                format!("SELECT /* lemmesee:execution:range-{id} */ min({col}), max({col}) FROM {from}")
            });
            let bucket = match range.map(|r| conn.query(&r, 1)) {
                Some(Ok((_, rows))) => {
                    let row = rows.first().cloned().unwrap_or_default();
                    let get = |i: usize| row.get(i).cloned().flatten().unwrap_or_default();
                    Bucket::for_range(&get(0), &get(1))
                }
                _ => Bucket::Month,
            };
            panel.bucket = Some(bucket.name());
            sql = exec::substitute(&sql, |e| bucket.expr(mysql, e));
        }
        let sql = sql.replacen("SELECT ", &format!("SELECT /* lemmesee:execution:{id} */ "), 1);
        match conn.plan(&sql) {
            Ok((full, cost)) => {
                panel.full_scan = Some(full);
                panel.cost = cost;
            }
            Err(e) => panel.error = Some(format!("plan: {e}")),
        }
        let started = Instant::now();
        match conn.query(&sql, ROWS) {
            Ok((columns, rows)) => {
                panel.columns = columns;
                panel.rows = rows;
            }
            Err(e) => panel.error = Some(e),
        }
        panel.client_ms = started.elapsed().as_secs_f64() * 1000.0;
        panel.sql = sql;
        panels.push(panel);
    }

    if let Some(a) = admin.as_mut() {
        for row in a.counters().expect("counters read") {
            let id = row.get("id").cloned().flatten().unwrap_or_default();
            let Some(id) = id.strip_prefix("execution:") else { continue };
            if let Some(p) = panels.iter_mut().find(|p| p.id == id) {
                p.server = row
                    .iter()
                    .filter(|(k, _)| k.as_str() != "id")
                    .map(|(k, v)| (k.clone(), model::value(v.as_deref())))
                    .collect();
            }
        }
    }

    let note = flag("--note").unwrap_or_default();
    let data = json!({
        "fixture": fixture,
        "note": note,
        "engine": snapshot.engine,
        "server_version": snapshot.server_version,
        "database": snapshot.database,
        "snapshot": candidates["snapshot"],
        "configs": top,
        "pool": pool,
        "panels": panels,
    });
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("the output directory");
    std::fs::write(out.join("runs.json"), serde_json::to_string_pretty(&data).unwrap() + "\n").expect("runs.json");
    let embedded = serde_json::to_string(&data).unwrap().replace("</", "<\\/");
    std::fs::write(out.join("see.html"), PAGE.replace("/*DATA*/null", &embedded)).expect("see.html");

    println!("{} {} — {} panels in the pool", snapshot.engine, snapshot.database, panels.len());
    for p in &panels {
        let tops: Vec<String> = p.top.iter().map(|(k, i)| format!("{k}{i}")).collect();
        println!(
            "  {:<8} {:<52} {:>4} rows {:>7.1} ms {}{}",
            tops.join(" "),
            p.title,
            p.rows.len(),
            p.client_ms,
            if p.full_scan == Some(true) { "full scan " } else { "" },
            p.error.as_deref().unwrap_or("")
        );
    }
    println!("page: {}", out.join("see.html").display());
}
