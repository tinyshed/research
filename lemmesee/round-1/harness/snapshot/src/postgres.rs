//! PostgreSQL through its catalogs: tiers S, N and Q of contract.md.

use model::*;
use crate::record;
use postgres::{Client, NoTls, SimpleQueryMessage};
use std::collections::BTreeMap;

const SETTINGS: &str = include_str!("../queries/postgres/settings.sql");
const S_TABLES: &str = include_str!("../queries/postgres/s_tables.sql");
const S_COLUMNS: &str = include_str!("../queries/postgres/s_columns.sql");
const S_CONSTRAINTS: &str = include_str!("../queries/postgres/s_constraints.sql");
const S_INDEXES: &str = include_str!("../queries/postgres/s_indexes.sql");
const N_TABLES: &str = include_str!("../queries/postgres/n_tables.sql");
const N_COLUMNS: &str = include_str!("../queries/postgres/n_columns.sql");
const Q_WORKLOAD: &str = include_str!("../queries/postgres/q_workload.sql");
const LEDGER: &str = include_str!("../queries/postgres/ledger.sql");

/// The column statistics read from `pg_stats`, by the names they keep.
const COLUMN_STATS: [&str; 8] = [
    "null_frac", "avg_width", "n_distinct", "correlation",
    "mcv_count", "mcv_top_freq", "mcv_freq_sum", "hist_bounds",
];

/// A share of rows changed since the last analysis above which statistics
/// are stale: the round's threshold, not a finding.
const STALE_SHARE: f64 = 0.1;

fn query(client: &mut Client, sql: &str) -> Result<Vec<Row>, String> {
    let messages = client.simple_query(sql).map_err(|e| match e.as_db_error() {
        Some(db) => format!("{}: {}", db.code().code(), db.message()),
        None => e.to_string(),
    })?;
    Ok(messages
        .into_iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(row) => Some(
                row.columns()
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.name().to_string(), row.get(i).map(str::to_string)))
                    .collect(),
            ),
            _ => None,
        })
        .collect())
}

/// The fixture's owner, which resets and reads `pg_stat_statements` around
/// discovery; never the role that discovers.
pub struct Admin(Client);

impl Admin {
    pub fn connect(url: &str) -> Result<Self, String> {
        Client::connect(url, NoTls).map(Admin).map_err(|e| e.to_string())
    }

    pub fn reset(&mut self) -> Result<(), String> {
        query(&mut self.0, "SELECT pg_stat_statements_reset()").map(drop)
    }

    pub fn ledger(&mut self) -> Result<Vec<ServerRecord>, String> {
        Ok(query(&mut self.0, LEDGER)?
            .into_iter()
            .map(|mut row| {
                let id = row.remove("id").flatten().unwrap_or_default();
                let counters = row.iter().map(|(k, v)| (k.clone(), value(v.as_deref()))).collect();
                ServerRecord { id, counters }
            })
            .collect())
    }
}

pub fn snapshot(url: &str, admin: Option<&str>) -> Result<Snapshot, String> {
    let mut admin = admin.map(Admin::connect).transpose()?;
    if let Some(a) = admin.as_mut() {
        a.reset()?;
    }

    let mut client = Client::connect(url, NoTls).map_err(|e| e.to_string())?;
    // Session settings only: every transaction read-only, nothing waits long.
    client
        .batch_execute(
            "SET default_transaction_read_only = on; \
             SET statement_timeout = '5s'; SET lock_timeout = '1s'",
        )
        .map_err(|e| e.to_string())?;
    let mut log = Vec::new();
    let mut run = |sql: &str| record(&mut log, sql, |s| query(&mut client, s));

    let settings = run(SETTINGS)?.pop().unwrap_or_default();
    let table_rows = run(S_TABLES)?;
    let column_rows = run(S_COLUMNS)?;
    let constraint_rows = run(S_CONSTRAINTS)?;
    let index_rows = run(S_INDEXES)?;
    let n_table_rows = run(N_TABLES)?;
    let n_column_rows = run(N_COLUMNS)?;
    let workload_row = run(Q_WORKLOAD)?.pop().unwrap_or_default();

    let mut tables = Vec::new();
    let mut by_oid = BTreeMap::new();
    for r in &table_rows {
        let kind = match get(r, "kind") {
            Some("r") => "table",
            Some("p") => "partitioned",
            Some("v") => "view",
            Some("m") => "materialized",
            Some("f") => "foreign",
            _ => "other",
        };
        by_oid.insert(get(r, "oid").unwrap_or_default().to_string(), tables.len());
        tables.push(Table {
            schema: get(r, "schema").unwrap_or_default().into(),
            name: get(r, "name").unwrap_or_default().into(),
            kind: kind.into(),
            partition_of: get(r, "partition_of").map(Into::into),
            comment: get(r, "comment").map(Into::into),
            can_select: flag(r, "can_select"),
            row_security_active: Some(flag(r, "row_security_active")),
            ..Default::default()
        });
    }
    let table_of = |r: &Row| get(r, "table_oid").and_then(|o| by_oid.get(o)).copied();

    for r in &column_rows {
        let Some(t) = table_of(r) else { continue };
        tables[t].columns.push(Column {
            name: get(r, "attname").unwrap_or_default().into(),
            position: get(r, "attnum").and_then(|s| s.parse().ok()).unwrap_or(0),
            type_name: get(r, "type_name").unwrap_or_default().into(),
            base_type: get(r, "base_type").unwrap_or_default().into(),
            nullable: flag(r, "nullable"),
            default: get(r, "default_expr").map(Into::into),
            identity: get(r, "identity").map(|s| if s == "a" { "always" } else { "by default" }.into()),
            generated: get(r, "generated").map(|s| if s == "s" { "stored" } else { "virtual" }.into()),
            on_update: None,
            comment: get(r, "comment").map(Into::into),
            enum_labels: list(r, "enum_labels"),
            can_select: flag(r, "can_select"),
            features: Features::new(),
        });
    }
    for r in &constraint_rows {
        let Some(t) = table_of(r) else { continue };
        tables[t].constraints.push(Constraint {
            name: get(r, "name").unwrap_or_default().into(),
            kind: get(r, "kind").unwrap_or_default().into(),
            columns: list(r, "columns"),
            ref_table: get(r, "ref_table").map(Into::into),
            ref_columns: list(r, "ref_columns"),
            definition: get(r, "definition").map(Into::into),
        });
    }
    for r in &index_rows {
        let Some(t) = table_of(r) else { continue };
        tables[t].indexes.push(Index {
            name: get(r, "name").unwrap_or_default().into(),
            columns: list(r, "columns"),
            unique: flag(r, "is_unique"),
            primary: flag(r, "is_primary"),
            predicate: get(r, "predicate").map(Into::into),
        });
    }

    // Tier N, tables: the estimate, the pages, and what has happened to them.
    let mut stale = vec![false; tables.len()];
    let mut analysed: Vec<Option<String>> = vec![None; tables.len()];
    for r in &n_table_rows {
        let Some(t) = table_of(r) else { continue };
        let src = "pg.n.tables";
        let last = [get(r, "last_analyze"), get(r, "last_autoanalyze")].into_iter().flatten().max();
        analysed[t] = last.map(Into::into);
        let reltuples: f64 = get(r, "reltuples").and_then(|s| s.parse().ok()).unwrap_or(-1.0);
        let changed: f64 = get(r, "n_mod_since_analyze").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        stale[t] = reltuples > 0.0 && changed > STALE_SHARE * reltuples;
        let f = &mut tables[t].features;
        // -1 is never vacuumed nor analysed: unknown, not empty.
        let rows = if reltuples < 0.0 {
            Feature::missing(State::Absent, src)
        } else {
            let f = Feature::present(get(r, "reltuples"), src).as_of(last);
            if stale[t] { f.with_state(State::Stale) } else { f }
        };
        f.insert("rows_estimate".into(), rows);
        f.insert("pages".into(), Feature::present(get(r, "relpages"), src));
        for k in [
            "n_live_tup", "n_dead_tup", "n_tup_ins", "n_tup_upd", "n_tup_hot_upd", "n_tup_del",
            "n_mod_since_analyze", "analyze_count", "autoanalyze_count",
        ] {
            let feature = match get(r, k) {
                Some(_) => Feature::present(get(r, k), src),
                None => Feature::missing(State::Absent, src),
            };
            f.insert(k.into(), feature);
        }
        f.insert(
            "last_analyzed".into(),
            match last {
                Some(_) => Feature::present(last, src),
                None => Feature::missing(State::Absent, src),
            },
        );
    }

    // Tier N, columns: a partitioned table's statistics are its inherited row.
    let mut column_stats: BTreeMap<(&str, &str, &str, bool), &Row> = BTreeMap::new();
    for r in &n_column_rows {
        let key = (
            get(r, "schemaname").unwrap_or_default(),
            get(r, "tablename").unwrap_or_default(),
            get(r, "attname").unwrap_or_default(),
            flag(r, "inherited"),
        );
        column_stats.insert(key, r);
    }
    for (t, table) in tables.iter_mut().enumerate() {
        let has_statistics = matches!(table.kind.as_str(), "table" | "partitioned" | "materialized");
        let inherited = table.kind == "partitioned";
        let rls = table.row_security_active == Some(true);
        for column in &mut table.columns {
            let src = "pg.n.columns";
            let denied = !table.can_select || !column.can_select || rls;
            let row = column_stats
                .get(&(table.schema.as_str(), table.name.as_str(), column.name.as_str(), inherited))
                .copied();
            for name in COLUMN_STATS {
                let feature = if !has_statistics {
                    Feature::missing(State::Unsupported, src)
                } else if denied {
                    Feature::missing(State::Denied, src)
                } else if let Some(r) = row {
                    let text = get(r, name);
                    let f = match (name, text) {
                        // No `<` operator for the type: no order to correlate.
                        ("correlation", None) => Feature::missing(State::Unsupported, src),
                        // No value is commoner than another, or no histogram.
                        ("mcv_count" | "mcv_freq_sum" | "hist_bounds", None) => Feature::present(Some("0"), src),
                        _ => Feature::present(text, src),
                    };
                    let f = f.as_of(analysed[t].as_deref());
                    if stale[t] && f.state == State::Present { f.with_state(State::Stale) } else { f }
                } else {
                    Feature::missing(State::Absent, src)
                };
                column.features.insert(name.into(), feature);
            }
        }
    }

    let mut workload = Features::new();
    let src = "pg.q.workload";
    workload.insert("statements_installed".into(), Feature::present(get(&workload_row, "installed"), src));
    workload.insert(
        "statements_readable".into(),
        if flag(&workload_row, "installed") && flag(&workload_row, "reads_all") {
            Feature::present(Some("true"), src)
        } else if flag(&workload_row, "installed") {
            Feature::missing(State::Denied, src)
        } else {
            Feature::missing(State::Absent, src)
        },
    );

    let ledger = match admin.as_mut() {
        Some(a) => a.ledger()?,
        None => Vec::new(),
    };
    let s = &settings;
    Ok(Snapshot {
        version: 1,
        engine: "postgresql".into(),
        server_version: get(s, "server_version").unwrap_or_default().into(),
        database: get(s, "database").unwrap_or_default().into(),
        role: get(s, "role").unwrap_or_default().into(),
        taken_at: get(s, "taken_at").unwrap_or_default().into(),
        settings: ["default_statistics_target", "autovacuum", "track_counts"]
            .into_iter()
            .map(|k| (k.to_string(), value(get(s, k))))
            .collect(),
        workload,
        tables,
        queries: log,
        ledger,
    })
}
