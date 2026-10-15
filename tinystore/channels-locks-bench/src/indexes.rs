use crate::{cpu_seconds, memory, percentile, us};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

fn schema(c: &Connection, kind: &str, suffix: &str) {
    if kind == "fts" {
        c.execute_batch(&format!("create virtual table search{suffix} using fts5(body, content='docs', content_rowid='id');
            create trigger ins{suffix} after insert on docs begin
                insert into search{suffix}(rowid,body) values(new.id,new.body); end;
            create trigger del{suffix} after delete on docs begin
                insert into search{suffix}(search{suffix},rowid,body) values('delete',old.id,old.body); end;
            create trigger upd{suffix} after update on docs begin
                insert into search{suffix}(search{suffix},rowid,body) values('delete',old.id,old.body);
                insert into search{suffix}(rowid,body) values(new.id,new.body); end;")).unwrap();
    } else if kind == "rtree" {
        c.execute_batch(&format!("create virtual table geo{suffix} using rtree(id,minx,maxx,miny,maxy);
            create trigger ins{suffix} after insert on docs begin
                insert into geo{suffix} values(new.id,new.x,new.x,new.y,new.y); end;
            create trigger del{suffix} after delete on docs begin delete from geo{suffix} where id=old.id; end;
            create trigger upd{suffix} after update on docs begin
                delete from geo{suffix} where id=old.id;
                insert into geo{suffix} values(new.id,new.x,new.x,new.y,new.y); end;")).unwrap();
    }
}

fn wal(path: &str) -> u64 {
    std::fs::metadata(format!("{path}-wal"))
        .map(|m| m.len())
        .unwrap_or(0)
}
fn objects(c: &Connection) -> Vec<Value> {
    c.prepare("select name,sum(pgsize) from dbstat group by name order by name")
        .unwrap()
        .query_map([], |r| {
            Ok(json!({"name":r.get::<_,String>(0)?,"bytes":r.get::<_,i64>(1)?}))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
fn point(i: usize) -> (f64, f64) {
    (
        ((i * 7919) % 10000) as f64 / 100.,
        ((i * 3571) % 10000) as f64 / 100.,
    )
}
fn body(i: usize) -> String {
    format!(
        "tenant{} order payment shipment customer{} invoice region{} status active product{}",
        i % 100,
        i % 1000,
        i % 20,
        i % 500
    )
}
fn query(c: &Connection, kind: &str, term: usize) -> (u64, u64) {
    let start = Instant::now();
    let result: i64 = match kind {
        "fts" => c
            .prepare_cached("select count(*) from search where search match ?1")
            .unwrap()
            .query_row([format!("tenant{term}")], |r| r.get(0))
            .unwrap(),
        "rtree" => c
            .prepare_cached(
                "select count(*) from geo where minx>=?1 and maxx<=?2 and miny>=?1 and maxy<=?2",
            )
            .unwrap()
            .query_row(params![term as f64, term as f64 + 10.], |r| r.get(0))
            .unwrap(),
        "plain" => c
            .prepare_cached("select count(*) from docs where body like ?1")
            .unwrap()
            .query_row([format!("tenant{term} %")], |r| r.get(0))
            .unwrap(),
        _ => panic!("unknown index kind"),
    };
    (result as u64, us(start.elapsed()))
}

pub fn run(a: &[String]) {
    let kind = &a[0];
    let rows: usize = a[1].parse().unwrap();
    let path = std::env::var("BENCH_DB").unwrap();
    let c = Connection::open(&path).unwrap();
    c.execute_batch(
        "pragma journal_mode=wal; pragma synchronous=FULL; pragma wal_autocheckpoint=0;
        pragma cache_size=-2048; create table docs(id integer primary key,body text,x real,y real)",
    )
    .unwrap();
    schema(&c, kind, "");
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    let initial = memory();
    let cpu = cpu_seconds();
    let start = Instant::now();
    let mut batch_latencies = vec![];
    for base in (0..rows).step_by(1000) {
        let began = Instant::now();
        c.execute_batch("begin immediate").unwrap();
        for i in base..(base + 1000).min(rows) {
            let (x, y) = point(i);
            c.prepare_cached("insert into docs values(?1,?2,?3,?4)")
                .unwrap()
                .execute(params![i as i64, body(i), x, y])
                .unwrap();
        }
        c.execute_batch("commit").unwrap();
        batch_latencies.push(us(began.elapsed()));
    }
    let insert_s = start.elapsed().as_secs_f64();
    let insert_wal = wal(&path);
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    let inserted_objects = objects(&c);
    let mut query_latencies = vec![];
    let mut checksum = 0;
    for i in 0..1000 {
        let (n, t) = query(&c, kind, i % 90);
        checksum += n;
        query_latencies.push(t);
    }
    let start = Instant::now();
    for base in (0..rows / 10).step_by(1000) {
        c.execute_batch("begin immediate").unwrap();
        for i in base..(base + 1000).min(rows / 10) {
            c.prepare_cached("update docs set body=?2,x=x+0.1,y=y+0.1 where id=?1")
                .unwrap()
                .execute(params![i as i64, format!("{} updated", body(i))])
                .unwrap();
        }
        c.execute_batch("commit").unwrap();
    }
    let update_s = start.elapsed().as_secs_f64();
    let update_wal = wal(&path);
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    let start = Instant::now();
    for base in (0..rows / 10).step_by(1000) {
        c.execute_batch("begin immediate").unwrap();
        c.execute(
            "delete from docs where id>=?1 and id<?2",
            params![base as i64, (base + 1000).min(rows / 10) as i64],
        )
        .unwrap();
        c.execute_batch("commit").unwrap();
    }
    let delete_s = start.elapsed().as_secs_f64();
    let delete_wal = wal(&path);
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    let mut rebuilt = json!(null);
    if kind != "plain" {
        let stop = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let (s, r, p, k) = (stop.clone(), ready.clone(), path.clone(), kind.clone());
        // The held snapshot deterministically pins the entire rebuild WAL until this reader exits.
        let reader = std::thread::spawn(move || {
            let reader = Connection::open(p).unwrap();
            reader.execute_batch("pragma query_only=on; begin").unwrap();
            let (first, _) = query(&reader, &k, 20);
            r.store(true, Ordering::Release);
            let mut times = vec![];
            while !s.load(Ordering::Acquire) {
                let (n, t) = query(&reader, &k, 20);
                assert_eq!(n, first);
                times.push(t);
            }
            reader.execute_batch("rollback").unwrap();
            times
        });
        while !ready.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        let start = Instant::now();
        c.execute_batch("begin immediate").unwrap();
        schema(&c, kind, "_next");
        if kind == "fts" {
            c.execute_batch("insert into search_next(search_next) values('rebuild')")
                .unwrap();
        } else {
            c.execute_batch("insert into geo_next select id,x,x,y,y from docs")
                .unwrap();
        }
        c.execute_batch("commit").unwrap();
        let duration = start.elapsed().as_secs_f64();
        let rebuild_wal = wal(&path);
        let checkpoint: (i64, i64, i64) = c
            .query_row("pragma wal_checkpoint(passive)", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        stop.store(true, Ordering::Release);
        let mut reader_times = reader.join().unwrap();
        let mut next_writes = vec![];
        for i in rows..rows + 1000 {
            let began = Instant::now();
            c.execute_batch("begin immediate").unwrap();
            let (x, y) = point(i);
            c.prepare_cached("insert into docs values(?1,?2,?3,?4)")
                .unwrap()
                .execute(params![i as i64, body(i), x, y])
                .unwrap();
            c.execute_batch("commit").unwrap();
            next_writes.push(us(began.elapsed()));
        }
        if kind == "fts" {
            let old: i64 = c
                .query_row(
                    "select count(*) from search where search match 'tenant20'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let next: i64 = c
                .query_row(
                    "select count(*) from search_next where search_next match 'tenant20'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(old, next);
            c.execute_batch(
                "insert into search(search) values('integrity-check');
                insert into search_next(search_next) values('integrity-check')",
            )
            .unwrap();
        } else {
            let diff: i64 = c
                .query_row(
                    "select count(*) from (select * from geo except select * from geo_next)",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(diff, 0);
        }
        rebuilt = json!({"duration_s":duration,"wal_bytes":rebuild_wal,"held_reader_queries":reader_times.len(),
            "held_reader_p50_us":percentile(&mut reader_times,0.5),"held_reader_p99_us":percentile(&mut reader_times,0.99),
            "checkpoint_busy":checkpoint.0,"checkpoint_log_frames":checkpoint.1,"checkpoint_done_frames":checkpoint.2,
            "dual_write_p50_us":percentile(&mut next_writes,0.5),"dual_write_p99_us":percentile(&mut next_writes,0.99),
            "note":"one writer transaction blocks all other writers; concurrent old readers survive; no chunked rebuild/switch prototype"});
    }
    assert_eq!(
        c.query_row("pragma integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    println!(
        "{}",
        json!({"experiment":"indexes","kind":kind,"rows":rows,"batch_rows":1000,
        "insert_per_s":rows as f64/insert_s,"insert_s":insert_s,"insert_wal_bytes":insert_wal,
        "batch_p50_us":percentile(&mut batch_latencies,0.5),"batch_p99_us":percentile(&mut batch_latencies,0.99),
        "query_p50_us":percentile(&mut query_latencies,0.5),"query_p99_us":percentile(&mut query_latencies,0.99),
        "query_count":1000,"query_checksum":checksum,"update_rows":rows/10,"update_s":update_s,"update_wal_bytes":update_wal,
        "delete_rows":rows/10,"delete_s":delete_s,"delete_wal_bytes":delete_wal,"inserted_dbstat":inserted_objects,
        "final_dbstat":objects(&c),"final_db_bytes":std::fs::metadata(&path).unwrap().len(),
        "rebuild":rebuilt,"cpu_s":cpu_seconds()-cpu,"initial_memory":initial,"final_memory":memory(),
        "sqlite":rusqlite::version(),"status":"pass"})
    );
}
