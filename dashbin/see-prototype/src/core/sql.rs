//! A question compiled to one statement for PostgreSQL or MySQL, as round 1c's
//! `compile_sql`: joins left so no row is lost, live rows only, a parent's
//! name grouped with its key. Aliases are made in the order Python makes them,
//! so the statement is the same to the byte.

use std::collections::HashMap;

use super::question::{Dim, Question, alive, named};
use super::schema::Schema;

pub fn quote(engine: &str, name: &str) -> String {
    if engine == "mysql" { format!("`{name}`") } else { format!("\"{}\"", name.replace('"', "\"\"")) }
}

struct Ctx<'a> {
    schema: &'a Schema,
    engine: &'a str,
    aliases: HashMap<Vec<(usize, usize)>, String>,
    joins: Vec<String>,
}

impl Ctx<'_> {
    fn e(&self, n: &str) -> String {
        quote(self.engine, n)
    }

    fn alias(&mut self, path: &[(usize, usize)]) -> String {
        if let Some(a) = self.aliases.get(path) {
            return a.clone();
        }
        let parent = self.alias(&path[..path.len() - 1]);
        let (t, c) = path[path.len() - 1];
        let p = self.schema.tables[t].columns[c].fk.unwrap();
        let pt = &self.schema.tables[p];
        let a = format!("p{}", self.aliases.len());
        self.aliases.insert(path.to_vec(), a.clone());
        let key = match pt.pk.first() {
            Some(&k) => pt.columns[k].name.clone(),
            None => pt.columns[0].name.clone(),
        };
        let join = format!("LEFT JOIN {} {a} ON {a}.{} = {parent}.{}", self.e(&pt.name), self.e(&key), self.e(&self.schema.tables[t].columns[c].name));
        self.joins.push(join);
        a
    }

    fn colref(&mut self, path: &[(usize, usize)], t: usize, c: usize) -> String {
        let a = self.alias(path);
        format!("{a}.{}", self.e(&self.schema.tables[t].columns[c].name))
    }

    fn b(&mut self, tc: (usize, usize)) -> String {
        self.colref(&[], tc.0, tc.1)
    }

    fn bucket(&mut self, d: &Dim, unit: Option<&str>) -> String {
        let col = &self.schema.tables[d.t].columns[d.c as usize];
        let fam = col.fam;
        let r = self.colref(&d.path, d.t, d.c as usize);
        if d.kind == "int" || fam == "int" {
            return r;
        }
        let unit = unit.unwrap_or("month");
        if self.engine == "mysql" {
            match unit {
                "day" => format!("DATE({r})"),
                "week" => format!("DATE({r} - INTERVAL WEEKDAY({r}) DAY)"),
                "month" => format!("DATE_FORMAT({r}, '%Y-%m-01')"),
                _ => format!("DATE_FORMAT({r}, '%Y-01-01')"),
            }
        } else {
            format!("date_trunc('{unit}', {r})::date")
        }
    }
}

pub fn compile(schema: &Schema, q: &Question, engine: &str, bucket_name: Option<&str>) -> String {
    let mut x = Ctx { schema, engine, aliases: HashMap::from([(vec![], "b".to_string())]), joins: Vec::new() };
    let base = &schema.tables[q.base];
    let m = &q.measure;
    if m.kind == "per" {
        let k = x.b(m.cols[0]);
        return format!(
            "SELECT n AS g, COUNT(*) AS v FROM (SELECT {k} AS k, COUNT(*) AS n FROM {} b WHERE {k} IS NOT NULL GROUP BY {k}) x GROUP BY n ORDER BY n LIMIT 200",
            x.e(&base.name)
        );
    }
    let value = match m.kind {
        "rows" => "COUNT(*)".to_string(),
        "column" => {
            let r = x.b(m.cols[0]);
            format!("{}({r})", m.agg.to_uppercase())
        }
        "product" => {
            let (a, y) = (m.cols[0], m.cols[1]);
            let yref = if y.0 != q.base {
                // the parent's price, one step away
                let step = base.out.iter().find(|&&(_, p)| p == y.0).map(|&(c, _)| (q.base, c)).expect("a key to the price");
                x.colref(&[step], y.0, y.1)
            } else {
                x.b(y)
            };
            let xr = x.b(a);
            format!("SUM({xr} * {yref})")
        }
        "flagged" => {
            let r = x.b(m.cols[0]);
            if engine != "mysql" { format!("SUM(CASE WHEN {r}::int <> 0 THEN 1 ELSE 0 END)") } else { format!("SUM({r} <> 0)") }
        }
        "net" => {
            let (a, b, c) = (x.b(m.cols[0]), x.b(m.cols[1]), x.b(m.cols[2]));
            format!("SUM({a} * {b} * (1 - COALESCE({c}, 0)))")
        }
        "parents" => format!("COUNT(DISTINCT {})", x.b(m.cols[0])),
        "gap" => {
            let (a, y) = (x.b(m.cols[0]), x.b(m.cols[1]));
            if engine == "mysql" {
                format!("AVG(TIMESTAMPDIFF(SECOND, {a}, {y})) / 86400")
            } else {
                format!("AVG(EXTRACT(EPOCH FROM ({y}::timestamp - {a}::timestamp))) / 86400")
            }
        }
        _ => {
            let (a, y) = (x.b(m.cols[0]), x.b(m.cols[1]));
            format!("SUM(CASE WHEN {y} > {a} THEN 1 ELSE 0 END)")
        }
    };
    let (mut select, mut group, mut order, mut wh) = (Vec::<String>::new(), Vec::<String>::new(), Vec::<String>::new(), Vec::<String>::new());
    for (c, how) in alive(base) {
        let r = x.colref(&[], q.base, c);
        wh.push(if how == "null" {
            format!("{r} IS NULL")
        } else if engine == "mysql" {
            format!("COALESCE({r}, 0) = 0")
        } else {
            format!("NOT COALESCE({r}::int::boolean, false)")
        });
    }
    if let Some(td) = &q.time {
        let t = x.bucket(td, bucket_name);
        select.push(format!("{t} AS t"));
        group.push(t);
        order.push("1".to_string());
        let r = x.colref(&td.path, td.t, td.c as usize);
        wh.push(format!("{r} IS NOT NULL"));
    }
    if let Some(by) = &q.by {
        let pt = &schema.tables[by.t];
        let g = if by.c >= 0 {
            x.colref(&by.path, by.t, by.c as usize)
        } else if by.kind != "key" && named(pt) && pt.first_last.is_some() {
            let (f, l) = pt.first_last.unwrap();
            let (a, z) = (x.colref(&by.path, by.t, f), x.colref(&by.path, by.t, l));
            format!("CONCAT_WS(' ', {a}, {z})")
        } else if by.kind != "key" && named(pt) {
            x.colref(&by.path, by.t, pt.label.unwrap())
        } else {
            // the key the base holds, with no join
            let (t, c) = by.path[by.path.len() - 1];
            x.colref(&by.path[..by.path.len() - 1], t, c)
        };
        select.push(format!("{g} AS g"));
        group.push(g);
        if by.c == -1 && by.kind != "key" && named(pt) {
            if let Some(&k) = pt.pk.first() {
                if !by.path.is_empty() {
                    group.push(x.colref(&by.path, by.t, k));
                }
            }
        }
    }
    select.push(format!("{value} AS v"));
    let mut sql = format!("SELECT {} FROM {} b", select.join(", "), x.e(&base.name));
    if !x.joins.is_empty() {
        sql += &format!(" {}", x.joins.join(" "));
    }
    if !wh.is_empty() {
        sql += &format!(" WHERE {}", wh.join(" AND "));
    }
    if !group.is_empty() {
        sql += &format!(" GROUP BY {}", group.join(", "));
    }
    if q.shape == "top" {
        sql += " ORDER BY v DESC LIMIT 12";
    } else if q.time.is_some() {
        sql += &format!(" ORDER BY {}", order.join(", "));
    } else {
        sql += " ORDER BY v DESC LIMIT 50";
    }
    sql
}
