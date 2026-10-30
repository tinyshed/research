//! The screen document made in Rust, as `export.py` makes it in Python: the
//! row of numbers, round 1o's screen of five, and for every role the next
//! questions in the ranker's order, every one run.

use std::collections::{BTreeMap, HashSet};

use serde_json::{Value, json};

use super::charts::{self, Item, Ran};
use super::model::Model;
use super::numbers;
use super::question::Identity;
use super::rank::{order, slots};
use super::roles::{role, timed_role};
use super::run::Runner;
use super::schema;

const SPARE: usize = 4;

/// What the build is doing, for a screen to show while it waits.
pub enum Event {
    /// a step begun
    Step(&'static str),
    /// the step done, and what it found
    Done(String),
    /// panels run of all there are to run
    Ran(usize, usize),
}

fn panel(p: Ran, role: &str, run: &Runner) -> Value {
    let estimate = run.note(&p.sql);
    json!({"key": p.title, "title": p.title, "role": role, "template": p.template, "bucket": p.bucket,
           "rows": p.rows.into_iter().take(200).collect::<Vec<_>>(), "error": p.error, "estimate": estimate})
}

pub fn build(snap: &Value, name: &str, about: &str, model: &Model, run: &Runner, tell: &(dyn Fn(Event) + Sync)) -> Value {
    tell(Event::Step("reading the schema"));
    let s = schema::load(snap, true);
    tell(Event::Done(format!("{} tables, {} keys", s.tables.len(), s.tables.iter().map(|t| t.out.len()).sum::<usize>())));
    tell(Event::Step("counting the numbers"));
    let tiles = numbers::row(&s, run);
    tell(Event::Done(format!("{} numbers", tiles.len())));
    tell(Event::Step("making and ranking the questions"));
    let r = charts::ranked(&s, model);
    tell(Event::Done(format!("{} questions", r.qs.len())));
    tell(Event::Step("choosing the screen"));
    let (five, _) = charts::screen(&s, &r, run);
    tell(Event::Done(format!("{} panels", five.len())));
    let title_of = |i: &Item| match i {
        Item::C(c) => c.title.clone(),
        Item::Q(q) => q.title(&s),
    };
    let on_screen: HashSet<String> = five.iter().map(title_of).collect();
    let mut spares: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut seen: HashSet<Identity> = HashSet::new();
    for i in order(&r.scores) {
        let q = &r.qs[i];
        let ro = timed_role(&r.ids[i], &r.slots[i]).unwrap_or_else(|| role(&r.ids[i], &r.slots[i]));
        if on_screen.contains(&q.title(&s)) || seen.contains(&r.ids[i]) || spares.get(ro).map_or(0, Vec::len) >= SPARE {
            continue;
        }
        seen.insert(r.ids[i].clone());
        spares.entry(ro).or_default().push(i);
    }
    let role_of = |i: &Item| -> String {
        match i {
            Item::C(c) => match c.unit {
                Some("share") => "health".into(),
                Some("seconds") => "speed".into(),
                _ => if c.template == "T4" { "money".into() } else { "other".into() },
            },
            Item::Q(q) => {
                let id = super::question::identity(q);
                let sl = slots(&s, q);
                timed_role(&id, &sl).unwrap_or_else(|| role(&id, &sl)).to_string()
            }
        }
    };
    tell(Event::Step("running the panels"));
    let total = five.len() + spares.values().map(Vec::len).sum::<usize>();
    let mut ran = 0;
    let mut panels: Vec<Value> = Vec::new();
    for i in &five {
        panels.push(panel(charts::run_item(&s, i, run), &role_of(i), run));
        ran += 1;
        tell(Event::Ran(ran, total));
    }
    let mut reserve = serde_json::Map::new();
    for (ro, idx) in &spares {
        let mut v: Vec<Value> = Vec::new();
        for &i in idx {
            v.push(panel(charts::run_question(&s, &r.qs[i], run), ro, run));
            ran += 1;
            tell(Event::Ran(ran, total));
        }
        reserve.insert(ro.to_string(), Value::Array(v));
    }
    tell(Event::Done(format!("{ran} run, {} spare", ran - five.len())));
    json!({
        "store": {"name": name, "engine": s.engine, "database": snap.get("database").cloned().unwrap_or(Value::Null), "about": about},
        "numbers": tiles.iter().map(|t| json!({"key": t.title, "title": t.title, "shown": numbers::shown(t, &tiles), "kind": t.kind, "parts": t.parts, "estimate": t.estimate})).collect::<Vec<_>>(),
        "panels": panels,
        "spares": reserve,
    })
}
