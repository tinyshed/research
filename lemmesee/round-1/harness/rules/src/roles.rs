//! What a column or a table probably is, and the evidence that says so:
//! candidates.md's roles, read from tier S alone or from S and N.

use crate::names::*;
use model::{Column, Snapshot, State, Table};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Created,
    Updated,
    Event,
    Period,
    Calendar,
    Deleted,
    Pk,
    Fk,
    FkGuessed,
    Identifier,
    Category,
    Currency,
    Money,
    Quantity,
    Level,
    Ratio,
    Score,
    Place,
    Code,
    Text,
    Number,
}

impl Role {
    pub fn is_measure(self) -> bool {
        matches!(self, Role::Money | Role::Quantity | Role::Level | Role::Ratio | Role::Score)
    }

    pub fn is_key(self) -> bool {
        matches!(self, Role::Pk | Role::Fk | Role::FkGuessed)
    }
}

/// Strongest first: a declaration, then a statistic, then a name, then a
/// type alone.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Declared,
    Measured,
    Named,
    Typed,
}

#[derive(Serialize, Clone, Debug)]
pub struct Evidence {
    pub kind: Kind,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Held<R> {
    pub role: R,
    pub evidence: Vec<Evidence>,
}

impl<R> Held<R> {
    pub fn strongest(&self) -> Kind {
        self.evidence.iter().map(|e| e.kind).min().unwrap_or(Kind::Typed)
    }
}

fn add<R: PartialEq + Copy>(held: &mut Vec<Held<R>>, role: R, kind: Kind, detail: impl Into<String>) {
    let evidence = Evidence { kind, detail: detail.into() };
    match held.iter_mut().find(|h| h.role == role) {
        Some(h) => h.evidence.push(evidence),
        None => held.push(Held { role, evidence: vec![evidence] }),
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ColumnRoles {
    pub column: String,
    pub held: Vec<Held<Role>>,
}

impl ColumnRoles {
    pub fn get(&self, role: Role) -> Option<&Held<Role>> {
        self.held.iter().find(|h| h.role == role)
    }

    pub fn has(&self, role: Role) -> bool {
        self.get(role).is_some()
    }

    /// The most declared role wins, then the most measured, then the first
    /// in candidates.md's table.
    pub fn primary(&self) -> Option<Role> {
        self.held
            .iter()
            .min_by_key(|h| {
                let declared = h.evidence.iter().any(|e| e.kind == Kind::Declared);
                let measured = h.evidence.iter().any(|e| e.kind == Kind::Measured);
                (!declared, !measured, h.role)
            })
            .map(|h| h.role)
    }

    /// The time role a column holds as an axis, if it may be one.
    pub fn axis(&self) -> Option<Role> {
        [Role::Created, Role::Event].into_iter().find(|r| self.has(*r))
    }
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TableRole {
    Fact,
    Event,
    Entity,
    Snapshot,
    Link,
    Lookup,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct TableRoles {
    pub table: String,
    pub held: Vec<Held<TableRole>>,
    /// A lookup's column to label its rows by.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub columns: Vec<ColumnRoles>,
    #[serde(skip)]
    pub label_index: Option<usize>,
}

impl TableRoles {
    pub fn has(&self, role: TableRole) -> bool {
        self.held.iter().any(|h| h.role == role)
    }
}

/// A foreign key, declared or guessed, from a child's column to a parent's.
#[derive(Serialize, Clone, Debug)]
pub struct Fk {
    pub child: usize,
    pub child_column: usize,
    pub parent: usize,
    pub parent_column: usize,
    pub guessed: bool,
}

/// The snapshot with its roles, for one way of reading it.
pub struct Schema<'a> {
    pub snapshot: &'a Snapshot,
    pub reads_n: bool,
    pub tables: Vec<TableRoles>,
    pub fks: Vec<Fk>,
}

/// A statistic's value when this reading may use it: tier N, present or stale.
pub fn stat(features: &model::Features, name: &str, reads_n: bool) -> Option<f64> {
    if !reads_n {
        return None;
    }
    let f = features.get(name)?;
    if !matches!(f.state, State::Present | State::Stale) {
        return None;
    }
    f.value.as_f64()
}

pub fn rows(table: &Table, reads_n: bool) -> Option<f64> {
    stat(&table.features, "rows_estimate", reads_n)
}

/// A column's distinct values as a count: PostgreSQL's negative `n_distinct`
/// is a share of the rows; MySQL's comes from a histogram or, for the first
/// column of an index, from its cardinality.
pub fn distinct(table: &Table, column: &Column, reads_n: bool) -> Option<f64> {
    let n = stat(&column.features, "n_distinct", reads_n)
        .or_else(|| stat(&column.features, "index_distinct", reads_n))?;
    if n >= 0.0 {
        return Some(n);
    }
    rows(table, reads_n).map(|r| (-n * r).round())
}

pub fn is_base_table(t: &Table) -> bool {
    matches!(t.kind.as_str(), "table" | "partitioned")
}

const CREATED: &[&str] = &["created", "create", "inserted", "registered", "signup", "joined"];
const UPDATED: &[&str] = &["updated", "modified", "changed"];
const EVENT: &[&str] = &["at", "date", "time", "timestamp", "occurred", "ordered", "paid", "shipped", "sent", "started"];
const CALENDAR: &[&str] = &["birth", "dob", "birthday", "year"];
const DELETED: &[&str] = &["deleted", "removed", "archived"];
const IDENTIFIER: &[&str] = &["id", "uuid", "guid", "key", "hash", "token", "code", "sku"];
const CATEGORY: &[&str] = &[
    "status", "state", "type", "kind", "category", "level", "role", "tier", "plan", "country", "source", "channel",
];
const CURRENCY: &[&str] = &["currency", "ccy"];
const MONEY: &[&str] = &[
    "amount", "price", "total", "subtotal", "cost", "fee", "charge", "tax", "discount", "paid", "salary", "wage", "revenue",
];
const CENTS: &[&str] = &["cents", "kopecks"];
const QUANTITY: &[&str] = &["qty", "quantity", "count", "units", "items", "seats"];
const LEVEL: &[&str] = &["balance", "stock", "inventory", "remaining", "level"];
const RATIO: &[&str] = &["percent", "pct", "rate", "ratio", "share"];
const SCORE: &[&str] = &["rating", "score", "stars", "rank", "priority"];
const PLACE: &[&str] = &["lat", "lng", "lon", "latitude", "longitude"];
const CODE: &[&str] = &["zip", "postal", "phone"];
const TEXT: &[&str] = &["name", "title", "description", "email", "url", "comment", "note", "body"];
const EVENT_TABLE: &[&str] = &["log", "event", "events", "history", "audit", "activity"];

/// Pairs that make two times of one table a period.
const PERIOD: &[(&str, &str)] = &[("from", "to"), ("start", "end")];

/// The table a `<table>_id` column is named for, when that table's primary
/// key is one column of the same kind of type.
fn guessed_parent(snapshot: &Snapshot, column: &Column) -> Option<(usize, usize)> {
    let tk = tokens(&column.name);
    if tk.len() < 2 || tk.last().map(String::as_str) != Some("id") {
        return None;
    }
    let stem: String = tk[..tk.len() - 1].concat();
    snapshot.tables.iter().enumerate().find_map(|(p, parent)| {
        let name = norm(&parent.name);
        if name != stem && name != format!("{stem}s") && name != format!("{stem}es") {
            return None;
        }
        let pk = parent.constraints.iter().find(|k| k.kind == "p" && k.columns.len() == 1)?;
        let pc = parent.columns.iter().position(|c| c.name == pk.columns[0])?;
        let same_kind = (is_integer(column) && is_integer(&parent.columns[pc]))
            || (is_uuid(column) && is_uuid(&parent.columns[pc]))
            || (is_text(column) && is_text(&parent.columns[pc]));
        same_kind.then_some((p, pc))
    })
}

fn column_roles(snapshot: &Snapshot, t: usize, c: usize, fks: &[Fk], reads_n: bool) -> ColumnRoles {
    let table = &snapshot.tables[t];
    let column = &table.columns[c];
    let tk = tokens(&column.name);
    let mut held: Vec<Held<Role>> = Vec::new();

    // Keys
    if table.constraints.iter().any(|k| k.kind == "p" && k.columns.contains(&column.name)) {
        add(&mut held, Role::Pk, Kind::Declared, "primary key");
    }
    for fk in fks.iter().filter(|f| f.child == t && f.child_column == c) {
        let parent = &snapshot.tables[fk.parent].name;
        if fk.guessed {
            add(&mut held, Role::FkGuessed, Kind::Named, format!("named for {parent}"));
        } else {
            add(&mut held, Role::Fk, Kind::Declared, format!("foreign key to {parent}"));
        }
    }

    // Times
    if is_time(column) {
        if stamps_now(column) {
            add(&mut held, Role::Created, Kind::Declared, "defaults to the moment of writing");
        }
        if any(&tk, CREATED) {
            add(&mut held, Role::Created, Kind::Named, "named as a creation time");
        }
        if column.on_update.is_some() {
            add(&mut held, Role::Updated, Kind::Declared, "set on every update");
        }
        if any(&tk, UPDATED) {
            add(&mut held, Role::Updated, Kind::Named, "named as an update time");
        }
        if any(&tk, DELETED) {
            add(&mut held, Role::Deleted, Kind::Named, "named as a deletion time");
        }
        if any(&tk, CALENDAR) {
            add(&mut held, Role::Calendar, Kind::Named, "named as a date of a thing");
        }
        let times: Vec<Vec<String>> =
            table.columns.iter().filter(|o| is_time(o)).map(|o| tokens(&o.name)).collect();
        for (a, b) in PERIOD {
            let mine = if any(&tk, &[a]) { Some(b) } else if any(&tk, &[b]) { Some(a) } else { None };
            if let Some(other) = mine {
                if times.iter().any(|o| any(o, &[other])) {
                    add(&mut held, Role::Period, Kind::Named, format!("one end of a {a}–{b} pair"));
                }
            }
        }
        let other_time = [Role::Created, Role::Updated, Role::Deleted, Role::Calendar, Role::Period];
        if !held.iter().any(|h| other_time.contains(&h.role)) && any(&tk, EVENT) {
            add(&mut held, Role::Event, Kind::Named, "named as a time");
        }
    }
    if is_bool(column) {
        if any(&tk, DELETED) {
            add(&mut held, Role::Deleted, Kind::Named, "named as deleted");
        }
        add(&mut held, Role::Category, Kind::Declared, "a boolean");
    }
    if is_integer(column) && any(&tk, &["year"]) {
        add(&mut held, Role::Calendar, Kind::Named, "named as a year");
    }

    // Identifiers
    if tk.last().is_some_and(|l| IDENTIFIER.contains(&l.as_str())) {
        add(&mut held, Role::Identifier, Kind::Named, "named as an identifier");
    }
    if is_uuid(column) {
        add(&mut held, Role::Identifier, Kind::Typed, "a uuid");
    }
    let is_key = held.iter().any(|h| h.role.is_key());
    if !is_time(column) && !is_key && stat(&column.features, "n_distinct", reads_n) == Some(-1.0) {
        add(&mut held, Role::Identifier, Kind::Measured, "every value distinct");
    }

    // Declared categories
    if !column.enum_labels.is_empty() {
        add(&mut held, Role::Category, Kind::Declared, format!("enumerated, {} values", column.enum_labels.len()));
    }
    let checked = table.constraints.iter().any(|k| {
        k.kind == "c"
            && k.columns.iter().any(|n| n == &column.name)
            && k.definition.as_deref().is_some_and(|d| {
                let d = d.to_lowercase();
                d.contains(" in (") || d.contains("= any")
            })
    });
    if checked {
        add(&mut held, Role::Category, Kind::Declared, "checked against a list");
    }

    // Measures, on numbers only
    if is_numeric(column) {
        if (is_decimal(column) && matches!(scale(column), Some(2 | 4))) || column.base_type.eq_ignore_ascii_case("money") {
            add(&mut held, Role::Money, Kind::Typed, format!("typed {}", column.type_name));
        }
        if any(&tk, MONEY) {
            add(&mut held, Role::Money, Kind::Named, "named as money");
        }
        if is_integer(column) && any(&tk, CENTS) {
            add(&mut held, Role::Money, Kind::Named, "named in cents");
        }
        if is_integer(column) && (any(&tk, QUANTITY) || tk.first().map(String::as_str) == Some("num")) {
            add(&mut held, Role::Quantity, Kind::Named, "named as a quantity");
        }
        let on_hand = tk.windows(2).any(|w| w[0] == "on" && w[1] == "hand");
        if any(&tk, LEVEL) || on_hand {
            add(&mut held, Role::Level, Kind::Named, "named as a level");
        }
        if any(&tk, RATIO) {
            add(&mut held, Role::Ratio, Kind::Named, "named as a ratio");
        }
        if any(&tk, SCORE) {
            add(&mut held, Role::Score, Kind::Named, "named as a score");
        }
    }
    if any(&tk, CURRENCY) {
        add(&mut held, Role::Currency, Kind::Named, "named as a currency");
    }
    let beside_money = table.columns.iter().any(|o| is_decimal(o) && matches!(scale(o), Some(2 | 4)));
    if is_text(column) && length(column) == Some(3) && beside_money && !any(&tk, CURRENCY) {
        add(&mut held, Role::Currency, Kind::Typed, "three characters beside money");
    }
    if any(&tk, PLACE) {
        add(&mut held, Role::Place, Kind::Named, "named as a place");
    }
    if any(&tk, CODE) {
        add(&mut held, Role::Code, Kind::Named, "named as a code");
    }

    // Text
    if is_text(column) && column.enum_labels.is_empty() {
        if long_text(column) {
            add(&mut held, Role::Text, Kind::Typed, format!("typed {}", column.type_name));
        }
        if any(&tk, TEXT) {
            add(&mut held, Role::Text, Kind::Named, "named as text");
        }
        if stat(&column.features, "avg_width", reads_n).is_some_and(|w| w > 40.0) {
            add(&mut held, Role::Text, Kind::Measured, "wide values");
        }
        if stat(&column.features, "n_distinct", reads_n).is_some_and(|n| n <= -0.9) {
            add(&mut held, Role::Text, Kind::Measured, "nearly every value distinct");
        }
    }

    // Named categories, on text and integers that are not keys
    let is_key = held.iter().any(|h| h.role.is_key());
    if (is_text(column) || is_integer(column)) && !is_key && any(&tk, CATEGORY) {
        add(&mut held, Role::Category, Kind::Named, "named as a category");
    }

    // Measured categories: never on money, a quantity or a decimal
    let has_measure = held.iter().any(|h| h.role.is_measure());
    let excluded = has_measure || is_key || is_decimal(column) || is_float(column) || is_time(column);
    if reads_n && !excluded && (is_text(column) || is_integer(column)) {
        let n = distinct(table, column, reads_n);
        let mcv_count = stat(&column.features, "mcv_count", reads_n);
        let mcv_sum = stat(&column.features, "mcv_freq_sum", reads_n);
        if let Some(n) = n.filter(|n| *n > 0.0 && *n <= 30.0) {
            add(&mut held, Role::Category, Kind::Measured, format!("{n} distinct values"));
        } else if let (Some(k), Some(s)) = (mcv_count, mcv_sum) {
            if k > 0.0 && k <= 30.0 && s >= 0.95 {
                add(&mut held, Role::Category, Kind::Measured, format!("{k} common values cover {:.0} %", s * 100.0));
            }
        }
    }

    // What nothing named: a number, and a short text or an integer that may
    // be a category, which only a statistic can confirm
    if held.is_empty() {
        if is_numeric(column) {
            add(&mut held, Role::Number, Kind::Typed, format!("typed {}", column.type_name));
        }
        if (is_text(column) && !long_text(column)) || is_integer(column) {
            add(&mut held, Role::Category, Kind::Typed, "could be a category by its type");
        }
    }

    ColumnRoles { column: column.name.clone(), held }
}

fn foreign_keys(snapshot: &Snapshot) -> Vec<Fk> {
    let mut fks = Vec::new();
    for (t, table) in snapshot.tables.iter().enumerate() {
        for k in table.constraints.iter().filter(|k| k.kind == "f" && k.columns.len() == 1) {
            let Some(child_column) = table.columns.iter().position(|c| c.name == k.columns[0]) else { continue };
            let Some(parent) = k.ref_table.as_deref().and_then(|r| {
                snapshot.tables.iter().position(|p| format!("{}.{}", p.schema, p.name) == r)
            }) else {
                continue;
            };
            let Some(parent_column) =
                snapshot.tables[parent].columns.iter().position(|c| Some(&c.name) == k.ref_columns.first())
            else {
                continue;
            };
            fks.push(Fk { child: t, child_column, parent, parent_column, guessed: false });
        }
    }
    for (t, table) in snapshot.tables.iter().enumerate() {
        if !is_base_table(table) {
            continue;
        }
        for (c, column) in table.columns.iter().enumerate() {
            let declared = fks.iter().any(|f| f.child == t && f.child_column == c)
                || table.constraints.iter().any(|k| k.kind == "p" && k.columns.contains(&column.name));
            if declared {
                continue;
            }
            if let Some((parent, parent_column)) = guessed_parent(snapshot, column) {
                fks.push(Fk { child: t, child_column: c, parent, parent_column, guessed: true });
            }
        }
    }
    fks
}

/// The roles of every column and table, from tier S alone or with tier N.
pub fn infer(snapshot: &Snapshot, reads_n: bool) -> Schema<'_> {
    let fks = foreign_keys(snapshot);
    let mut tables: Vec<TableRoles> = snapshot
        .tables
        .iter()
        .enumerate()
        .map(|(t, table)| TableRoles {
            table: table.name.clone(),
            columns: (0..table.columns.len()).map(|c| column_roles(snapshot, t, c, &fks, reads_n)).collect(),
            ..Default::default()
        })
        .collect();

    for (t, table) in snapshot.tables.iter().enumerate() {
        if !is_base_table(table) {
            continue;
        }
        let roles = &tables[t].columns;
        let out = fks.iter().filter(|f| f.child == t).count();
        let referenced = fks.iter().filter(|f| f.parent == t && f.child != t).count();
        let has = |r: Role| roles.iter().any(|c| c.has(r));
        let primary = |r: Role| roles.iter().any(|c| c.primary() == Some(r));
        let mut held = Vec::new();

        if out >= 2 && (primary(Role::Money) || primary(Role::Quantity) || has(Role::Event)) {
            add(&mut held, TableRole::Fact, Kind::Declared, format!("{out} foreign keys out and a measure or a time"));
        }
        let timed = has(Role::Created) || has(Role::Event);
        if timed && !has(Role::Updated) {
            add(&mut held, TableRole::Event, Kind::Named, "a time and no update time");
            let ins = stat(&table.features, "n_tup_ins", reads_n);
            let upd = stat(&table.features, "n_tup_upd", reads_n);
            let del = stat(&table.features, "n_tup_del", reads_n);
            if let (Some(i), Some(u), Some(d)) = (ins, upd, del) {
                if i > 0.0 && u == 0.0 && d == 0.0 {
                    add(&mut held, TableRole::Event, Kind::Measured, "inserted, never updated nor deleted");
                }
            }
            if any(&tokens(&table.name), EVENT_TABLE) {
                add(&mut held, TableRole::Event, Kind::Named, "named as a log");
            }
        }
        if referenced >= 2 && has(Role::Created) {
            add(&mut held, TableRole::Entity, Kind::Declared, format!("referenced by {referenced} keys, with a creation time"));
        }
        let snapshot_key = table.constraints.iter().any(|k| {
            (k.kind == "u" || k.kind == "p")
                && k.columns.len() >= 2
                && k.columns.iter().any(|n| {
                    table.columns.iter().position(|c| &c.name == n).is_some_and(|c| roles[c].has(Role::Fk) || roles[c].has(Role::FkGuessed))
                })
                && k.columns.iter().any(|n| table.columns.iter().any(|c| &c.name == n && is_time(c)))
        });
        if snapshot_key {
            add(&mut held, TableRole::Snapshot, Kind::Declared, "unique over a key and a date");
        }
        let only_keys = roles.iter().zip(&table.columns).all(|(r, c)| {
            r.has(Role::Pk) || r.has(Role::Fk) || r.has(Role::FkGuessed) || is_time(c)
        });
        if out == 2 && only_keys {
            add(&mut held, TableRole::Link, Kind::Declared, "two foreign keys and nothing else");
        }
        let besides_key = roles.iter().filter(|r| !r.has(Role::Pk)).count();
        if referenced >= 1 && besides_key <= 3 && !only_keys {
            add(&mut held, TableRole::Lookup, Kind::Declared, format!("referenced, {besides_key} columns besides its key"));
            if rows(table, reads_n).is_some_and(|r| r < 100.0) {
                add(&mut held, TableRole::Lookup, Kind::Measured, "under 100 estimated rows");
            }
            let label = roles
                .iter()
                .position(|r| r.has(Role::Text) && !r.has(Role::Pk))
                .or_else(|| table.columns.iter().zip(roles).position(|(c, r)| is_text(c) && !r.has(Role::Pk)));
            tables[t].label = label.map(|l| table.columns[l].name.clone());
            tables[t].label_index = label;
        }
        tables[t].held = held;
    }

    Schema { snapshot, reads_n, tables, fks }
}
