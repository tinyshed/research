//! The one generator: every candidate a template allows from tier S's roles,
//! the same for every configuration, with its SQL for both engines.

use crate::names::{hash, norm};
use crate::roles::{Fk, Role, Schema, TableRole, is_base_table};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Template {
    T1,
    T2,
    T3,
    T4,
    T5,
    T6,
    T7,
    T8,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Aggregate {
    Count,
    Sum,
    Avg,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ColumnRef {
    pub table: String,
    pub column: String,
    /// Whether the column is reached through the candidate's join.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub joined: bool,
    #[serde(skip)]
    pub t: usize,
    #[serde(skip)]
    pub c: usize,
}

impl ColumnRef {
    fn label(&self) -> String {
        format!("{}.{}", self.table, self.column)
    }

    fn canonical(&self) -> String {
        format!("{}.{}{}", norm(&self.table), norm(&self.column), if self.joined { "^" } else { "" })
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Join {
    pub from: ColumnRef,
    pub to: ColumnRef,
    pub guessed: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Sql {
    /// `{bucket(<expression>)}` stands for the time bucket, chosen when the
    /// candidate is drawn.
    pub postgres: Option<String>,
    pub mysql: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Candidate {
    pub id: String,
    pub template: Template,
    /// The table whose rows are counted or summed.
    pub table: String,
    pub aggregate: Aggregate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join: Option<Join>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<ColumnRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<ColumnRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measure: Option<ColumnRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ColumnRef>,
    pub chart: &'static str,
    pub title: String,
    pub guessed: bool,
    pub sql: Sql,
    /// The counted table's index; none for T8, which is the database's.
    #[serde(skip)]
    pub t: Option<usize>,
    /// O's order: the table as the catalog lists it, the first column used
    /// in its declared position, the template, the order of generation.
    #[serde(skip)]
    pub order: (usize, i64, Template, usize),
}

impl Candidate {
    /// The column a candidate is about, for the top five's rule against two
    /// of one template on one column.
    pub fn main_column(&self) -> Option<&ColumnRef> {
        match self.template {
            Template::T1 | Template::T6 | Template::T7 => self.time.as_ref(),
            Template::T2 => self.category.as_ref(),
            Template::T3 | Template::T4 => self.measure.as_ref(),
            Template::T5 => self.measure.as_ref().or(self.join.as_ref().map(|j| &j.from)),
            Template::T8 => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Engine {
    Postgres,
    Mysql,
}

fn ident(e: Engine, name: &str) -> String {
    match e {
        Engine::Postgres => format!("\"{}\"", name.replace('"', "\"\"")),
        Engine::Mysql => format!("`{}`", name.replace('`', "``")),
    }
}

struct Ctx<'a> {
    schema: &'a Schema<'a>,
}

impl Ctx<'_> {
    fn col(&self, t: usize, c: usize, joined: bool) -> ColumnRef {
        let table = &self.schema.snapshot.tables[t];
        ColumnRef { table: table.name.clone(), column: table.columns[c].name.clone(), joined, t, c }
    }

    fn table(&self, e: Engine, t: usize) -> String {
        let table = &self.schema.snapshot.tables[t];
        match e {
            Engine::Postgres => format!("{}.{}", ident(e, &table.schema), ident(e, &table.name)),
            Engine::Mysql => ident(e, &table.name),
        }
    }

    fn expr(&self, e: Engine, r: &ColumnRef) -> String {
        format!("{}.{}", if r.joined { "p" } else { "t" }, ident(e, &r.column))
    }

    fn from(&self, e: Engine, t: usize, join: Option<&Join>) -> String {
        let mut from = format!("{} t", self.table(e, t));
        if let Some(j) = join {
            from += &format!(
                " LEFT JOIN {} p ON p.{} = t.{}",
                self.table(e, j.to.t),
                ident(e, &j.to.column),
                ident(e, &j.from.column)
            );
        }
        from
    }

    fn join(&self, fk: &Fk) -> Join {
        Join {
            from: self.col(fk.child, fk.child_column, false),
            to: self.col(fk.parent, fk.parent_column, true),
            guessed: fk.guessed,
        }
    }
}

/// What a candidate is made of before its SQL and identity.
struct Shape {
    template: Template,
    t: usize,
    aggregate: Aggregate,
    join: Option<Join>,
    time: Option<ColumnRef>,
    category: Option<ColumnRef>,
    measure: Option<ColumnRef>,
    columns: Vec<ColumnRef>,
}

fn sql(ctx: &Ctx, e: Engine, s: &Shape) -> String {
    let from = ctx.from(e, s.t, s.join.as_ref());
    let x = s.time.as_ref().map(|r| ctx.expr(e, r)).unwrap_or_default();
    let g = s.category.as_ref().map(|r| ctx.expr(e, r)).unwrap_or_default();
    let m = s.measure.as_ref().map(|r| ctx.expr(e, r)).unwrap_or_default();
    let agg = match s.aggregate {
        Aggregate::Count => "count",
        Aggregate::Sum => "sum",
        Aggregate::Avg => "avg",
    };
    match s.template {
        Template::T1 => format!("SELECT {{bucket({x})}} AS bucket, count(*) AS n FROM {from} GROUP BY 1 ORDER BY 1"),
        Template::T2 => format!("SELECT {g} AS category, count(*) AS n FROM {from} GROUP BY 1 ORDER BY 2 DESC"),
        Template::T3 => format!("SELECT {{bucket({x})}} AS bucket, {agg}({m}) AS value FROM {from} GROUP BY 1 ORDER BY 1"),
        Template::T4 => format!("SELECT {g} AS category, {agg}({m}) AS value FROM {from} GROUP BY 1 ORDER BY 2 DESC"),
        Template::T5 => match (&s.measure, &s.join) {
            (Some(r), _) => {
                let table = ctx.table(e, s.t);
                let bare = ident(e, &r.column);
                format!(
                    "SELECT floor(({m} - r.lo) * 20 / nullif(r.hi - r.lo, 0)) AS bin, min({m}) AS low, count(*) AS n \
                     FROM {table} t CROSS JOIN (SELECT min({bare}) AS lo, max({bare}) AS hi FROM {table}) r \
                     WHERE {m} IS NOT NULL GROUP BY 1 ORDER BY 1"
                )
            }
            (None, Some(j)) => {
                let table = ctx.table(e, s.t);
                let fk = format!("t.{}", ident(e, &j.from.column));
                format!(
                    "SELECT c.n AS children, count(*) AS parents FROM \
                     (SELECT {fk} AS parent, count(*) AS n FROM {table} t WHERE {fk} IS NOT NULL GROUP BY 1) c \
                     GROUP BY 1 ORDER BY 1"
                )
            }
            _ => unreachable!("a distribution has a measure or a join"),
        },
        Template::T6 => format!(
            "SELECT {{bucket({x})}} AS bucket, {g} AS category, count(*) AS n FROM {from} GROUP BY 1, 2 ORDER BY 1, 2"
        ),
        Template::T7 => {
            let cols: Vec<String> = s.columns.iter().map(|r| ctx.expr(e, r)).collect();
            format!("SELECT {} FROM {from} ORDER BY {x} DESC LIMIT 10", cols.join(", "))
        }
        Template::T8 => unreachable!("T8 is read from the snapshot"),
    }
}

fn title(ctx: &Ctx, s: &Shape) -> String {
    let table = &ctx.schema.snapshot.tables[s.t].name;
    let agg = match s.aggregate {
        Aggregate::Sum => "sum",
        Aggregate::Avg => "average",
        Aggregate::Count => "count",
    };
    let label = |r: &Option<ColumnRef>| r.as_ref().map(ColumnRef::label).unwrap_or_default();
    match s.template {
        Template::T1 => format!("rows of {table} by {}", label(&s.time)),
        Template::T2 => format!("rows of {table} by {}", label(&s.category)),
        Template::T3 => format!("{agg} of {} by {}", label(&s.measure), label(&s.time)),
        Template::T4 => format!("{agg} of {} by {}", label(&s.measure), label(&s.category)),
        Template::T5 => match &s.join {
            Some(j) if s.measure.is_none() => format!("rows of {table} per {}", j.to.table),
            _ => format!("distribution of {}", label(&s.measure)),
        },
        Template::T6 => format!("rows of {table} by {} and {}", label(&s.time), label(&s.category)),
        Template::T7 => format!("latest rows of {table}"),
        Template::T8 => "largest tables, estimated rows".into(),
    }
}

fn identity(s: &Shape, table: &str) -> String {
    let canon = |r: &Option<ColumnRef>| r.as_ref().map(ColumnRef::canonical).unwrap_or_default();
    let join = s.join.as_ref().map(|j| format!("{}>{}", j.from.canonical(), j.to.canonical())).unwrap_or_default();
    let columns: Vec<String> = s.columns.iter().map(ColumnRef::canonical).collect();
    let key = format!(
        "{:?}|{}|{:?}|{}|{}|{}|{}|{}",
        s.template,
        norm(table),
        s.aggregate,
        join,
        canon(&s.time),
        canon(&s.category),
        canon(&s.measure),
        columns.join(",")
    );
    hash(key.as_bytes())
}

fn build(ctx: &Ctx, s: Shape, sequence: usize) -> Candidate {
    let table = ctx.schema.snapshot.tables[s.t].name.clone();
    let chart = match s.template {
        Template::T1 | Template::T3 => "line",
        Template::T6 => "stacked bar",
        Template::T7 => "table",
        _ => "bar",
    };
    let first = [&s.time, &s.category, &s.measure]
        .into_iter()
        .flatten()
        .chain(s.join.as_ref().map(|j| &j.from))
        .filter(|r| !r.joined)
        .map(|r| ctx.schema.snapshot.tables[r.t].columns[r.c].position)
        .min()
        .unwrap_or(i64::MAX);
    Candidate {
        id: identity(&s, &table),
        template: s.template,
        table,
        aggregate: s.aggregate,
        sql: Sql { postgres: Some(sql(ctx, Engine::Postgres, &s)), mysql: Some(sql(ctx, Engine::Mysql, &s)) },
        title: title(ctx, &s),
        chart,
        guessed: true,
        t: Some(s.t),
        order: (s.t, first, s.template, sequence),
        join: s.join,
        time: s.time,
        category: s.category,
        measure: s.measure,
        columns: s.columns,
    }
}

fn aggregates(role: Role) -> &'static [Aggregate] {
    match role {
        Role::Money | Role::Quantity => &[Aggregate::Sum, Aggregate::Avg],
        Role::Score | Role::Ratio => &[Aggregate::Avg],
        _ => &[],
    }
}

/// Every candidate the templates allow from the roles of tier S.
pub fn generate(schema: &Schema) -> Vec<Candidate> {
    let ctx = Ctx { schema };
    let snapshot = schema.snapshot;
    let mut shapes: Vec<Shape> = Vec::new();

    for (t, table) in snapshot.tables.iter().enumerate() {
        if !is_base_table(table) {
            continue;
        }
        let roles = &schema.tables[t];
        let out: Vec<&Fk> = schema
            .fks
            .iter()
            .filter(|f| f.child == t && is_base_table(&snapshot.tables[f.parent]))
            .collect();

        // Times and groups of the table's own, and of each parent through
        // one many-to-one join: (the join, the column).
        let mut times: Vec<(Option<&Fk>, ColumnRef)> = Vec::new();
        let mut groups: Vec<(Option<&Fk>, ColumnRef)> = Vec::new();
        for (c, r) in roles.columns.iter().enumerate() {
            if r.axis().is_some() {
                times.push((None, ctx.col(t, c, false)));
            }
            if r.primary() == Some(Role::Category) {
                groups.push((None, ctx.col(t, c, false)));
            }
        }
        for fk in &out {
            let parent = &schema.tables[fk.parent];
            for (c, r) in parent.columns.iter().enumerate() {
                if r.axis().is_some() {
                    times.push((Some(fk), ctx.col(fk.parent, c, true)));
                }
                if r.primary() == Some(Role::Category) || parent.label_index == Some(c) {
                    groups.push((Some(fk), ctx.col(fk.parent, c, true)));
                }
            }
        }
        let measures: Vec<(ColumnRef, Role)> = roles
            .columns
            .iter()
            .enumerate()
            .filter_map(|(c, r)| {
                let p = r.primary()?;
                matches!(p, Role::Money | Role::Quantity | Role::Score | Role::Ratio).then(|| (ctx.col(t, c, false), p))
            })
            .collect();

        let join_of = |fk: Option<&Fk>| fk.map(|f| ctx.join(f));
        let shape = |template, aggregate, join, time, category, measure| Shape {
            template,
            t,
            aggregate,
            join,
            time,
            category,
            measure,
            columns: Vec::new(),
        };

        for (fk, time) in &times {
            shapes.push(shape(Template::T1, Aggregate::Count, join_of(*fk), Some(time.clone()), None, None));
        }
        for (fk, group) in &groups {
            shapes.push(shape(Template::T2, Aggregate::Count, join_of(*fk), None, Some(group.clone()), None));
        }
        for (m, role) in &measures {
            for agg in aggregates(*role) {
                for (fk, time) in &times {
                    shapes.push(shape(Template::T3, *agg, join_of(*fk), Some(time.clone()), None, Some(m.clone())));
                }
                for (fk, group) in &groups {
                    shapes.push(shape(Template::T4, *agg, join_of(*fk), None, Some(group.clone()), Some(m.clone())));
                }
            }
            if matches!(role, Role::Money | Role::Quantity | Role::Score) {
                shapes.push(shape(Template::T5, Aggregate::Count, None, None, None, Some(m.clone())));
            }
        }
        for fk in &out {
            shapes.push(shape(Template::T5, Aggregate::Count, join_of(Some(fk)), None, None, None));
        }
        for (tf, time) in &times {
            for (gf, group) in &groups {
                // At most one join: both from the table, or one parent's.
                let fk = match (tf, gf) {
                    (None, None) => None,
                    (Some(f), None) | (None, Some(f)) => Some(*f),
                    (Some(a), Some(b)) if std::ptr::eq(*a, *b) => Some(*a),
                    _ => continue,
                };
                shapes.push(shape(Template::T6, Aggregate::Count, join_of(fk), Some(time.clone()), Some(group.clone()), None));
            }
        }
        if roles.has(TableRole::Event) {
            if let Some((_, time)) = times.iter().find(|(fk, _)| fk.is_none()) {
                let shown = |c: usize| {
                    let r = &roles.columns[c];
                    !(r.has(Role::Text) || r.has(Role::Identifier) || r.has(Role::Pk) || r.has(Role::Fk) || r.has(Role::FkGuessed))
                };
                let mut columns = vec![time.clone()];
                columns.extend(
                    (0..table.columns.len()).filter(|&c| c != time.c && shown(c)).take(4).map(|c| ctx.col(t, c, false)),
                );
                let mut s = shape(Template::T7, Aggregate::Count, None, Some(time.clone()), None, None);
                s.columns = columns;
                shapes.push(s);
            }
        }
    }

    let mut candidates: Vec<Candidate> =
        shapes.into_iter().enumerate().map(|(i, s)| build(&ctx, s, i)).collect();
    candidates.push(Candidate {
        id: hash(b"T8|largest tables"),
        template: Template::T8,
        table: snapshot.database.clone(),
        aggregate: Aggregate::Count,
        join: None,
        time: None,
        category: None,
        measure: None,
        columns: Vec::new(),
        chart: "bar",
        title: "largest tables, estimated rows".into(),
        guessed: true,
        sql: Sql::default(),
        t: None,
        order: (usize::MAX, i64::MAX, Template::T8, usize::MAX),
    });
    candidates
}
