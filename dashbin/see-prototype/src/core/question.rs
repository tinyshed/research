//! The analytical question of round 1c: a base whose rows are aggregated, a
//! measure, at most a time and a breakdown, and a shape; its identity, by
//! which questions are told apart whatever words they use; and its title.

use super::names::{DELETED, has};
use super::schema::{Schema, Table};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Measure {
    pub agg: &'static str,
    pub cols: Vec<(usize, usize)>,
    pub kind: &'static str,
}

impl Measure {
    pub fn count() -> Measure {
        Measure { agg: "count", cols: vec![], kind: "rows" }
    }
    pub fn new(agg: &'static str, cols: Vec<(usize, usize)>, kind: &'static str) -> Measure {
        Measure { agg, cols, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dim {
    pub path: Vec<(usize, usize)>,
    pub t: usize,
    /// the column, or -1 for a parent's identity (its label)
    pub c: i64,
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Question {
    pub base: usize,
    pub measure: Measure,
    pub time: Option<Dim>,
    pub by: Option<Dim>,
    pub shape: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum By {
    Entity(usize),
    Col(usize, usize),
}

/// (base, (aggregate, sorted columns), time column, breakdown)
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Identity {
    pub base: usize,
    pub agg: &'static str,
    pub cols: Vec<(usize, usize)>,
    pub time: Option<(usize, usize)>,
    pub by: Option<By>,
}

pub fn identity(q: &Question) -> Identity {
    let mut cols = q.measure.cols.clone();
    cols.sort();
    cols.dedup();
    Identity {
        base: q.base,
        agg: if q.measure.kind == "parents" { "distinct" } else { q.measure.agg },
        cols,
        time: q.time.as_ref().map(|d| (d.t, d.c as usize)),
        by: q.by.as_ref().map(|d| if d.c == -1 { By::Entity(d.t) } else { By::Col(d.t, d.c as usize) }),
    }
}

/// A table's rows have names to group them by, and it records no events.
pub fn named(tb: &Table) -> bool {
    (tb.label.is_some() || tb.first_last.is_some()) && !tb.timed && tb.detail_of.is_none()
}

pub fn dim_name(schema: &Schema, d: &Dim) -> String {
    let tb = &schema.tables[d.t];
    if d.c == -1 {
        if d.kind == "key" || !named(tb) {
            return format!("{} (by key)", tb.name);
        }
        if tb.first_last.is_some() {
            return format!("{} (named)", tb.name);
        }
        return format!("{}.{}", tb.name, tb.columns[tb.label.unwrap()].name);
    }
    format!("{}.{}", tb.name, tb.columns[d.c as usize].name)
}

/// The columns that say a row was deleted, and how a live row reads them.
pub fn alive(tb: &Table) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    for c in &tb.columns {
        if c.time == Some("deleted") {
            out.push((c.c, "null"));
        } else if (c.fam == "bool" || c.category == Some("bool")) && has(&c.tk, DELETED) {
            out.push((c.c, "off"));
        }
    }
    out
}

impl Question {
    pub fn title(&self, schema: &Schema) -> String {
        let b = &schema.tables[self.base].name;
        let m = &self.measure;
        let cname = |tc: (usize, usize)| format!("{}.{}", schema.tables[tc.0].name, schema.tables[tc.0].columns[tc.1].name);
        let what = match m.kind {
            "rows" => format!("rows of {b}"),
            "column" => format!("{} of {}", if m.agg == "sum" { "sum" } else { "average" }, cname(m.cols[0])),
            "product" | "net" => {
                let mut s = format!("sum of {}", m.cols[..2].iter().map(|&c| cname(c)).collect::<Vec<_>>().join(" × "));
                if m.kind == "net" {
                    s += &format!(" × (1 − {})", cname(m.cols[2]));
                }
                s
            }
            "parents" => {
                let col = schema.col(m.cols[0]);
                match col.fk {
                    Some(fk) => format!("distinct {} in {b}", schema.tables[fk].name),
                    None => format!("distinct {} in {b}", cname(m.cols[0])),
                }
            }
            "flagged" => format!("rows of {b} with {}", cname(m.cols[0])),
            "gap" => format!("average days from {} to {}", cname(m.cols[0]), cname(m.cols[1])),
            "per" => format!("rows of {b} per {}", schema.tables[schema.col(m.cols[0]).fk.unwrap()].name),
            _ => format!("rows of {b} with {} after {}", cname(m.cols[1]), cname(m.cols[0])),
        };
        let mut parts = vec![what];
        if let Some(t) = &self.time {
            parts.push(format!("by {}", dim_name(schema, t)));
        }
        if let Some(by) = &self.by {
            parts.push(format!("{}{}", if self.time.is_some() { "split by " } else { "by " }, dim_name(schema, by)));
        }
        if self.shape == "top" {
            parts.push("the twelve most".into());
            return parts.join(", ");
        }
        parts.join(" ")
    }
}
