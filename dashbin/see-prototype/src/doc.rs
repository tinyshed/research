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
}

#[derive(Debug, Clone, Deserialize)]
pub struct Number {
    pub key: String,
    pub title: String,
    pub shown: String,
    #[serde(default)]
    pub kind: String,
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
