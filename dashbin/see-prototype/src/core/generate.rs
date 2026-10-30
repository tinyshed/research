//! Round 1c's generator: every question a schema can be asked, as `aq.generate`
//! makes them, in its order.

use std::collections::HashSet;

use super::names::*;
use super::question::{Dim, Identity, Measure, Question, identity, named};
use super::schema::Schema;

/// Every chain of keys from the base, many to one, no table twice.
fn paths(schema: &Schema, base: usize, depth: usize) -> Vec<Vec<(usize, usize)>> {
    let mut out: Vec<Vec<(usize, usize)>> = vec![vec![]];
    let mut frontier: Vec<(Vec<(usize, usize)>, usize)> = vec![(vec![], base)];
    for _ in 0..depth {
        let mut next = Vec::new();
        for (path, t) in &frontier {
            let mut seen: HashSet<Option<usize>> = HashSet::from([Some(base)]);
            for s in path {
                seen.insert(schema.tables[s.0].columns[s.1].fk);
            }
            for &(c, p) in &schema.tables[*t].out {
                if seen.contains(&Some(p)) {
                    continue;
                }
                let mut step = path.clone();
                step.push((*t, c));
                out.push(step.clone());
                next.push((step, p));
            }
        }
        frontier = next;
    }
    out
}

fn end(schema: &Schema, base: usize, path: &[(usize, usize)]) -> usize {
    match path.last() {
        Some(&(t, c)) => schema.tables[t].columns[c].fk.unwrap(),
        None => base,
    }
}

pub fn generate(schema: &Schema) -> Vec<Question> {
    let max_depth = 3;
    let mut qs: Vec<Question> = Vec::new();
    let mut seen: HashSet<(Identity, &'static str)> = HashSet::new();
    let mut add = |q: Question, qs: &mut Vec<Question>| {
        let k = (identity(&q), q.shape);
        if seen.insert(k) {
            qs.push(q);
        }
    };
    for base in &schema.tables {
        if base.rows == Some(0.0) {
            continue;
        }
        let b = base.t;
        let paths = paths(schema, b, max_depth);
        if base.role == "link" {
            for path in &paths {
                if path.len() == 1 {
                    let p = end(schema, b, path);
                    let pt = &schema.tables[p];
                    let kind = if named(pt) { if pt.person { "person" } else { "label" } } else { "key" };
                    add(Question { base: b, measure: Measure::count(), time: None, by: Some(Dim { path: path.clone(), t: p, c: -1, kind }), shape: "top" }, &mut qs);
                    for c in &pt.columns {
                        if let Some(cat) = c.category {
                            if cat != "typed" {
                                add(Question { base: b, measure: Measure::count(), time: None, by: Some(Dim { path: path.clone(), t: p, c: c.c as i64, kind: cat }), shape: "breakdown" }, &mut qs);
                            }
                        }
                    }
                }
            }
            continue;
        }
        let cols = &base.columns;
        let mut measures = vec![Measure::count()];
        let money: Vec<_> = cols.iter().filter(|c| c.measure == Some("money")).collect();
        let qty: Vec<_> = cols.iter().filter(|c| c.measure == Some("quantity")).collect();
        let disc: Vec<_> = cols.iter().filter(|c| c.measure == Some("ratio") && has(&c.tk, &["discount"])).collect();
        for c in cols {
            if matches!(c.measure, Some("money") | Some("quantity") | Some("number")) {
                measures.push(Measure::new("sum", vec![(b, c.c)], "column"));
            }
            if matches!(c.measure, Some("money") | Some("quantity") | Some("number") | Some("ratio") | Some("score")) {
                measures.push(Measure::new("avg", vec![(b, c.c)], "column"));
            }
        }
        for m in &money {
            for q in &qty {
                if has(&q.tk, UNITS) && has(&m.tk, &["price", "cost", "fee", "rate", "charge", "value"]) {
                    if !disc.is_empty() {
                        for d in &disc {
                            measures.push(Measure::new("sum", vec![(b, m.c), (b, q.c), (b, d.c)], "net"));
                        }
                    } else {
                        measures.push(Measure::new("sum", vec![(b, m.c), (b, q.c)], "product"));
                    }
                }
            }
        }
        if !qty.is_empty() && money.is_empty() {
            for &(_, p) in &base.out {
                for pm in &schema.tables[p].columns {
                    if p != b && pm.measure == Some("money") && has(&pm.tk, &["price", "cost", "fee", "rate", "value"]) {
                        for q in &qty {
                            if has(&q.tk, UNITS) {
                                measures.push(Measure::new("sum", vec![(b, q.c), (p, pm.c)], "product"));
                            }
                        }
                    }
                }
            }
        }
        for &(c, p) in &base.out {
            if p != b && schema.tables[p].role != "lookup" && !schema.tables[p].boilerplate {
                measures.push(Measure::new("distinct", vec![(b, c)], "parents"));
            }
        }
        if base.timed {
            for c in cols {
                if c.fk.is_none() && c.key && !c.pk && ["int", "text", "uuid"].contains(&c.fam) && norm(&c.name).ends_with("id") {
                    measures.push(Measure::new("distinct", vec![(b, c.c)], "parents"));
                }
            }
        }
        let flags: Vec<_> = cols.iter().filter(|c| c.fam == "bool" || c.category == Some("bool")).collect();
        for fl in flags.iter().take(3) {
            measures.push(Measure::new("count_if", vec![(b, fl.c)], "flagged"));
        }
        for &(c, p) in &base.out {
            if p != b && !schema.tables[p].boilerplate {
                add(Question { base: b, measure: Measure::new("per", vec![(b, c)], "per"), time: None, by: None, shape: "distribution" }, &mut qs);
            }
        }
        let times: Vec<_> = cols.iter().filter(|c| matches!(c.time, Some("created") | Some("event")) && (c.fam == "timestamp" || c.fam == "date")).collect();
        for x in &times {
            for y in &times {
                if x.c == y.c {
                    continue;
                }
                if has(&x.tk, PLAN) && has(&y.tk, ACTUAL) && !has(&x.tk, ACTUAL) {
                    measures.push(Measure::new("avg", vec![(b, x.c), (b, y.c)], "gap"));
                    if has(&x.tk, &["required", "due", "scheduled", "expected", "promised", "planned", "target"]) {
                        measures.push(Measure::new("late", vec![(b, x.c), (b, y.c)], "late"));
                    }
                }
            }
        }
        // times: the base's own, else the nearest parent's
        let mut tdims: Vec<Dim> = cols
            .iter()
            .filter(|c| matches!(c.time, Some("created") | Some("event") | Some("int")))
            .map(|c| Dim { path: vec![], t: b, c: c.c as i64, kind: c.time.unwrap() })
            .collect();
        if tdims.is_empty() {
            for depth in [1, 2] {
                for path in &paths {
                    if path.len() != depth {
                        continue;
                    }
                    let p = end(schema, b, path);
                    if !matches!(schema.tables[p].role, "fact" | "event" | "snapshot") {
                        continue;
                    }
                    for c in &schema.tables[p].columns {
                        if matches!(c.time, Some("created") | Some("event") | Some("int")) {
                            tdims.push(Dim { path: path.clone(), t: p, c: c.c as i64, kind: c.time.unwrap() });
                        }
                    }
                }
                if !tdims.is_empty() {
                    break;
                }
            }
        }
        // breakdowns
        let mut bdims: Vec<Dim> = Vec::new();
        for c in cols {
            if let Some(cat) = c.category {
                bdims.push(Dim { path: vec![], t: b, c: c.c as i64, kind: cat });
            } else if c.label.is_some() && matches!(base.role, "fact" | "event" | "snapshot") {
                bdims.push(Dim { path: vec![], t: b, c: c.c as i64, kind: "own_label" });
            }
        }
        for path in &paths {
            if path.is_empty() {
                continue;
            }
            let p = end(schema, b, path);
            let pt = &schema.tables[p];
            if pt.boilerplate {
                continue;
            }
            if named(pt) {
                bdims.push(Dim { path: path.clone(), t: p, c: -1, kind: if pt.person { "person" } else { "label" } });
            } else if path.len() <= 2 {
                bdims.push(Dim { path: path.clone(), t: p, c: -1, kind: "key" });
            }
            if path.len() <= 2 {
                for c in &pt.columns {
                    if let Some(cat) = c.category {
                        if cat != "typed" || path.len() == 1 {
                            bdims.push(Dim { path: path.clone(), t: p, c: c.c as i64, kind: cat });
                        }
                    }
                }
            }
        }
        for c in cols {
            if c.fk.is_none() && c.key && !(c.pk && base.pk.len() == 1) && (c.fam == "int" || c.fam == "text") && norm(&c.name).ends_with("id") {
                bdims.push(Dim { path: vec![], t: b, c: c.c as i64, kind: "key" });
            }
        }
        for m in &measures {
            for td in &tdims {
                add(Question { base: b, measure: m.clone(), time: Some(td.clone()), by: None, shape: "series" }, &mut qs);
            }
            for bd in &bdims {
                let col = if bd.c >= 0 { Some(&schema.tables[bd.t].columns[bd.c as usize]) } else { None };
                let many = bd.c == -1 || bd.kind == "key" || bd.kind == "own_label" || col.is_some_and(|c| c.distinct.unwrap_or(0.0) > 12.0);
                add(Question { base: b, measure: m.clone(), time: None, by: Some(bd.clone()), shape: if many { "top" } else { "breakdown" } }, &mut qs);
                let pt = &schema.tables[bd.t];
                let small_lookup = bd.c == -1 && bd.kind == "label" && pt.role == "lookup" && pt.rows.is_none_or(|r| r <= 12.0);
                let few_values = bd.c >= 0 && matches!(bd.kind, "named" | "measured" | "bool" | "geo" | "typed") && col.is_some_and(|c| c.distinct.is_none_or(|d| d <= 8.0));
                if matches!(m.kind, "rows" | "column" | "product" | "net" | "flagged" | "parents") && (few_values || small_lookup) {
                    for td in tdims.iter().take(2) {
                        add(Question { base: b, measure: m.clone(), time: Some(td.clone()), by: Some(bd.clone()), shape: "split" }, &mut qs);
                    }
                }
            }
        }
    }
    qs
}
