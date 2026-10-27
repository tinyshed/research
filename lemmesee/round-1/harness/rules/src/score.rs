//! What each configuration may draw and in what order: the hard rules, the
//! abstentions and flags, the penalties and the top five of candidates.md.

use crate::generate::{Aggregate, Candidate, ColumnRef, Template};
use crate::names::tokens;
use crate::roles::{distinct, rows, stat, ColumnRoles, Kind, Role, Schema, TableRole};
use model::State;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Config {
    /// The schema's own order: the floor.
    O,
    /// Tier S, ordered by the penalties.
    A,
    /// Tiers S and N, ordered by the same penalties.
    B1,
}

impl Config {
    pub fn reads_n(self) -> bool {
        self == Config::B1
    }

    pub fn name(self) -> &'static str {
        match self {
            Config::O => "O",
            Config::A => "A",
            Config::B1 => "B1",
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Penalty {
    pub what: String,
    pub points: i32,
}

#[derive(Serialize, Clone, Debug)]
pub struct Scored {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstained: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub penalties: Vec<Penalty>,
    pub total: i32,
    /// Its place among what may be shown, before the top five's diversity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Panel {
    pub id: String,
    pub title: String,
    pub total: i32,
    /// The total with the top five's penalties for repeating a table or a
    /// column, as it was when the panel was chosen.
    pub chosen_at: i32,
}

/// The categories whose values decide which rows count: candidates.md's
/// `needs-population`.
const POPULATION: &[&str] = &["status", "state", "type", "kind"];

/// Words of an address: beside them `state` is a place, as Chinook's
/// `billing_state` is, not a status.
const ADDRESS: &[&str] = &["country", "city", "address", "postal", "zip", "region", "province"];

fn roles_of<'a>(schema: &'a Schema, r: &ColumnRef) -> &'a ColumnRoles {
    &schema.tables[r.t].columns[r.c]
}

fn column_of<'a>(schema: &'a Schema, r: &ColumnRef) -> &'a model::Column {
    &schema.snapshot.tables[r.t].columns[r.c]
}

/// The state of the statistics a candidate leans on, when this reading
/// reads them and they are not there.
fn stats_flag(schema: &Schema, r: &ColumnRef) -> Option<&'static str> {
    if !schema.reads_n {
        return None;
    }
    let column = column_of(schema, r);
    let states: Vec<State> = ["null_frac", "n_distinct"]
        .iter()
        .filter_map(|n| column.features.get(*n).map(|f| f.state))
        .collect();
    if states.contains(&State::Denied) {
        Some("stats-denied")
    } else if states.contains(&State::Stale) {
        Some("stats-stale")
    } else if !states.contains(&State::Present) {
        Some("stats-missing")
    } else {
        None
    }
}

fn hard_rule(c: &Candidate, schema: &Schema, config: Config) -> Option<&'static str> {
    let reads_n = config.reads_n();
    if c.template == Template::T8 {
        return (!reads_n).then_some("H0: needs tier N");
    }
    let t = c.t?;
    let table = &schema.snapshot.tables[t];
    let troles = &schema.tables[t];
    let summed = matches!(c.aggregate, Aggregate::Sum | Aggregate::Avg);

    if let Some(m) = &c.measure {
        let r = roles_of(schema, m);
        let banned = [Role::Pk, Role::Fk, Role::FkGuessed, Role::Identifier, Role::Code, Role::Place, Role::Calendar];
        if summed && banned.iter().any(|b| r.has(*b)) {
            return Some("H1: an identifier or a code summed");
        }
        if c.aggregate == Aggregate::Sum && [Role::Level, Role::Ratio, Role::Score].iter().any(|b| r.has(*b)) {
            return Some("H2: a level, ratio or score summed");
        }
        if c.aggregate == Aggregate::Sum && c.time.is_some() && troles.has(TableRole::Snapshot) {
            return Some("H2: a snapshot summed over time");
        }
    }
    if let Some(g) = c.category.as_ref().filter(|_| c.template != Template::T9) {
        let r = roles_of(schema, g);
        let gtable = &schema.snapshot.tables[g.t];
        let is_label = schema.tables[g.t].label_index == Some(g.c) && g.joined;
        if !(r.has(Role::Category) || is_label) || (r.has(Role::Text) && !is_label) {
            return Some("H3: grouped by what is not a category");
        }
        let n = distinct(gtable, column_of(schema, g), reads_n);
        if n.is_some_and(|n| !crate::roles::few_values(n, rows(gtable, reads_n))) {
            return Some("H3: grouped by too many values");
        }
        if c.template == Template::T6 && n.is_some_and(|n| n > 6.0) {
            return Some("T6: split by over 6 values");
        }
    }
    if let (Some(j), Some(m)) = (&c.join, &c.measure) {
        if m.joined || m.t != j.from.t {
            return Some("H4: aggregated after a join that repeats its rows");
        }
    }
    if let Some(m) = &c.measure {
        let currency = schema.tables[m.t].columns.iter().position(|r| r.has(Role::Currency));
        let grouped_by_it = c.category.as_ref().is_some_and(|g| g.t == m.t && Some(g.c) == currency);
        if roles_of(schema, m).has(Role::Money) && currency.is_some() && !grouped_by_it {
            return Some("H5: money in a table with a currency");
        }
    }
    if let Some(x) = &c.time {
        if roles_of(schema, x).axis().is_none() {
            return Some("H6: a time axis that is not a creation or event time");
        }
    }
    let lookup_or_link = troles.has(TableRole::Lookup) || troles.has(TableRole::Link);
    let children_per_parent = c.template == Template::T5 && c.measure.is_none();
    if lookup_or_link && !children_per_parent {
        return Some("H7: a lookup's or a link's own panel");
    }
    if rows(table, reads_n) == Some(0.0) {
        return Some("H8: a table estimated empty");
    }
    None
}

fn abstention(c: &Candidate, schema: &Schema) -> Option<&'static str> {
    let m = c.measure.as_ref()?;
    let money = roles_of(schema, m).has(Role::Money);
    let columns = &schema.tables[m.t].columns;
    let address = columns.iter().any(|r| tokens(&r.column).iter().any(|t| ADDRESS.contains(&t.as_str())));
    let population = columns.iter().position(|r| {
        r.primary() == Some(Role::Category)
            && tokens(&r.column)
                .iter()
                .any(|t| POPULATION.contains(&t.as_str()) && !(t == "state" && address))
    });
    let by_that = c.category.as_ref().is_some_and(|g| g.t == m.t && Some(g.c) == population);
    let sum_by_another =
        matches!(c.template, Template::T4 | Template::T9) && c.aggregate == Aggregate::Sum && !by_that;
    if population.is_some() && (c.template == Template::T3 || sum_by_another) {
        return Some("needs-population");
    }
    let parent_currency = c
        .join
        .as_ref()
        .is_some_and(|j| schema.tables[j.to.t].columns.iter().any(|r| r.has(Role::Currency)));
    if money && parent_currency {
        return Some("currencies");
    }
    if c.template == Template::T3 && roles_of(schema, m).has(Role::Level) {
        return Some("level-over-time");
    }
    None
}

/// Penalty points for how a role a candidate uses is known.
fn evidence_points(r: &ColumnRoles, role: Role) -> (i32, &'static str) {
    match r.get(role).map(|h| h.strongest()) {
        Some(Kind::Declared | Kind::Measured) | None => (0, ""),
        Some(Kind::Named) => (1, "named only"),
        Some(Kind::Typed) => (2, "typed only"),
    }
}

fn score_one(c: &Candidate, schema: &Schema) -> (Vec<String>, Vec<Penalty>) {
    let reads_n = schema.reads_n;
    let mut flags: Vec<String> = Vec::new();
    let mut penalties: Vec<Penalty> = Vec::new();
    let mut add = |what: String, points: i32| {
        if points != 0 {
            penalties.push(Penalty { what, points });
        }
    };

    // Sums first, counts beside them: a person looks for money and volume
    // over time before rows by a category
    let template_points = match (c.template, c.aggregate) {
        (Template::T1 | Template::T2, _) => 0,
        (Template::T9, _) => 1,
        (Template::T3 | Template::T4, Aggregate::Avg) => 1,
        (Template::T3 | Template::T4, _) => 0,
        (Template::T8, _) => 1,
        (Template::T5 | Template::T6 | Template::T7, _) => 2,
    };
    add(format!("template {:?}", c.template), template_points);
    let Some(t) = c.t else { return (flags, penalties) };
    let table = &schema.snapshot.tables[t];
    let troles = &schema.tables[t];

    // How the roles it uses are known
    let used: Vec<(&ColumnRef, Role)> = [
        c.time.as_ref().and_then(|x| roles_of(schema, x).axis().map(|r| (x, r))),
        c.category.as_ref().map(|g| (g, Role::Category)),
        c.measure.as_ref().and_then(|m| roles_of(schema, m).primary().map(|r| (m, r))),
    ]
    .into_iter()
    .flatten()
    .collect();
    for (r, role) in &used {
        let is_label = c.category.as_ref() == Some(*r) && schema.tables[r.t].label_index == Some(r.c) && r.joined;
        if is_label {
            continue;
        }
        let (points, how) = evidence_points(roles_of(schema, r), *role);
        add(format!("{} {how}", r.column), points);
    }
    if let Some(m) = &c.measure {
        let r = roles_of(schema, m);
        let col = column_of(schema, m);
        let named_money = r.get(Role::Money).is_some_and(|h| h.evidence.iter().any(|e| e.kind == Kind::Named));
        let cents = tokens(&col.name).iter().any(|t| t == "cents" || t == "kopecks");
        if named_money && crate::names::is_integer(col) && !cents {
            add(format!("{} named as money on an integer", m.column), 1);
        }
        let points = match r.primary() {
            Some(Role::Money) => -1,
            Some(Role::Quantity) => 0,
            Some(Role::Score | Role::Ratio) => 1,
            _ => 3,
        };
        add(format!("{} as a measure", m.column), points);
    }
    if let Some(x) = &c.time {
        if stat(&column_of(schema, x).features, "correlation", reads_n).is_some_and(|k| k >= 0.9) {
            add(format!("{} in physical order", x.column), -1);
        }
    }

    // The table
    let table_points = if troles.has(TableRole::Fact) {
        -2
    } else if troles.has(TableRole::Event) {
        -1
    } else if troles.has(TableRole::Entity) && c.template == Template::T1 {
        0
    } else {
        1
    };
    add(format!("{} as a table", table.name), table_points);
    let counts = c.aggregate == Aggregate::Count && c.measure.is_none() && c.template != Template::T8;
    if counts && troles.has(TableRole::Snapshot) {
        add(format!("rows of {}, a state at moments, counted", table.name), 2);
    }
    // A table of eight employees is not an overview's subject
    match rows(table, reads_n) {
        Some(n) if n < 50.0 => add(format!("{} under 50 rows", table.name), 5),
        Some(n) if n < 200.0 => add(format!("{} under 200 rows", table.name), 2),
        Some(n) if n < 1000.0 => add(format!("{} under 1 000 rows", table.name), 1),
        Some(_) => {}
        None => add(format!("{} rows unknown", table.name), 1),
    }

    // The category; a top list shows its twelve most whatever their number
    if let Some(g) = c.category.as_ref().filter(|_| c.template != Template::T9) {
        let col = column_of(schema, g);
        let gtable = &schema.snapshot.tables[g.t];
        let declared = (!col.enum_labels.is_empty()).then(|| col.enum_labels.len() as f64)
            .or_else(|| crate::names::is_bool(col).then_some(2.0));
        match declared.or_else(|| distinct(gtable, col, reads_n)) {
            Some(n) if (3.0..=12.0).contains(&n) => {}
            Some(n) if n <= 2.0 => add(format!("{} of {n} values", g.column), 2),
            Some(n) => add(format!("{} of {n} values", g.column), 1),
            None => add(format!("{} of unknown size", g.column), 1),
        }
        if stat(&col.features, "mcv_top_freq", reads_n).is_some_and(|f| f > 0.95) {
            add(format!("{} nearly one value", g.column), 3);
        }
    }
    for (r, _) in &used {
        if stat(&column_of(schema, r).features, "null_frac", reads_n).is_some_and(|f| f > 0.5) {
            add(format!("{} mostly null", r.column), 2);
        }
    }

    // The join
    if let Some(j) = &c.join {
        add(format!("join to {}", j.to.table), if j.guessed { 2 } else { 1 });
        if j.guessed {
            flags.push("join-guessed".into());
        }
    }

    // Flags
    if let Some(flag) = used.iter().find_map(|(r, _)| stats_flag(schema, r)) {
        flags.push(flag.into());
    }
    if troles.columns.iter().any(|r| r.has(Role::Deleted)) {
        flags.push("soft-deleted".into());
    }
    if let Some(x) = &c.time {
        let xtable = &schema.tables[x.t];
        if xtable.columns.iter().filter(|r| r.axis().is_some()).count() > 1 {
            flags.push("time-ambiguous".into());
        }
        let col = column_of(schema, x);
        let n = stat(&col.features, "n_distinct", reads_n);
        let all = rows(&schema.snapshot.tables[x.t], reads_n);
        let clumped = match (n, all) {
            (Some(n), Some(all)) if n > 0.0 && all > 0.0 => n / all < 0.01,
            (Some(n), _) if n < 0.0 => -n < 0.01,
            _ => false,
        };
        if clumped {
            flags.push("time-clumped".into());
        }
    }
    if c.template == Template::T7 {
        flags.push("shows-rows".into());
    }
    for f in &flags {
        let points = match f.as_str() {
            "time-clumped" => 3,
            f if f.starts_with("stats-") => 0,
            _ => 1,
        };
        add(format!("flag {f}"), points);
    }
    (flags, penalties)
}

fn reasons(c: &Candidate, schema: &Schema) -> Vec<String> {
    let mut out = Vec::new();
    for r in [&c.time, &c.category, &c.measure].into_iter().flatten() {
        for h in &roles_of(schema, r).held {
            for e in &h.evidence {
                out.push(format!("{}.{}: {}", r.table, r.column, e.detail));
            }
        }
    }
    if let Some(t) = c.t {
        for h in &schema.tables[t].held {
            for e in &h.evidence {
                out.push(format!("{}: {}", schema.snapshot.tables[t].name, e.detail));
            }
        }
    }
    out
}

/// Scores every candidate for one configuration, and chooses its five.
pub fn score(candidates: &[Candidate], schema: &Schema, config: Config) -> (Vec<Scored>, Vec<Panel>) {
    let mut scored: Vec<Scored> = candidates
        .iter()
        .map(|c| {
            let removed_by = hard_rule(c, schema, config).map(str::to_string);
            let (flags, penalties) = if config == Config::O { (Vec::new(), Vec::new()) } else { score_one(c, schema) };
            let abstained = if config == Config::O { None } else { abstention(c, schema).map(str::to_string) };
            Scored {
                id: c.id.clone(),
                title: c.title.clone(),
                total: penalties.iter().map(|p| p.points).sum(),
                removed_by,
                abstained,
                flags,
                penalties,
                rank: None,
                reasons: if config == Config::O { Vec::new() } else { reasons(c, schema) },
            }
        })
        .collect();

    let mut eligible: Vec<usize> = (0..candidates.len())
        .filter(|&i| scored[i].removed_by.is_none() && scored[i].abstained.is_none())
        .collect();
    eligible.sort_by_key(|&i| (scored[i].total, candidates[i].order));
    for (rank, &i) in eligible.iter().enumerate() {
        scored[i].rank = Some(rank + 1);
    }

    let mut panels: Vec<Panel> = Vec::new();
    if config == Config::O {
        for &i in eligible.iter().take(5) {
            panels.push(Panel { id: scored[i].id.clone(), title: scored[i].title.clone(), total: 0, chosen_at: 0 });
        }
        return (scored, panels);
    }
    let mut per_table: BTreeMap<String, usize> = BTreeMap::new();
    let mut chosen: Vec<usize> = Vec::new();
    while panels.len() < 5 {
        let next = eligible
            .iter()
            .filter(|i| !chosen.contains(i))
            .filter(|&&i| per_table.get(&candidates[i].table).copied().unwrap_or(0) < 2)
            .map(|&i| {
                let c = &candidates[i];
                let same_table = per_table.get(&c.table).copied().unwrap_or(0) as i32;
                let same_column = chosen.iter().any(|&k| {
                    candidates[k].template == c.template
                        && candidates[k].main_column().is_some()
                        && candidates[k].main_column() == c.main_column()
                });
                let same_template = chosen.iter().filter(|&&k| candidates[k].template == c.template).count() as i32;
                let at = scored[i].total + 3 * same_table + 2 * same_template + if same_column { 5 } else { 0 };
                (at, c.order, i)
            })
            .min();
        let Some((at, _, i)) = next else { break };
        chosen.push(i);
        *per_table.entry(candidates[i].table.clone()).or_default() += 1;
        panels.push(Panel { id: scored[i].id.clone(), title: scored[i].title.clone(), total: scored[i].total, chosen_at: at });
    }
    (scored, panels)
}
