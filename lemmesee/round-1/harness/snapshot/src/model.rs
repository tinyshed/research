//! The snapshot as contract.md describes it: every table and column with
//! its features, each `{value, state, source, as_of}`, and the ledger of the
//! queries that read them.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// A query's result as text, column by column, as both engines' text
/// protocols return it.
pub type Row = BTreeMap<String, Option<String>>;

pub fn get<'a>(row: &'a Row, column: &str) -> Option<&'a str> {
    row.get(column).and_then(|v| v.as_deref())
}

/// PostgreSQL's `t` and `f`, MySQL's `1` and `0`.
pub fn flag(row: &Row, column: &str) -> bool {
    matches!(get(row, column), Some("t" | "1" | "true" | "YES"))
}

/// A list the queries join with a tab, which no column name holds.
pub fn list(row: &Row, column: &str) -> Vec<String> {
    get(row, column)
        .map(|s| s.split('\t').map(str::to_string).collect())
        .unwrap_or_default()
}

/// A number when the text is one, an integer when it is whole, else the text.
pub fn value(text: Option<&str>) -> Value {
    let Some(t) = text else { return Value::Null };
    if let Ok(i) = t.parse::<i64>() {
        return Value::from(i);
    }
    match t.parse::<f64>() {
        Ok(f) if f.is_finite() => Value::from(f),
        _ => Value::String(t.to_string()),
    }
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Present,
    Absent,
    Denied,
    Stale,
    Unsupported,
}

#[derive(Serialize, Clone, Debug)]
pub struct Feature {
    pub value: Value,
    pub state: State,
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
}

impl Feature {
    pub fn present(text: Option<&str>, source: &'static str) -> Self {
        Feature { value: value(text), state: State::Present, source, as_of: None }
    }

    pub fn missing(state: State, source: &'static str) -> Self {
        Feature { value: Value::Null, state, source, as_of: None }
    }

    pub fn with_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    pub fn as_of(mut self, as_of: Option<&str>) -> Self {
        self.as_of = as_of.map(str::to_string);
        self
    }
}

pub type Features = BTreeMap<String, Feature>;

#[derive(Serialize, Debug, Default)]
pub struct Column {
    pub name: String,
    pub position: i64,
    /// The type as declared: `numeric(10,2)`, `enum('a','b')`.
    pub type_name: String,
    /// The type without its modifiers: `numeric`, `enum`.
    pub base_type: String,
    pub nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated: Option<String>,
    /// MySQL's `on update CURRENT_TIMESTAMP`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_update: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enum_labels: Vec<String>,
    pub can_select: bool,
    pub features: Features,
}

#[derive(Serialize, Debug)]
pub struct Constraint {
    pub name: String,
    /// `p`, `u`, `f` or `c`, as PostgreSQL's `contype`.
    pub kind: String,
    pub columns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_table: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ref_columns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
}

#[derive(Serialize, Debug, Default)]
pub struct Table {
    pub schema: String,
    pub name: String,
    /// `table`, `partitioned`, `view`, `materialized`, `foreign`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_of: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub can_select: bool,
    /// Whether row-level security applies to the reader; PostgreSQL only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_security_active: Option<bool>,
    pub features: Features,
    pub columns: Vec<Column>,
    pub constraints: Vec<Constraint>,
    pub indexes: Vec<Index>,
}

/// One query as the client saw it.
#[derive(Serialize, Debug)]
pub struct QueryRecord {
    pub id: String,
    pub rows: usize,
    pub client_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One query as the server counted it, matched by the comment it carries.
#[derive(Serialize, Debug)]
pub struct ServerRecord {
    pub id: String,
    pub counters: BTreeMap<String, Value>,
}

#[derive(Serialize, Debug)]
pub struct Snapshot {
    pub version: u32,
    pub engine: &'static str,
    pub server_version: String,
    pub database: String,
    pub role: String,
    pub taken_at: String,
    pub settings: BTreeMap<String, Value>,
    /// Tier Q: whether the application's own queries could be read, and
    /// nothing of what they say.
    pub workload: Features,
    pub tables: Vec<Table>,
    pub queries: Vec<QueryRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ledger: Vec<ServerRecord>,
}
