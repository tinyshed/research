//! A snapshot read as round 1c's `aq.load` reads it: tables and columns, the
//! keys declared and guessed, what each column may be — a time, a measure, a
//! category, a label — and what each table is.

use std::collections::HashMap;

use serde_json::Value;

use super::names::*;

#[derive(Debug, Clone)]
pub struct Column {
    pub t: usize,
    pub c: usize,
    pub name: String,
    pub type_name: String,
    pub fam: &'static str,
    pub nullable: bool,
    pub tk: Vec<String>,
    pub distinct: Option<f64>,
    pub null_frac: Option<f64>,
    pub top_freq: Option<f64>,
    pub pk: bool,
    pub unique: bool,
    pub fk: Option<usize>,
    pub fk_guessed: bool,
    pub time: Option<&'static str>,
    pub measure: Option<&'static str>,
    pub category: Option<&'static str>,
    pub label: Option<&'static str>,
    pub key: bool,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub t: usize,
    pub name: String,
    pub tk: Vec<String>,
    pub rows: Option<f64>,
    pub columns: Vec<Column>,
    pub pk: Vec<usize>,
    pub boilerplate: bool,
    pub out: Vec<(usize, usize)>,
    pub referenced_by: Vec<(usize, usize)>,
    pub role: &'static str,
    pub timed: bool,
    pub subject: Option<usize>,
    pub detail_of: Option<usize>,
    pub label: Option<usize>,
    pub person: bool,
    pub first_last: Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub engine: String,
    pub tables: Vec<Table>,
    pub reads_n: bool,
}

impl Schema {
    pub fn col(&self, r: (usize, usize)) -> &Column {
        &self.tables[r.0].columns[r.1]
    }
}

fn stat(features: &Value, name: &str, reads_n: bool) -> Option<f64> {
    if !reads_n {
        return None;
    }
    let f = features.get(name)?;
    let state = f.get("state").and_then(Value::as_str)?;
    if state != "present" && state != "stale" {
        return None;
    }
    match f.get("value")? {
        Value::Number(n) => n.as_f64(),
        // Python's bool is an int
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

pub fn load(snap: &Value, reads_n: bool) -> Schema {
    let empty = Vec::new();
    let raw: Vec<&Value> = snap
        .get("tables")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
        .iter()
        .filter(|t| {
            let k = t.get("kind").and_then(Value::as_str).unwrap_or("table");
            k == "table" || k == "partitioned"
        })
        .collect();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (t, rt) in raw.iter().enumerate() {
        let schema = rt.get("schema").and_then(Value::as_str).unwrap_or("public");
        index.insert(format!("{}.{}", schema, s(rt, "name")).to_lowercase(), t);
        index.entry(s(rt, "name").to_lowercase()).or_insert(t);
    }
    let mut tables = Vec::new();
    let none = Value::Null;
    for (t, rt) in raw.iter().enumerate() {
        let feats = rt.get("features").unwrap_or(&none);
        let rows = stat(feats, "rows_estimate", reads_n);
        let constraints = rt.get("constraints").and_then(Value::as_array).cloned().unwrap_or_default();
        let pk: Vec<String> = constraints.iter().find(|k| s(k, "kind") == "p").map(|k| strings(&k["columns"])).unwrap_or_default();
        let mut uniques: Vec<Vec<String>> = constraints.iter().filter(|k| s(k, "kind") == "u").map(|k| strings(&k["columns"])).collect();
        if let Some(ix) = rt.get("indexes").and_then(Value::as_array) {
            uniques.extend(ix.iter().filter(|i| i.get("unique").and_then(Value::as_bool).unwrap_or(false)).map(|i| strings(&i["columns"])));
        }
        let mut cols = Vec::new();
        for (c, rc) in rt.get("columns").and_then(Value::as_array).unwrap_or(&empty).iter().enumerate() {
            let cf = rc.get("features").unwrap_or(&none);
            let name = s(rc, "name").to_string();
            let mut n = stat(cf, "n_distinct", reads_n).or_else(|| stat(cf, "index_distinct", reads_n));
            if let (Some(v), Some(r)) = (n, rows) {
                if v < 0.0 && r != 0.0 {
                    n = Some(-v * r);
                }
            }
            let distinct = match n {
                Some(v) if v >= 0.0 => Some(v),
                Some(_) => None,
                None => None,
            };
            let is_pk = pk.contains(&name);
            cols.push(Column {
                t,
                c,
                tk: tokens(&name),
                type_name: s(rc, "type_name").to_string(),
                fam: family(s(rc, "type_name"), s(rc, "base_type")),
                nullable: rc.get("nullable").and_then(Value::as_bool).unwrap_or(true),
                distinct,
                null_frac: stat(cf, "null_frac", reads_n),
                top_freq: stat(cf, "mcv_top_freq", reads_n),
                pk: is_pk,
                unique: (is_pk && pk.len() == 1) || uniques.iter().any(|u| u.len() == 1 && u[0] == name),
                fk: None,
                fk_guessed: false,
                time: None,
                measure: None,
                category: None,
                label: None,
                key: false,
                name,
            });
        }
        let name = s(rt, "name").to_string();
        tables.push(Table {
            t,
            tk: tokens(&name),
            rows,
            pk: cols.iter().filter(|c| c.pk).map(|c| c.c).collect(),
            columns: cols,
            boilerplate: BOILERPLATE.is_match(&name),
            out: Vec::new(),
            referenced_by: Vec::new(),
            role: "table",
            timed: false,
            subject: None,
            detail_of: None,
            label: None,
            person: false,
            first_last: None,
            name,
        });
    }
    let mut schema = Schema { engine: s(snap, "engine").to_string(), tables, reads_n };
    // declared keys of one column
    for (t, rt) in raw.iter().enumerate() {
        for k in rt.get("constraints").and_then(Value::as_array).unwrap_or(&empty) {
            let cols = strings(&k["columns"]);
            if s(k, "kind") != "f" || cols.len() != 1 {
                continue;
            }
            let rt_name = s(k, "ref_table").to_lowercase();
            let p = index.get(&rt_name).or_else(|| index.get(rt_name.rsplit('.').next().unwrap_or(""))).copied();
            let Some(p) = p else { continue };
            if let Some(c) = schema.tables[t].columns.iter_mut().find(|x| x.name == cols[0]) {
                if c.fk.is_none() {
                    c.fk = Some(p);
                }
            }
        }
    }
    guess_keys(&mut schema);
    for t in 0..schema.tables.len() {
        let links: Vec<(usize, usize)> = schema.tables[t].columns.iter().filter_map(|c| c.fk.map(|p| (c.c, p))).collect();
        for (c, p) in links {
            schema.tables[t].out.push((c, p));
            if p != t {
                schema.tables[p].referenced_by.push((t, c));
            }
        }
    }
    roles(&mut schema);
    subjects(&mut schema);
    schema
}

fn guess_keys(schema: &mut Schema) {
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for tb in &schema.tables {
        by_name.entry(norm(&tb.name)).or_insert(tb.t);
        by_name.entry(norm(&singular(&tb.name))).or_insert(tb.t);
    }
    let mut key_of: HashMap<String, usize> = HashMap::new();
    for tb in &schema.tables {
        if tb.pk.len() == 1 {
            key_of.insert(norm(&tb.columns[tb.pk[0]].name), tb.t);
        }
    }
    let ends = regex::Regex::new(r"^(.*?)(id|key|code|no)$").unwrap();
    let mut set = Vec::new();
    for tb in &schema.tables {
        for col in &tb.columns {
            if col.fk.is_some() || (col.pk && tb.pk.len() == 1) || !["int", "text", "uuid"].contains(&col.fam) {
                continue;
            }
            let n = norm(&col.name);
            let mut target = None;
            let via_key = key_of.get(&n).copied().filter(|&k| {
                k != tb.t && norm(&schema.tables[k].columns[schema.tables[k].pk[0]].name) != "id"
            });
            if let Some(k) = via_key {
                target = Some(k);
            } else if let Some(m) = ends.captures(&n) {
                if !m[1].is_empty() {
                    target = by_name.get(&m[1]).copied();
                }
            }
            let Some(target) = target else { continue };
            if target == tb.t {
                continue;
            }
            let p = &schema.tables[target];
            if p.pk.len() != 1 {
                continue;
            }
            let pf = p.columns[p.pk[0]].fam;
            let same = pf == col.fam || (pf == "int" && col.fam == "int") || (["text", "uuid"].contains(&pf) && ["text", "uuid"].contains(&col.fam));
            if same {
                set.push((tb.t, col.c, target));
            }
        }
    }
    for (t, c, p) in set {
        let col = &mut schema.tables[t].columns[c];
        col.fk = Some(p);
        col.fk_guessed = true;
    }
}

fn prefix(tk: &[String], a: &str, b: &str) -> bool {
    tk.len() >= 2 && tk[0] == a && tk[1] == b
}

fn roles(schema: &mut Schema) {
    let small_int_re = regex::Regex::new(r"(?i)^((tiny|small)int|int2|bit)").unwrap();
    for t in 0..schema.tables.len() {
        let rows = schema.tables[t].rows;
        let npk = schema.tables[t].pk.len();
        for ci in 0..schema.tables[t].columns.len() {
            let col = &mut schema.tables[t].columns[ci];
            let tk = col.tk.clone();
            let number = ["int", "decimal", "float"].contains(&col.fam);
            let of = prefix(&tk, "no", "of") || prefix(&tk, "num", "of") || prefix(&tk, "number", "of");
            let counted = number && (has(&tk, QUANTITY) || has(&tk, MONEY) || of);
            let mut keyish = (has(&tk, KEYISH) && !counted) || col.pk || col.fk.is_some() || norm(&col.name).ends_with("id");
            let composite = col.pk && npk > 1;
            let code = col.fam == "text" && col.fk.is_none() && !(col.pk && npk == 1) && (composite || col.distinct.is_some_and(|d| d <= 60.0));
            if code && has(&tk, &["id", "code"]) {
                keyish = false;
            }
            col.key = keyish;
            // times
            if col.fam == "timestamp" || col.fam == "date" {
                col.time = Some(if has(&tk, UPDATED) {
                    "updated"
                } else if has(&tk, DELETED) {
                    "deleted"
                } else if has(&tk, BIRTH) {
                    "birth"
                } else if has(&tk, CREATED) {
                    "created"
                } else {
                    "event"
                });
            } else if col.fam == "int"
                // Python's `not col.fk`: no key, or a key to the first table
                && col.fk.is_none_or(|f| f == 0)
                && (has(&tk, YEAR) || tk.iter().any(|x| x.ends_with("year")) || YEARMONTH.contains(&norm(&col.name).as_str()) || (has(&tk, &["date"]) && !has(&tk, UPDATED)))
            {
                if !has(&tk, BIRTH) && !has(&tk, &["min", "max"]) {
                    col.time = Some("int");
                    col.key = false;
                }
            }
            // measures, on numbers that are no keys
            if number && !keyish && col.time.is_none() {
                col.measure = Some(if has(&tk, RATIO) {
                    "ratio"
                } else if has(&tk, &["qty", "quantity", "units", "sold", "pieces", "pcs"]) || of {
                    "quantity"
                } else if has(&tk, MONEY) || (col.fam == "decimal" && matches!(scale(&col.type_name), Some(2) | Some(4))) {
                    "money"
                } else if has(&tk, SCORE) {
                    "score"
                } else if has(&tk, QUANTITY) {
                    "quantity"
                } else {
                    "number"
                });
            }
            // categories
            let len = length(&col.type_name).filter(|&l| l != 0).unwrap_or(255);
            let short = col.fam == "text" && len <= 255
                && !(col.distinct.is_none() && has(&tk, &["description", "comment", "comments", "note", "notes", "body", "content", "message", "text", "summary", "bio", "about", "address"]));
            let few = col.distinct.is_some_and(|d| 1.0 < d && d <= 60.0 && rows.is_none_or(|r| d <= f64::max(12.0, 0.2 * r)));
            let small_int = col.fam == "int" && small_int_re.is_match(&col.type_name);
            let flaglike = col.fam == "int"
                && (tk.first().is_some_and(|x| x == "is" || x == "has" || x == "can")
                    || has(&tk, &["enabled", "active", "deleted", "flag", "visible", "published", "approved", "verified", "blocked", "banned", "hidden", "archived", "paid", "done", "completed"]));
            if col.fam == "bool" || flaglike {
                col.category = Some("bool");
            } else if col.fam == "other" && has(&tk, CATEGORY) {
                col.category = Some("named");
            } else if small_int && !keyish && col.time.is_none() && matches!(col.measure, Some("number") | None) {
                col.category = Some(if has(&tk, CATEGORY) { "named" } else { "typed" });
            } else if (short || (col.fam == "int" && matches!(col.measure, None | Some("number") | Some("score")))) && !keyish && col.time.is_none() {
                if has(&tk, GEO) {
                    col.category = Some("geo");
                } else if has(&tk, CATEGORY) {
                    col.category = Some("named");
                } else if few && (col.fam == "text" || col.measure.is_none()) {
                    col.category = Some("measured");
                } else if short && col.distinct.is_none() && !has(&tk, LABEL) && !has(&tk, PERSON_PART) {
                    col.category = Some("typed");
                }
            }
            if col.category.is_some() && col.distinct.is_some_and(|d| d <= 1.0 || d > 200.0) {
                col.category = None;
            }
            if col.fam == "int" && col.category == Some("typed") && !small_int {
                col.category = None;
            }
            // labels
            if col.fam == "text" && !col.pk && len <= 255 {
                let n = norm(&col.name);
                if (has(&tk, PERSON_PART) && has(&tk, &["name"])) || ["firstname", "lastname", "namefirst", "namelast"].contains(&n.as_str()) {
                    col.label = Some(if has(&tk, &["first", "given", "fore"]) {
                        "first"
                    } else if has(&tk, &["last", "family", "sur"]) {
                        "last"
                    } else {
                        "person"
                    });
                } else if (has(&tk, LABEL) || tk.iter().any(|x| x.ends_with("name") && x.chars().count() <= 8)) && !has(&tk, &["email", "mail", "url"]) {
                    col.label = Some("name");
                }
            }
        }
        let tb = &mut schema.tables[t];
        let firsts: Vec<usize> = tb.columns.iter().filter(|c| c.label == Some("first")).map(|c| c.c).collect();
        let lasts: Vec<usize> = tb.columns.iter().filter(|c| c.label == Some("last")).map(|c| c.c).collect();
        let mut names: Vec<&Column> = tb.columns.iter().filter(|c| c.label == Some("name")).collect();
        names.sort_by_key(|c| {
            let rank = if c.tk.last().is_some_and(|x| x == "name") && c.tk.len() <= 2 {
                0
            } else if c.tk.iter().any(|x| x == "name") {
                1
            } else if c.tk.iter().any(|x| x == "title") {
                2
            } else {
                3
            };
            (rank, c.c)
        });
        let first_name = names.first().map(|c| c.c);
        tb.person = has(&tb.tk, PERSON_TABLE) || (!firsts.is_empty() && !lasts.is_empty());
        if let Some(c) = first_name {
            tb.label = Some(c);
        } else {
            let texts: Vec<usize> = tb.columns.iter().filter(|c| c.fam == "text" && !c.pk && c.fk.is_none() && !c.key).map(|c| c.c).collect();
            if texts.len() == 1 && tb.columns.iter().filter(|c| !c.pk).count() <= 3 {
                tb.label = Some(texts[0]);
            }
        }
        if !firsts.is_empty() && !lasts.is_empty() {
            tb.first_last = Some((firsts[0], lasts[0]));
            if tb.label.is_none() || tb.person {
                tb.label = Some(lasts[0]);
            }
        }
    }
}

fn subjects(schema: &mut Schema) {
    for t in 0..schema.tables.len() {
        let tb = &schema.tables[t];
        let out: Vec<usize> = tb.out.iter().map(|x| x.1).filter(|&p| p != tb.t).collect();
        let measures = tb.columns.iter().filter(|c| matches!(c.measure, Some("money") | Some("quantity"))).count();
        let timed = tb.columns.iter().any(|c| matches!(c.time, Some("created") | Some("event") | Some("int")));
        let non_key = tb.columns.iter().filter(|c| !(c.pk || c.fk.is_some())).count();
        let any_measure = tb.columns.iter().any(|c| c.measure.is_some());
        let role = if out.len() == 2 && non_key <= 1 && !any_measure {
            "link"
        } else if has(&tb.tk, SNAPSHOT) {
            "snapshot"
        } else if out.len() >= 2 || (!out.is_empty() && (timed || measures > 0)) {
            if measures > 0 || out.len() >= 2 { "fact" } else { "event" }
        } else if timed {
            "event"
        } else if !tb.referenced_by.is_empty() && non_key <= 3 && measures == 0 {
            "lookup"
        } else {
            "entity"
        };
        let tb = &mut schema.tables[t];
        tb.timed = timed;
        tb.role = role;
    }
    let mut hanging: HashMap<usize, Vec<usize>> = HashMap::new();
    for t in 0..schema.tables.len() {
        let tb = &schema.tables[t];
        if tb.timed || !(tb.role == "fact" || tb.role == "snapshot") {
            continue;
        }
        let parents: Vec<usize> = tb.out.iter().map(|x| x.1).filter(|&p| p != tb.t && schema.tables[p].timed).collect();
        if let Some(&p) = parents.first() {
            hanging.entry(p).or_default().push(t);
            schema.tables[t].detail_of = Some(p);
        }
    }
    let activity = |tb: &Table| -> f64 {
        let hang = hanging.get(&tb.t).cloned().unwrap_or_default();
        let mut rows = vec![tb.rows];
        rows.extend(hang.iter().map(|&h| schema.tables[h].rows));
        if rows.iter().all(Option::is_some) {
            rows.iter().map(|r| r.unwrap().ln_1p()).sum()
        } else {
            1.0 + hang.len() as f64 + 0.5 * tb.out.len() as f64 + 0.5 * tb.referenced_by.len() as f64
        }
    };
    let mut cands: Vec<(f64, usize)> = schema
        .tables
        .iter()
        .filter(|tb| tb.timed && matches!(tb.role, "fact" | "event" | "snapshot") && !tb.boilerplate)
        .map(|tb| (activity(tb), tb.t))
        .collect();
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (rank, (_, t)) in cands.into_iter().enumerate() {
        schema.tables[t].subject = Some(rank);
    }
}
