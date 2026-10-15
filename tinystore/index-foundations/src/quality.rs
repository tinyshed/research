use crate::{common::*, fixture, native};
use serde_json::json;

pub fn run() {
    let c = open(&path());
    c.execute_batch("create virtual table literal using fts5(t,tokenize='unicode61');create virtual table stem using fts5(t,tokenize='unicode61');create virtual table geo using rtree(id,x0,x1,y0,y1)").unwrap();
    let docs = [
        "Человек идёт в город",
        "Люди шли по городам",
        "Машина работает",
        "Машины работали",
        "Ёлка и елки",
        "Сталь прочна",
        "Они стали работать",
    ];
    for (i, d) in docs.iter().enumerate() {
        c.execute(
            "insert into literal(rowid,t) values(?1,?2)",
            rusqlite::params![i as i64, *d],
        )
        .unwrap();
        c.execute(
            "insert into stem(rowid,t) values(?1,?2)",
            rusqlite::params![i as i64, fixture::normalize(d, true)],
        )
        .unwrap();
    }
    let gold = [
        ("человек", vec![0, 1]),
        ("город", vec![0, 1]),
        ("машина", vec![2, 3]),
        ("елка", vec![4]),
        ("сталь", vec![5]),
    ];
    let mut results = vec![];
    for (q, expected) in gold {
        for (table, query) in [
            ("literal", q.to_owned()),
            ("stem", fixture::normalize(q, true)),
        ] {
            let ids: Vec<i64> = c
                .prepare(&format!(
                    "select rowid from {table} where {table} match ?1 order by rowid"
                ))
                .unwrap()
                .query_map([&query], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            let tp = ids.iter().filter(|id| expected.contains(id)).count();
            results.push(json!({"query":q,"mode":table,"expected":expected,"actual":ids,"precision":tp as f64/ids.len().max(1) as f64,"recall":tp as f64/expected.len() as f64}));
        }
    }
    let points = [
        (-180., -90.),
        (180., 90.),
        (179.99999999, 0.),
        (-179.99999999, 0.),
        (37.60000001, 55.70000001),
        (37.60000002, 55.70000002),
    ];
    c.execute_batch("create table exact(id integer primary key,x real,y real)")
        .unwrap();
    for (i, (x, y)) in points.iter().enumerate() {
        c.execute(
            "insert into exact values(?1,?2,?3)",
            rusqlite::params![i as i64, x, y],
        )
        .unwrap();
        c.execute(
            "insert into geo values(?1,?2,?2,?3,?3)",
            rusqlite::params![i as i64, x, y],
        )
        .unwrap();
    }
    let q = (37.600000005, 37.600000015, 55.700000005, 55.700000015);
    let candidates: i64 = c
        .query_row(
            "select count(*) from geo where x0<=?2 and x1>=?1 and y0<=?4 and y1>=?3",
            rusqlite::params![q.0, q.1, q.2, q.3],
            |r| r.get(0),
        )
        .unwrap();
    let refined:i64=c.query_row("select count(*) from geo g join exact e using(id) where g.x0<=?2 and g.x1>=?1 and g.y0<=?4 and g.y1>=?3 and e.x between ?1 and ?2 and e.y between ?3 and ?4",rusqlite::params![q.0,q.1,q.2,q.3],|r|r.get(0)).unwrap();
    assert_eq!(refined, 1);
    let dateline: i64 = c
        .query_row(
            "select count(*) from exact where (x>=179.9 or x<=-179.9) and y between -1 and 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dateline, 2);
    // A cached statement binds to a SQL name. Generation metadata alone cannot retarget it.
    c.execute_batch("create table old_index(id);insert into old_index values(1);create table new_index(id);insert into new_index values(2),(3)").unwrap();
    let mut prepared = c.prepare("select count(*) from old_index").unwrap();
    assert_eq!(prepared.query_row([], |r| r.get::<_, i64>(0)).unwrap(), 1);
    c.execute_batch("begin;alter table old_index rename to retired_index;alter table new_index rename to old_index;commit").unwrap();
    let renamed = prepared.query_row([], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(renamed, 2);
    println!(
        "{}",
        json!({"experiment":"quality","russian_gold":results,"geo_candidates":candidates,"geo_exact_matches":refined,"dateline_matches":dateline,"prepared_rename_count":renamed,"sqlite":rusqlite::version(),"status":"pass"})
    );
    let _ = native::table(0);
}
