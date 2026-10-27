//! Reads a database's schema and statistics into a snapshot, as round 1's
//! contract.md says, and prints what each query cost.
//!
//!   snapshot postgres|mysql --url <url> [--admin <url>] --out <file>
//!
//! `--admin` is the fixture's owner, used only to reset and read the
//! server's own statement counters around discovery.

mod mysql;
mod postgres;

use model::{QueryRecord, Row, Snapshot, State};
use std::collections::BTreeMap;
use std::time::Instant;

/// The id a query carries in its first comment, `lemmesee:<step>:<id>`.
fn marker(sql: &str) -> String {
    sql.split("lemmesee:")
        .nth(1)
        .and_then(|rest| rest.split(" */").next())
        .and_then(|step_id| step_id.split(':').nth(1))
        .unwrap_or("unmarked")
        .to_string()
}

/// Runs one query and keeps what the client saw of it.
pub fn record(
    log: &mut Vec<QueryRecord>,
    sql: &str,
    f: impl FnOnce(&str) -> Result<Vec<Row>, String>,
) -> Result<Vec<Row>, String> {
    let start = Instant::now();
    let result = f(sql);
    let client_ms = start.elapsed().as_secs_f64() * 1000.0;
    log.push(QueryRecord {
        id: marker(sql),
        rows: result.as_ref().map_or(0, Vec::len),
        client_ms,
        error: result.as_ref().err().cloned(),
    });
    result
}

fn summary(s: &Snapshot) {
    println!("{} {} {} as {}", s.engine, s.server_version, s.database, s.role);
    println!("{:<22} {:>6} {:>10}  error", "query", "rows", "client ms");
    for q in &s.queries {
        println!("{:<22} {:>6} {:>10.2}  {}", q.id, q.rows, q.client_ms, q.error.as_deref().unwrap_or(""));
    }
    if !s.ledger.is_empty() {
        println!("server ledger:");
        for r in &s.ledger {
            let counters: Vec<String> = r.counters.iter().map(|(k, v)| format!("{k}={v}")).collect();
            println!("  {:<20} {}", r.id, counters.join(" "));
        }
    }
    // How many of each feature were in each state: missing is not zero.
    let mut by_feature: BTreeMap<(&str, &str), BTreeMap<State, usize>> = BTreeMap::new();
    let mut add = |level: &'static str, features: &'_ model::Features| {
        for (name, feature) in features {
            *by_feature
                .entry((level, leak(name)))
                .or_default()
                .entry(feature.state)
                .or_default() += 1;
        }
    };
    for t in &s.tables {
        add("table", &t.features);
        for c in &t.columns {
            add("column", &c.features);
        }
    }
    add("workload", &s.workload);
    println!("feature states:");
    for ((level, name), states) in by_feature {
        let s: Vec<String> = states.iter().map(|(k, v)| format!("{k:?} {v}").to_lowercase()).collect();
        println!("  {level:<8} {name:<22} {}", s.join(", "));
    }
}

/// Feature names are a few fixed words; keeping them for the summary's
/// keys is simpler than borrowing them across the closure.
fn leak(name: &str) -> &'static str {
    Box::leak(name.to_string().into_boxed_str())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (Some(engine), Some(url), Some(out)) = (args.get(1), flag("--url"), flag("--out")) else {
        eprintln!("usage: snapshot postgres|mysql --url <url> [--admin <url>] --out <file>");
        std::process::exit(2);
    };
    let admin = flag("--admin");
    let result = match engine.as_str() {
        "postgres" => postgres::snapshot(&url, admin.as_deref()),
        "mysql" => mysql::snapshot(&url, admin.as_deref()),
        other => Err(format!("unknown engine {other}")),
    };
    match result {
        Ok(snapshot) => {
            let json = serde_json::to_string_pretty(&snapshot).expect("a snapshot serialises");
            if let Err(e) = std::fs::write(&out, json + "\n") {
                eprintln!("writing {out}: {e}");
                std::process::exit(1);
            }
            summary(&snapshot);
        }
        Err(e) => {
            eprintln!("snapshot failed: {e}");
            std::process::exit(1);
        }
    }
}
