//! A candidate on the ledger of execution: its time bucket chosen from its
//! column's range, its plan read before it runs, its rows read under the
//! session's limits, and the server's own count of what it did.

use model::Row;
use mysql::prelude::Queryable;
use postgres::{Client, NoTls, SimpleQueryMessage};

const PG_LEDGER: &str = include_str!("../../snapshot/queries/postgres/ledger.sql");
const MY_LEDGER: &str = include_str!("../../snapshot/queries/mysql/ledger.sql");

pub enum Conn {
    Pg(Client),
    My(mysql::Conn),
}

/// Columns and rows, every value as text.
pub type Table = (Vec<String>, Vec<Vec<Option<String>>>);

impl Conn {
    /// A session that may only read, each statement bounded by `limit_ms`;
    /// `None` is the fixture's owner, which only resets and reads counters.
    pub fn connect(engine: &str, url: &str, limit_ms: Option<u64>) -> Result<Conn, String> {
        if engine == "mysql" {
            let opts = mysql::Opts::from_url(url).map_err(|e| e.to_string())?;
            let mut c = mysql::Conn::new(opts).map_err(|e| e.to_string())?;
            if let Some(ms) = limit_ms {
                c.query_drop("SET SESSION TRANSACTION READ ONLY").map_err(|e| e.to_string())?;
                c.query_drop(format!("SET SESSION max_execution_time = {ms}")).map_err(|e| e.to_string())?;
            }
            Ok(Conn::My(c))
        } else {
            let mut c = Client::connect(url, NoTls).map_err(|e| e.to_string())?;
            if let Some(ms) = limit_ms {
                c.batch_execute(&format!(
                    "SET default_transaction_read_only = on; SET statement_timeout = '{ms}ms'; SET lock_timeout = '1s'"
                ))
                .map_err(|e| e.to_string())?;
            }
            Ok(Conn::Pg(c))
        }
    }

    pub fn is_mysql(&self) -> bool {
        matches!(self, Conn::My(_))
    }

    pub fn query(&mut self, sql: &str, cap: usize) -> Result<Table, String> {
        match self {
            Conn::Pg(c) => {
                let messages = c.simple_query(sql).map_err(|e| match e.as_db_error() {
                    Some(db) => format!("{}: {}", db.code().code(), db.message()),
                    None => e.to_string(),
                })?;
                let (mut columns, mut rows) = (Vec::new(), Vec::new());
                for m in messages {
                    match m {
                        SimpleQueryMessage::RowDescription(cols) => {
                            columns = cols.iter().map(|c| c.name().to_string()).collect();
                        }
                        SimpleQueryMessage::Row(row) if rows.len() < cap => {
                            if columns.is_empty() {
                                columns = row.columns().iter().map(|c| c.name().to_string()).collect();
                            }
                            rows.push((0..row.len()).map(|i| row.get(i).map(str::to_string)).collect());
                        }
                        _ => {}
                    }
                }
                Ok((columns, rows))
            }
            Conn::My(c) => {
                let fetch = |c: &mut mysql::Conn| -> Result<Table, mysql::Error> {
                    let (mut columns, mut rows) = (Vec::new(), Vec::new());
                    let mut result = c.query_iter(sql)?;
                    while let Some(set) = result.iter() {
                        for row in set {
                            let row = row?;
                            if columns.is_empty() {
                                columns = row.columns_ref().iter().map(|c| c.name_str().into_owned()).collect();
                            }
                            if rows.len() < cap {
                                rows.push(
                                    (0..row.len())
                                        .map(|i| match row.as_ref(i) {
                                            None | Some(mysql::Value::NULL) => None,
                                            Some(mysql::Value::Bytes(b)) => Some(String::from_utf8_lossy(b).into_owned()),
                                            Some(v) => Some(v.as_sql(true)),
                                        })
                                        .collect(),
                                );
                            }
                        }
                    }
                    Ok((columns, rows))
                };
                fetch(c).map_err(|e| e.to_string())
            }
        }
    }

    /// Whether the plan reads a whole table, and its estimated cost, before
    /// the statement runs.
    pub fn plan(&mut self, sql: &str) -> Result<(bool, Option<f64>), String> {
        let explain = if self.is_mysql() { format!("EXPLAIN FORMAT=JSON {sql}") } else { format!("EXPLAIN (FORMAT JSON) {sql}") };
        let (_, rows) = self.query(&explain, 10)?;
        let text = rows.first().and_then(|r| r.first().cloned().flatten()).unwrap_or_default();
        let full = text.contains("\"Seq Scan\"") || text.contains("\"access_type\": \"ALL\"");
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let cost = json[0]["Plan"]["Total Cost"]
            .as_f64()
            .or_else(|| json["query_block"]["cost_info"]["query_cost"].as_str().and_then(|s| s.parse().ok()));
        Ok((full, cost))
    }

    pub fn reset_counters(&mut self) -> Result<(), String> {
        let sql = if self.is_mysql() {
            "TRUNCATE TABLE performance_schema.events_statements_history_long"
        } else {
            "SELECT pg_stat_statements_reset()"
        };
        self.query(sql, 1).map(drop)
    }

    /// The server's counters for every marked statement since the reset.
    pub fn counters(&mut self) -> Result<Vec<Row>, String> {
        let sql = if self.is_mysql() { MY_LEDGER } else { PG_LEDGER };
        let (columns, rows) = self.query(sql, usize::MAX)?;
        Ok(rows.into_iter().map(|r| columns.iter().cloned().zip(r).collect()).collect())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Bucket {
    Day,
    Week,
    Month,
}

impl Bucket {
    pub fn name(self) -> &'static str {
        match self {
            Bucket::Day => "day",
            Bucket::Week => "week",
            Bucket::Month => "month",
        }
    }

    /// About 60 buckets or fewer over the range a time column spans.
    pub fn for_range(min: &str, max: &str) -> Bucket {
        let span = match (days(min), days(max)) {
            (Some(a), Some(b)) => b - a,
            _ => return Bucket::Month,
        };
        if span <= 90 {
            Bucket::Day
        } else if span <= 420 {
            Bucket::Week
        } else {
            Bucket::Month
        }
    }

    pub fn expr(self, mysql: bool, inner: &str) -> String {
        if !mysql {
            return format!("date_trunc('{}', {inner})", self.name());
        }
        match self {
            Bucket::Day => format!("DATE({inner})"),
            Bucket::Week => format!("DATE({inner} - INTERVAL WEEKDAY({inner}) DAY)"),
            Bucket::Month => format!("DATE_FORMAT({inner}, '%Y-%m-01')"),
        }
    }
}

/// Days since 1970 of a text that begins `YYYY-MM-DD`.
fn days(text: &str) -> Option<i64> {
    let y: i64 = text.get(0..4)?.parse().ok()?;
    let m: i64 = text.get(5..7)?.parse().ok()?;
    let d: i64 = text.get(8..10)?.parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Every `{bucket(<expression>)}` of a candidate's SQL replaced by `f` of
/// its expression.
pub fn substitute(sql: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut rest = sql;
    while let Some(start) = rest.find("{bucket(") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "{bucket(".len()..];
        let Some(end) = after.find(")}") else { break };
        out.push_str(&f(&after[..end]));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_follow_the_range() {
        assert_eq!(days("1970-01-01"), Some(0));
        assert_eq!(days("2026-10-28 01:02:03"), Some(20_754));
        assert!(matches!(Bucket::for_range("2021-01-01", "2021-02-15"), Bucket::Day));
        assert!(matches!(Bucket::for_range("2021-01-01", "2021-12-31"), Bucket::Week));
        assert!(matches!(Bucket::for_range("2021-01-01 00:00:00", "2025-12-22 00:00:00"), Bucket::Month));
        let sql = "SELECT {bucket(t.\"at\")} AS bucket FROM x GROUP BY 1";
        assert_eq!(
            substitute(sql, |e| Bucket::Week.expr(false, e)),
            "SELECT date_trunc('week', t.\"at\") AS bucket FROM x GROUP BY 1"
        );
    }
}
