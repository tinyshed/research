//! MySQL through `information_schema` and the Performance Schema: tiers S, N
//! and Q of contract.md.

use crate::model::*;
use crate::record;
use mysql::prelude::Queryable;
use mysql::{Conn, Opts, Value as Raw};
use std::collections::BTreeMap;

const SETTINGS: &str = include_str!("../queries/mysql/settings.sql");
const S_TABLES: &str = include_str!("../queries/mysql/s_tables.sql");
const S_COLUMNS: &str = include_str!("../queries/mysql/s_columns.sql");
const S_CONSTRAINTS: &str = include_str!("../queries/mysql/s_constraints.sql");
const S_INDEXES: &str = include_str!("../queries/mysql/s_indexes.sql");
const N_TABLES: &str = include_str!("../queries/mysql/n_tables.sql");
const N_INDEXES: &str = include_str!("../queries/mysql/n_indexes.sql");
const N_HISTOGRAMS: &str = include_str!("../queries/mysql/n_histograms.sql");
const N_ACTIVITY: &str = include_str!("../queries/mysql/n_activity.sql");
const Q_WORKLOAD: &str = include_str!("../queries/mysql/q_workload.sql");
const LEDGER: &str = include_str!("../queries/mysql/ledger.sql");

fn query(conn: &mut Conn, sql: &str) -> Result<Vec<Row>, String> {
    let fetch = |conn: &mut Conn| -> Result<Vec<Row>, mysql::Error> {
        let mut out = Vec::new();
        let mut result = conn.query_iter(sql)?;
        while let Some(set) = result.iter() {
            for row in set {
                let row = row?;
                let mut map = Row::new();
                for (i, column) in row.columns_ref().iter().enumerate() {
                    let text = match row.as_ref(i) {
                        None | Some(Raw::NULL) => None,
                        Some(Raw::Bytes(b)) => Some(String::from_utf8_lossy(b).into_owned()),
                        Some(other) => Some(other.as_sql(true)),
                    };
                    map.insert(column.name_str().into_owned(), text);
                }
                out.push(map);
            }
        }
        Ok(out)
    };
    fetch(conn).map_err(|e| e.to_string())
}

fn denied(error: &str) -> bool {
    error.contains("denied") || error.contains("1142") || error.contains("1227")
}

/// The fixture's root, which empties and reads the statement history around
/// discovery; never the role that discovers.
pub struct Admin(Conn);

impl Admin {
    pub fn connect(url: &str) -> Result<Self, String> {
        let opts = Opts::from_url(url).map_err(|e| e.to_string())?;
        Conn::new(opts).map(Admin).map_err(|e| e.to_string())
    }

    pub fn reset(&mut self) -> Result<(), String> {
        query(&mut self.0, "TRUNCATE TABLE performance_schema.events_statements_history_long").map(drop)
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

/// `enum('a','b')` to its labels; a quote inside a label is doubled.
fn enum_labels(column_type: &str) -> Vec<String> {
    let Some(inner) = column_type.strip_prefix("enum(").and_then(|s| s.strip_suffix(')')) else {
        return Vec::new();
    };
    let inner = inner.trim_start_matches('\'').trim_end_matches('\'');
    inner.split("','").map(|s| s.replace("''", "'")).collect()
}

pub fn snapshot(url: &str, admin: Option<&str>) -> Result<Snapshot, String> {
    let mut admin = admin.map(Admin::connect).transpose()?;
    if let Some(a) = admin.as_mut() {
        a.reset()?;
    }

    let opts = Opts::from_url(url).map_err(|e| e.to_string())?;
    let mut conn = Conn::new(opts).map_err(|e| e.to_string())?;
    // Session settings only: read-only, and no SELECT runs past 5 s.
    for set in ["SET SESSION TRANSACTION READ ONLY", "SET SESSION max_execution_time = 5000"] {
        conn.query_drop(set).map_err(|e| e.to_string())?;
    }
    let mut log = Vec::new();
    let mut run = |sql: &str| record(&mut log, sql, |s| query(&mut conn, s));

    let settings = run(SETTINGS)?.pop().unwrap_or_default();
    let table_rows = run(S_TABLES)?;
    let column_rows = run(S_COLUMNS)?;
    let constraint_rows = run(S_CONSTRAINTS)?;
    let index_rows = run(S_INDEXES)?;
    let n_table_rows = run(N_TABLES)?;
    let n_index_rows = run(N_INDEXES)?;
    let n_histogram_rows = run(N_HISTOGRAMS)?;
    // The Performance Schema is refused to a role without SELECT on it.
    let activity = run(N_ACTIVITY);
    let workload = run(Q_WORKLOAD);

    let schema = get(&settings, "database").unwrap_or_default().to_string();
    let mut tables = Vec::new();
    let mut by_name = BTreeMap::new();
    for r in &table_rows {
        let name = get(r, "TABLE_NAME").unwrap_or_default();
        let kind = match (get(r, "TABLE_TYPE"), get(r, "CREATE_OPTIONS")) {
            (Some("VIEW"), _) => "view",
            (_, Some(o)) if o.contains("partitioned") => "partitioned",
            _ => "table",
        };
        by_name.insert(name.to_string(), tables.len());
        tables.push(Table {
            schema: schema.clone(),
            name: name.into(),
            kind: kind.into(),
            comment: get(r, "TABLE_COMMENT").filter(|c| !c.is_empty()).map(Into::into),
            ..Default::default()
        });
    }
    let table_of = |r: &Row| get(r, "TABLE_NAME").and_then(|n| by_name.get(n)).copied();

    for r in &column_rows {
        let Some(t) = table_of(r) else { continue };
        let extra = get(r, "EXTRA").unwrap_or_default();
        let column_type = get(r, "COLUMN_TYPE").unwrap_or_default();
        let can_select = get(r, "PRIVILEGES").is_some_and(|p| p.split(',').any(|p| p == "select"));
        tables[t].can_select |= can_select;
        tables[t].columns.push(Column {
            name: get(r, "COLUMN_NAME").unwrap_or_default().into(),
            position: get(r, "ORDINAL_POSITION").and_then(|s| s.parse().ok()).unwrap_or(0),
            type_name: column_type.into(),
            base_type: get(r, "DATA_TYPE").unwrap_or_default().into(),
            nullable: get(r, "IS_NULLABLE") == Some("YES"),
            default: get(r, "COLUMN_DEFAULT").map(Into::into),
            identity: extra.contains("auto_increment").then(|| "auto_increment".into()),
            generated: get(r, "GENERATION_EXPRESSION").filter(|g| !g.is_empty()).map(Into::into),
            on_update: extra
                .find("on update ")
                .map(|i| extra[i + "on update ".len()..].trim().to_string()),
            comment: get(r, "COLUMN_COMMENT").filter(|c| !c.is_empty()).map(Into::into),
            enum_labels: enum_labels(column_type),
            can_select,
            features: Features::new(),
        });
    }
    for r in &constraint_rows {
        let Some(t) = table_of(r) else { continue };
        let kind = match get(r, "CONSTRAINT_TYPE") {
            Some("PRIMARY KEY") => "p",
            Some("UNIQUE") => "u",
            Some("FOREIGN KEY") => "f",
            Some("CHECK") => "c",
            _ => continue,
        };
        tables[t].constraints.push(Constraint {
            name: get(r, "CONSTRAINT_NAME").unwrap_or_default().into(),
            kind: kind.into(),
            columns: list(r, "cols"),
            ref_table: get(r, "ref_table").map(|t| format!("{schema}.{t}")),
            ref_columns: list(r, "ref_cols"),
            definition: get(r, "check_clause").map(Into::into),
        });
    }
    for r in &index_rows {
        let Some(t) = table_of(r) else { continue };
        let name = get(r, "INDEX_NAME").unwrap_or_default();
        tables[t].indexes.push(Index {
            name: name.into(),
            columns: list(r, "cols"),
            unique: get(r, "NON_UNIQUE") == Some("0"),
            primary: name == "PRIMARY",
            predicate: None,
        });
    }

    // Tier N, tables: InnoDB's estimates, cached by information_schema.
    for r in &n_table_rows {
        let Some(t) = table_of(r) else { continue };
        let src = "my.n.tables";
        let f = &mut tables[t].features;
        let updated = get(r, "UPDATE_TIME");
        f.insert("rows_estimate".into(), Feature::present(get(r, "TABLE_ROWS"), src).as_of(updated));
        for (k, c) in [("avg_row_length", "AVG_ROW_LENGTH"), ("data_length", "DATA_LENGTH"), ("auto_increment", "AUTO_INCREMENT")] {
            let feature = match get(r, c) {
                Some(_) => Feature::present(get(r, c), src),
                None => Feature::missing(State::Absent, src),
            };
            f.insert(k.into(), feature);
        }
    }
    match &activity {
        Ok(rows) => {
            for r in rows {
                let Some(t) = table_of(r) else { continue };
                for (k, c) in [("n_tup_ins", "COUNT_INSERT"), ("n_tup_upd", "COUNT_UPDATE"), ("n_tup_del", "COUNT_DELETE")] {
                    tables[t].features.insert(k.into(), Feature::present(get(r, c), "my.n.activity"));
                }
            }
        }
        Err(e) => {
            let state = if denied(e) { State::Denied } else { State::Absent };
            for table in tables.iter_mut().filter(|t| t.kind != "view") {
                for k in ["n_tup_ins", "n_tup_upd", "n_tup_del"] {
                    table.features.insert(k.into(), Feature::missing(state, "my.n.activity"));
                }
            }
        }
    }

    // Tier N, columns: a histogram where someone made one, and an index's
    // cardinality for the first column of each index only.
    let mut index_distinct: BTreeMap<(&str, &str), &str> = BTreeMap::new();
    for r in &n_index_rows {
        if let (Some(t), Some(c), Some(n)) = (get(r, "TABLE_NAME"), get(r, "COLUMN_NAME"), get(r, "CARDINALITY")) {
            let best = index_distinct.entry((t, c)).or_insert(n);
            if n.parse::<f64>().unwrap_or(0.0) > best.parse::<f64>().unwrap_or(0.0) {
                *best = n;
            }
        }
    }
    let histograms: BTreeMap<(&str, &str), &Row> = n_histogram_rows
        .iter()
        .filter_map(|r| Some(((get(r, "TABLE_NAME")?, get(r, "COLUMN_NAME")?), r)))
        .collect();
    for table in &mut tables {
        let has_statistics = table.kind != "view";
        for column in &mut table.columns {
            let src = "my.n.histograms";
            let key = (table.name.as_str(), column.name.as_str());
            let f = &mut column.features;
            if !has_statistics {
                for k in ["null_frac", "n_distinct", "index_distinct", "correlation", "avg_width"] {
                    f.insert(k.into(), Feature::missing(State::Unsupported, src));
                }
                continue;
            }
            if !column.can_select {
                for k in ["null_frac", "n_distinct", "index_distinct"] {
                    f.insert(k.into(), Feature::missing(State::Denied, src));
                }
            } else {
                match histograms.get(&key) {
                    Some(h) => {
                        let as_of = get(h, "last_updated");
                        let distinct = match get(h, "kind") {
                            Some("singleton") => get(h, "buckets"),
                            _ => get(h, "distinct_sum"),
                        };
                        f.insert("null_frac".into(), Feature::present(get(h, "null_frac"), src).as_of(as_of));
                        f.insert("n_distinct".into(), Feature::present(distinct, src).as_of(as_of));
                        f.insert("sampling_rate".into(), Feature::present(get(h, "sampling_rate"), src).as_of(as_of));
                    }
                    None => {
                        f.insert("null_frac".into(), Feature::missing(State::Absent, src));
                        f.insert("n_distinct".into(), Feature::missing(State::Absent, src));
                    }
                }
                f.insert(
                    "index_distinct".into(),
                    match index_distinct.get(&key) {
                        Some(n) => Feature::present(Some(n), "my.n.indexes"),
                        None => Feature::missing(State::Absent, "my.n.indexes"),
                    },
                );
            }
            // MySQL keeps no order of rows against a column's, and no width.
            f.insert("correlation".into(), Feature::missing(State::Unsupported, src));
            f.insert("avg_width".into(), Feature::missing(State::Unsupported, src));
        }
    }

    let mut workload_features = Features::new();
    workload_features.insert(
        "statements_readable".into(),
        match &workload {
            Ok(_) => Feature::present(Some("true"), "my.q.workload"),
            Err(e) if denied(e) => Feature::missing(State::Denied, "my.q.workload"),
            Err(_) => Feature::missing(State::Absent, "my.q.workload"),
        },
    );

    let ledger = match admin.as_mut() {
        Some(a) => a.ledger()?,
        None => Vec::new(),
    };
    let s = &settings;
    Ok(Snapshot {
        version: 1,
        engine: "mysql",
        server_version: get(s, "server_version").unwrap_or_default().into(),
        database: schema,
        role: get(s, "role").unwrap_or_default().into(),
        taken_at: get(s, "taken_at").unwrap_or_default().into(),
        settings: [
            "information_schema_stats_expiry",
            "innodb_stats_persistent",
            "innodb_stats_auto_recalc",
            "innodb_stats_persistent_sample_pages",
        ]
        .into_iter()
        .map(|k| (k.to_string(), value(get(s, k))))
        .collect(),
        workload: workload_features,
        tables,
        queries: log,
        ledger,
    })
}
