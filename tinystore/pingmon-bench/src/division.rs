//! A metrics.db divided by b-tree, and its bytes a sample: division <db> <samples>.

use rusqlite::{Connection, OpenFlags};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (path, samples): (&str, f64) = (&args[1], args[2].parse().unwrap());
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let one = |sql: &str| -> i64 { db.query_row(sql, [], |row| row.get(0)).unwrap() };
    let total = one("pragma page_count") * one("pragma page_size");
    println!(
        "{} pages, {total} bytes, {:.3} B/sample, free {}",
        one("pragma page_count"),
        total as f64 / samples,
        one("pragma freelist_count")
    );
    println!("{:14} {:>6} {:>9} {:>9} {:>9} {:>8}", "b-tree", "pages", "bytes", "B/sample", "payload", "unused");
    let mut select = db
        .prepare("select name, count(*), sum(pgsize), sum(payload), sum(unused) from dbstat group by name order by 3 desc")
        .unwrap();
    let rows: Vec<(String, i64, i64, i64, i64)> = select
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut rest = (0, 0, 0, 0);
    for (name, pages, bytes, payload, unused) in rows {
        if pages > 1 {
            println!("{name:14} {pages:6} {bytes:9} {:9.3} {payload:9} {unused:8}", bytes as f64 / samples);
        } else {
            rest = (rest.0 + pages, rest.1 + bytes, rest.2 + payload, rest.3 + unused);
        }
    }
    println!("{:14} {:6} {:9} {:9.3} {:9} {:8}", "the rest", rest.0, rest.1, rest.1 as f64 / samples, rest.2, rest.3);
    let tables = [
        ("clocks", "body"),
        ("groups", "directory"),
        ("payloads", "body"),
        ("heads", "body"),
        ("blocks", "body"),
        ("block_grams", "grams"),
        ("block_traces", "bloom"),
        ("block_filters", "bloom"),
    ];
    for (table, column) in tables {
        let exists = |name: &str| -> bool {
            db.query_row("select count(*) from sqlite_schema where name = ?1", [name], |row| row.get::<_, i64>(0))
                .unwrap()
                > 0
        };
        if !exists(table) {
            continue;
        }
        let (count, length): (i64, i64) = db
            .query_row(&format!("select count(*), coalesce(sum(length({column})), 0) from {table}"), [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        println!("{table}: {count} rows, {length} bytes of {column}, {:.3} B/sample", length as f64 / samples);
    }
}
