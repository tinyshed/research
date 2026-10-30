//! The screen document `export.py` writes: a store, its row of numbers, the
//! screen's five panels and, for every role, the panels next in the ranker's
//! order.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Doc {
    pub store: Store,
    pub numbers: Vec<Number>,
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub spares: BTreeMap<String, Vec<Panel>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Store {
    pub name: String,
    pub engine: String,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub database: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Number {
    pub key: String,
    pub title: String,
    pub shown: String,
    #[serde(default)]
    pub kind: String,
    /// A funnel's steps, the titles of the numbers it is made of.
    #[serde(default)]
    pub parts: Vec<String>,
    /// What the value is when it is not the whole's count: a sample, the catalog.
    #[serde(default)]
    pub estimate: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Panel {
    pub key: String,
    pub title: String,
    pub role: String,
    pub template: String,
    pub bucket: Option<String>,
    #[serde(default)]
    pub rows: Vec<Vec<Option<String>>>,
    pub error: Option<String>,
    /// What the rows are when they are not the whole's: a sample, the newest rows.
    #[serde(default)]
    pub estimate: Option<String>,
}

impl Panel {
    /// How the panel is drawn, by what its rows hold.
    pub fn shape(&self) -> Shape {
        if self.error.is_some() || self.rows.is_empty() {
            return Shape::Empty;
        }
        let width = self.rows[0].len();
        if self.rows.len() == 1 && width == 1 {
            return Shape::Number;
        }
        if self.bucket.is_some() && width >= 3 {
            return Shape::Split;
        }
        if self.bucket.is_some() {
            return Shape::Line;
        }
        Shape::Bars
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Line,
    Split,
    Bars,
    Number,
    Empty,
}

pub fn num(v: &Option<String>) -> Option<f64> {
    v.as_deref().and_then(|s| s.trim().parse::<f64>().ok()).filter(|x| x.is_finite())
}

pub fn label(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "∅".to_string())
}

/// A word naming a column through its table, `loan.date_loan`, as table and column.
fn dotted(w: &str) -> Option<(String, String)> {
    w.split_once('.')
        .filter(|(t, c)| !c.is_empty() && t.chars().next().is_some_and(|ch| ch.is_alphabetic() || ch == '_') && t.chars().all(|ch| ch.is_alphanumeric() || ch == '_'))
        .map(|(t, c)| (t.to_string(), c.to_string()))
}

/// A title with each table named once: `loan.date_loan` after `loan` reads
/// `date_loan`.
pub fn once(title: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    title
        .split_whitespace()
        .map(|w| match dotted(w) {
            Some((t, c)) if seen.contains(&t) => c,
            Some((t, _)) => {
                seen.push(t);
                w.to_string()
            }
            None => {
                seen.push(w.trim_end_matches(',').to_string());
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A title with no table before any column, when even once is too long.
pub fn bare(title: &str) -> String {
    title.split_whitespace().map(|w| dotted(w).map(|(_, c)| c).unwrap_or_else(|| w.to_string())).collect::<Vec<_>>().join(" ")
}

/// A day as `Oct 29`; anything else as it came.
pub fn short_date(s: &str) -> String {
    const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    match (s.get(5..7).and_then(|m| m.parse::<usize>().ok()), s.get(8..10).and_then(|d| d.parse::<u32>().ok())) {
        (Some(m), Some(d)) if (1..=12).contains(&m) && s.len() >= 10 => format!("{} {}", M[m - 1], d),
        _ => s.to_string(),
    }
}

/// A bucket's start as its bucket reads: a day or week as `Oct 29`, a month
/// as `Oct 2026`, a year as `2026`.
pub fn when(s: &str, bucket: Option<&str>) -> String {
    match bucket {
        Some("year") => s.get(..4).unwrap_or(s).to_string(),
        Some("month") => short_date(s).split_once(' ').map(|(m, _)| format!("{m} {}", s.get(..4).unwrap_or(""))).unwrap_or_else(|| s.to_string()),
        _ => short_date(s),
    }
}

/// Today as the database writes a day, in UTC: the day a chart's last point
/// may only have begun.
pub fn today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // days to a civil date, after Howard Hinnant
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// A number as a person reads it on a screen.
pub fn fmt(x: f64) -> String {
    let a = x.abs();
    if a >= 1e9 {
        format!("{:.1}B", x / 1e9)
    } else if a >= 1e6 {
        format!("{:.1}M", x / 1e6)
    } else if a >= 1e4 {
        format!("{:.0}k", x / 1e3)
    } else if a >= 100.0 || x.fract() == 0.0 {
        format!("{x:.0}")
    } else {
        format!("{x:.2}")
    }
}
