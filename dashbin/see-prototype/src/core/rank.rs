//! What orders the questions, as round 1c's `rank.py`: the features a question
//! shows, the statistics' vetoes and points, the words by where they stand,
//! and the choice of five that do not repeat each other.

use std::collections::{HashMap, HashSet};

use super::names::{has, singular};
use super::question::{By, Identity, Question, identity};
use super::schema::Schema;

const ACTIVITY: &[&str] = &["order", "orders", "sale", "sales", "invoice", "invoices", "payment", "payments", "transaction", "transactions",
    "booking", "bookings", "reservation", "reservations", "event", "events", "log", "logs", "visit", "visits",
    "ticket", "tickets", "purchase", "purchases", "shipment", "shipments", "subscription", "subscriptions",
    "rental", "rentals", "match", "matches", "game", "games", "result", "results", "entry", "entries", "post",
    "posts", "comment", "comments", "review", "reviews", "message", "messages", "session", "sessions", "loan",
    "loans", "trip", "trips", "flight", "flights", "race", "races", "appointment", "appointments", "enrollment",
    "enrollments", "application", "applications", "submission", "submissions", "attempt", "attempts", "vote",
    "votes", "click", "clicks", "view", "views", "donation", "donations", "expense", "expenses", "deposit",
    "withdrawal", "transfer", "transfers", "line", "lines", "item", "items", "detail", "details", "batting",
    "pitching", "fielding", "salaries", "salary", "rating", "ratings", "answer", "answers", "question",
    "questions", "task", "tasks", "issue", "issues", "call", "calls", "delivery", "deliveries", "receipt"];
const ENTITY: &[&str] = &["user", "users", "customer", "customers", "product", "products", "employee", "employees", "player", "players",
    "team", "teams", "member", "members", "account", "accounts", "client", "clients", "student", "students",
    "course", "courses", "book", "books", "movie", "movies", "store", "stores", "company", "companies", "supplier",
    "suppliers", "vendor", "vendors", "author", "authors", "driver", "drivers", "patient", "patients", "doctor"];
const LOOKUPISH: &[&str] = &["type", "types", "status", "statuses", "category", "categories", "country", "countries", "role", "roles",
    "region", "regions", "language", "languages", "currency", "currencies", "genre", "genres", "state", "states"];
pub const MONEYWORD: &[&str] = &["total", "amount", "revenue", "price", "sales", "income", "profit", "cost", "salary", "paid", "payment", "fare"];
const FAMOUS_TIME: &[&str] = &["created", "order", "ordered", "date", "booked", "book", "invoice", "sale", "paid", "placed", "posted", "submitted", "start", "started"];
const ROLES: [&str; 7] = ["fact", "event", "entity", "lookup", "link", "snapshot", "table"];
const MKINDS: [&str; 8] = ["rows", "column", "product", "net", "parents", "gap", "late", "per"];
const MROLES: [&str; 5] = ["money", "quantity", "ratio", "score", "number"];
const BYKINDS: [&str; 9] = ["label", "person", "key", "geo", "bool", "named", "measured", "typed", "own_label"];
const SHAPES: [&str; 5] = ["series", "breakdown", "top", "split", "distribution"];

fn log(x: Option<f64>) -> f64 {
    x.map(f64::ln_1p).unwrap_or(f64::NAN)
}

fn ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<f64> = values.to_vec();
    order.sort_by(|a, b| b.total_cmp(a));
    order.dedup();
    values.iter().map(|v| order.iter().position(|o| o == v).unwrap() as f64).collect()
}

/// What a table is beside the others.
pub struct Context {
    in_rank: Vec<f64>,
    out_rank: Vec<f64>,
    ncols_rank: Vec<f64>,
    meas_rank: Vec<f64>,
    money_rank: Vec<f64>,
    time_rank: Vec<f64>,
    in_share: Vec<f64>,
    out_share: Vec<f64>,
    ncols_share: Vec<f64>,
    meas_share: Vec<f64>,
    timed_tables: f64,
    money_tables: f64,
    measure_order: HashMap<(usize, usize), f64>,
    category_order: HashMap<(usize, usize), f64>,
}

fn share(v: &[f64]) -> Vec<f64> {
    let m = v.iter().copied().fold(f64::MIN, f64::max).max(1.0);
    v.iter().map(|x| x / m).collect()
}

pub fn context(schema: &Schema) -> Context {
    let t = &schema.tables;
    let ins: Vec<f64> = t.iter().map(|x| x.referenced_by.len() as f64).collect();
    let outs: Vec<f64> = t.iter().map(|x| x.out.len() as f64).collect();
    let ncols: Vec<f64> = t.iter().map(|x| x.columns.len() as f64).collect();
    let meas: Vec<f64> = t.iter().map(|x| x.columns.iter().filter(|c| matches!(c.measure, Some("money") | Some("quantity") | Some("number"))).count() as f64).collect();
    let money: Vec<f64> = t.iter().map(|x| x.columns.iter().filter(|c| c.measure == Some("money")).count() as f64).collect();
    let times: Vec<f64> = t.iter().map(|x| x.columns.iter().filter(|c| matches!(c.time, Some("created") | Some("event") | Some("int"))).count() as f64).collect();
    let mut measure_order = HashMap::new();
    let mut category_order = HashMap::new();
    for tb in t {
        let mut ms: Vec<usize> = tb.columns.iter().filter(|c| matches!(c.measure, Some("money") | Some("quantity") | Some("number") | Some("ratio") | Some("score"))).map(|c| c.c).collect();
        ms.sort_by_key(|&c| (match tb.columns[c].measure { Some("money") => 0, Some("quantity") => 1, _ => 2 }, c));
        for (i, c) in ms.iter().enumerate() {
            measure_order.insert((tb.t, *c), i as f64);
        }
        for (i, c) in tb.columns.iter().filter(|c| c.category.is_some()).enumerate() {
            category_order.insert((tb.t, c.c), i as f64);
        }
    }
    Context {
        in_rank: ranks(&ins),
        out_rank: ranks(&outs),
        ncols_rank: ranks(&ncols),
        meas_rank: ranks(&meas),
        money_rank: ranks(&money),
        time_rank: ranks(&times),
        in_share: share(&ins),
        out_share: share(&outs),
        ncols_share: share(&ncols),
        meas_share: share(&meas),
        timed_tables: t.iter().filter(|x| x.timed).count() as f64,
        money_tables: money.iter().filter(|&&m| m > 0.0).count() as f64,
        measure_order,
        category_order,
    }
}

pub fn features(schema: &Schema, ctx: &Context, q: &Question) -> HashMap<&'static str, f64> {
    let tb = &schema.tables[q.base];
    let mut f: HashMap<&'static str, f64> = HashMap::new();
    let b = |x: bool| if x { 1.0 } else { 0.0 };
    let nan = f64::NAN;
    f.insert("base_subject", b(tb.subject.is_some()));
    f.insert("base_subject_rank", tb.subject.map(|s| s as f64).unwrap_or(9.0));
    f.insert("base_first_subject", b(tb.subject == Some(0)));
    f.insert("base_detail", b(tb.detail_of.is_some()));
    f.insert("base_detail_of_first", b(tb.detail_of.is_some_and(|d| schema.tables[d].subject == Some(0))));
    for r in ROLES {
        f.insert(role_key(r), b(tb.role == r));
    }
    f.insert("base_timed", b(tb.timed));
    f.insert("base_out", tb.out.len() as f64);
    f.insert("base_in", tb.referenced_by.len() as f64);
    f.insert("base_ncols", tb.columns.len() as f64);
    f.insert("base_boilerplate", b(tb.boilerplate));
    f.insert("base_activity_word", b(has(&tb.tk, ACTIVITY)));
    f.insert("base_entity_word", b(has(&tb.tk, ENTITY)));
    f.insert("base_lookup_word", b(has(&tb.tk, LOOKUPISH)));
    f.insert("base_person", b(tb.person));
    f.insert("base_money_cols", tb.columns.iter().filter(|c| c.measure == Some("money")).count() as f64);
    f.insert("base_qty_cols", tb.columns.iter().filter(|c| c.measure == Some("quantity")).count() as f64);
    let i = q.base;
    f.insert("base_in_rank", ctx.in_rank[i]);
    f.insert("base_out_rank", ctx.out_rank[i]);
    f.insert("base_ncols_rank", ctx.ncols_rank[i]);
    f.insert("base_meas_rank", ctx.meas_rank[i]);
    f.insert("base_money_rank", ctx.money_rank[i]);
    f.insert("base_time_rank", ctx.time_rank[i]);
    f.insert("base_in_share", ctx.in_share[i]);
    f.insert("base_out_share", ctx.out_share[i]);
    f.insert("base_ncols_share", ctx.ncols_share[i]);
    f.insert("base_meas_share", ctx.meas_share[i]);
    f.insert("schema_timed_tables", ctx.timed_tables);
    f.insert("schema_money_tables", ctx.money_tables);
    f.insert("schema_tables", schema.tables.len() as f64);
    f.insert("schema_subjects", schema.tables.iter().filter(|t| t.subject.is_some()).count() as f64);
    f.insert("base_log_rows", log(tb.rows));
    let known: Vec<f64> = schema.tables.iter().filter_map(|t| t.rows).collect();
    let mx = known.iter().copied().fold(f64::MIN, f64::max);
    f.insert("base_rows_share", match tb.rows {
        Some(r) if !known.is_empty() && mx > 0.0 => r / mx,
        _ => nan,
    });
    let m = &q.measure;
    for k in MKINDS {
        f.insert(mkind_key(k), b(m.kind == k));
    }
    f.insert("m_product", b(m.kind == "product" || m.kind == "net"));
    f.insert("m_sum", b(m.agg == "sum"));
    f.insert("m_avg", b(m.agg == "avg"));
    let col = m.cols.first().map(|&c| schema.col(c));
    for r in MROLES {
        f.insert(mrole_key(r), b(col.is_some_and(|c| m.kind == "column" && c.measure == Some(r))));
    }
    f.insert("m_moneyword", b(col.is_some_and(|c| has(&c.tk, MONEYWORD))));
    f.insert("m_named_total", b(col.is_some_and(|c| has(&c.tk, &["total", "amount", "revenue"]))));
    f.insert("m_null_frac", col.and_then(|c| c.null_frac).unwrap_or(nan));
    f.insert("m_short_name", b(col.is_some_and(|c| c.name.chars().count() <= 3)));
    f.insert("m_order", match m.cols.first() {
        Some(c) => *ctx.measure_order.get(c).unwrap_or(&9.0),
        None => -1.0,
    });
    f.insert("m_position", match m.cols.first() {
        Some(&(t, c)) => c as f64 / schema.tables[t].columns.len().max(1) as f64,
        None => -1.0,
    });
    f.insert("has_time", b(q.time.is_some()));
    if let Some(d) = &q.time {
        let tc = &schema.tables[d.t].columns[d.c as usize];
        f.insert("time_created", b(d.kind == "created"));
        f.insert("time_event", b(d.kind == "event"));
        f.insert("time_int", b(d.kind == "int"));
        f.insert("time_on_base", b(d.path.is_empty()));
        f.insert("time_path", d.path.len() as f64);
        f.insert("time_famous", b(has(&tc.tk, FAMOUS_TIME)));
        let axes: Vec<usize> = schema.tables[d.t].columns.iter().filter(|c| matches!(c.time, Some("created") | Some("event") | Some("int"))).map(|c| c.c).collect();
        f.insert("time_first_axis", b(axes.first() == Some(&(d.c as usize))));
        f.insert("time_axes", axes.len() as f64);
        f.insert("time_null_frac", tc.null_frac.unwrap_or(nan));
    } else {
        for k in ["time_created", "time_event", "time_int", "time_on_base", "time_path", "time_famous", "time_first_axis", "time_axes"] {
            f.insert(k, 0.0);
        }
        f.insert("time_null_frac", nan);
    }
    f.insert("has_by", b(q.by.is_some()));
    for k in BYKINDS {
        f.insert(bykind_key(k), b(q.by.as_ref().is_some_and(|g| g.kind == k)));
    }
    if let Some(g) = &q.by {
        let gt = &schema.tables[g.t];
        let gc = if g.c >= 0 { Some(&gt.columns[g.c as usize]) } else { None };
        f.insert("by_path", g.path.len() as f64);
        f.insert("by_on_base", b(g.path.is_empty()));
        f.insert("by_table_lookup", b(gt.role == "lookup"));
        f.insert("by_table_entity", b(gt.role == "entity"));
        f.insert("by_table_subject", b(gt.subject.is_some()));
        f.insert("by_table_boilerplate", b(gt.boilerplate));
        f.insert("by_log_distinct", match gc {
            Some(c) => c.distinct.map(f64::ln_1p).unwrap_or(nan),
            None => log(gt.rows),
        });
        f.insert("by_null_frac", gc.and_then(|c| c.null_frac).unwrap_or(nan));
        f.insert("by_top_freq", gc.and_then(|c| c.top_freq).unwrap_or(nan));
        f.insert("by_status", b(gc.is_some_and(|c| has(&c.tk, &["status", "state", "type", "kind", "category"]))));
        f.insert("by_country", b(gc.is_some_and(|c| has(&c.tk, &["country", "nationality"]))));
        f.insert("by_order", if g.c >= 0 { *ctx.category_order.get(&(g.t, g.c as usize)).unwrap_or(&9.0) } else { -1.0 });
        f.insert("by_table_in_rank", ctx.in_rank[g.t]);
        f.insert("by_table_in_share", ctx.in_share[g.t]);
        f.insert("by_table_ncols", gt.columns.len() as f64);
    } else {
        for k in ["by_path", "by_on_base", "by_table_lookup", "by_table_entity", "by_table_subject", "by_table_boilerplate", "by_status", "by_country"] {
            f.insert(k, 0.0);
        }
        for k in ["by_order", "by_table_in_rank", "by_table_in_share", "by_table_ncols"] {
            f.insert(k, -1.0);
        }
        for k in ["by_log_distinct", "by_null_frac", "by_top_freq"] {
            f.insert(k, nan);
        }
    }
    for s in SHAPES {
        f.insert(shape_key(s), b(q.shape == s));
    }
    f
}

fn role_key(r: &str) -> &'static str {
    match r { "fact" => "base_role_fact", "event" => "base_role_event", "entity" => "base_role_entity", "lookup" => "base_role_lookup", "link" => "base_role_link", "snapshot" => "base_role_snapshot", _ => "base_role_table" }
}
fn mkind_key(k: &str) -> &'static str {
    match k { "rows" => "m_rows", "column" => "m_column", "product" => "m_product", "net" => "m_net", "parents" => "m_parents", "gap" => "m_gap", "late" => "m_late", _ => "m_per" }
}
fn mrole_key(k: &str) -> &'static str {
    match k { "money" => "m_role_money", "quantity" => "m_role_quantity", "ratio" => "m_role_ratio", "score" => "m_role_score", _ => "m_role_number" }
}
fn bykind_key(k: &str) -> &'static str {
    match k { "label" => "by_label", "person" => "by_person", "key" => "by_key", "geo" => "by_geo", "bool" => "by_bool", "named" => "by_named", "measured" => "by_measured", "typed" => "by_typed", _ => "by_own_label" }
}
fn shape_key(k: &str) -> &'static str {
    match k { "series" => "shape_series", "breakdown" => "shape_breakdown", "top" => "shape_top", "split" => "shape_split", _ => "shape_distribution" }
}

/// The words a question is made of, by where they stand.
pub fn slots(schema: &Schema, q: &Question) -> HashMap<&'static str, Vec<String>> {
    let tb = &schema.tables[q.base];
    let mut out = HashMap::new();
    let none = || vec!["#none".to_string()];
    let base: Vec<String> = tb.tk.iter().map(|t| singular(t)).collect();
    out.insert("base", if base.is_empty() { none() } else { base });
    let m = &q.measure;
    if let Some(&c0) = m.cols.first() {
        let c = schema.col(c0);
        let kind = if m.kind == "net" { "product" } else { m.kind };
        let mut v: Vec<String> = c.tk.iter().map(|t| format!("{kind}:{t}")).collect();
        v.push(format!("#{kind}:{}", m.agg));
        out.insert("measure", v);
    } else {
        out.insert("measure", vec![format!("#{}", m.kind)]);
    }
    out.insert("time", match &q.time {
        Some(d) => {
            let tk = &schema.tables[d.t].columns[d.c as usize].tk;
            if tk.is_empty() { none() } else { tk.clone() }
        }
        None => none(),
    });
    out.insert("by", match &q.by {
        Some(d) => {
            let bt = &schema.tables[d.t];
            let v: Vec<String> = if d.c >= 0 { bt.columns[d.c as usize].tk.clone() } else { bt.tk.iter().map(|t| format!("@{}", singular(t))).collect() };
            if v.is_empty() { none() } else { v }
        }
        None => none(),
    });
    out
}

pub fn veto(schema: &Schema, q: &Question) -> Option<&'static str> {
    let tb = &schema.tables[q.base];
    if tb.rows == Some(0.0) {
        return Some("an empty table");
    }
    if let Some(by) = &q.by {
        if by.c >= 0 {
            let col = &schema.tables[by.t].columns[by.c as usize];
            if let Some(d) = col.distinct {
                if d <= 1.0 {
                    return Some("one value");
                }
                if q.shape == "split" && d > 8.0 {
                    return Some("a split of too many values");
                }
                if q.shape == "breakdown" && d > 60.0 {
                    return Some("too many values");
                }
            }
            if col.null_frac.is_some_and(|x| x > 0.9) {
                return Some("mostly null");
            }
            if col.top_freq.is_some_and(|x| x > 0.97) {
                return Some("nearly one value");
            }
        }
        if by.c == -1 && q.shape == "top" && matches!(q.measure.kind, "rows" | "column" | "product" | "net") {
            let pt = &schema.tables[by.t];
            if let (Some(pr), Some(tr)) = (pt.rows, tb.rows) {
                if pr != 0.0 && pr > 10000.0 && tr / pr < 1.5 {
                    return Some("about one row each");
                }
            }
        }
        if by.c == -1 && q.shape == "split" && schema.tables[by.t].rows.is_some_and(|r| r > 8.0) {
            return Some("a split of too many values");
        }
    }
    if let Some(t) = &q.time {
        let col = &schema.tables[t.t].columns[t.c as usize];
        if col.null_frac.is_some_and(|x| x > 0.9) {
            return Some("mostly null");
        }
        if col.distinct.is_some_and(|d| d <= 1.0) {
            return Some("one value");
        }
    }
    for &tc in &q.measure.cols {
        if schema.col(tc).null_frac.is_some_and(|x| x > 0.95) {
            return Some("mostly null");
        }
    }
    None
}

pub fn stats_points(schema: &Schema, q: &Question) -> f64 {
    let tb = &schema.tables[q.base];
    let mut p = 0.0;
    if let Some(r) = tb.rows {
        if r < 20.0 {
            p -= 1.5;
        } else if r < 100.0 {
            p -= 0.6;
        }
        let known: Vec<f64> = schema.tables.iter().filter_map(|t| t.rows).filter(|&r| r != 0.0).collect();
        let mx = known.iter().copied().fold(f64::MIN, f64::max);
        if !known.is_empty() && mx > 1.0 {
            p += 0.8 * r.ln_1p() / mx.ln_1p();
        }
    }
    if let Some(by) = &q.by {
        if by.c >= 0 {
            let col = &schema.tables[by.t].columns[by.c as usize];
            if col.null_frac.is_some_and(|x| x > 0.3) {
                p -= 0.5;
            }
            if col.distinct.is_some_and(|d| (3.0..=12.0).contains(&d)) {
                p += 0.3;
            }
        }
    }
    p
}

pub const MAX_KIND: usize = 5;

fn kind_of(i: &Identity) -> (&'static str, bool, bool, bool) {
    (i.agg, i.cols.is_empty(), i.time.is_some(), i.by.is_some())
}

/// The order Python's stable sort by -score gives.
pub fn order(scores: &[f64]) -> Vec<usize> {
    let mut o: Vec<usize> = (0..scores.len()).collect();
    o.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    o
}

/// The k best that add something, as `rank.choose`.
pub fn choose(qs: &[Question], scores: &[f64], k: usize) -> Vec<usize> {
    let mut chosen = Vec::new();
    let mut per_table: HashMap<usize, usize> = HashMap::new();
    let mut per_kind: HashMap<(&str, bool, bool, bool), usize> = HashMap::new();
    let (mut times, mut bys, mut asked): (HashSet<(usize, usize)>, HashSet<By>, HashSet<Identity>) = Default::default();
    for i in order(scores) {
        let q = &qs[i];
        let ident = identity(q);
        let t = q.time.as_ref().map(|d| (d.t, d.c as usize));
        let kind = kind_of(&ident);
        if asked.contains(&ident) || *per_table.get(&q.base).unwrap_or(&0) >= 2 || *per_kind.get(&kind).unwrap_or(&0) >= MAX_KIND {
            continue;
        }
        if t.is_some_and(|t| times.contains(&t)) {
            continue;
        }
        if ident.by.as_ref().is_some_and(|b| bys.contains(b)) {
            continue;
        }
        chosen.push(i);
        *per_table.entry(q.base).or_insert(0) += 1;
        *per_kind.entry(kind).or_insert(0) += 1;
        if let Some(t) = t {
            times.insert(t);
        }
        if let Some(b) = ident.by.clone() {
            bys.insert(b);
        }
        asked.insert(ident);
        if chosen.len() == k {
            break;
        }
    }
    chosen
}
