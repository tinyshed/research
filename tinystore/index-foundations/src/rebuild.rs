use crate::{
    common::*,
    fixture::{self, Fixture},
    native,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default)]
struct BuildStats {
    chunks: u64,
    rows: u64,
    input_bytes: u64,
    frames: u64,
    time_overruns: u64,
    page_overruns: u64,
    holds: Vec<u64>,
    commit_frames: Vec<u64>,
    row_limit: usize,
}

fn start(c: &Connection, g: i64, kind: &str, algo: &str, guard: bool, definition: i64) {
    c.execute_batch("begin immediate").unwrap();
    native::generation(c, g, kind, algo, definition, false, guard);
    c.execute_batch("commit").unwrap();
}
fn finish(c: &Connection, g: i64) {
    c.execute(
        "update generation set active=0,state='retiring' where active=1",
        [],
    )
    .unwrap();
    c.execute(
        "update generation set active=1,state='active' where id=?1",
        [g],
    )
    .unwrap();
}
fn chunk(c: &Connection, g: i64, row_limit: usize, time_us: u64) -> (bool, u64, u64) {
    let (kind, algo, high, frontier, definition): (String, String, Option<i64>, Option<i64>, i64) =
        c.query_row(
            "select kind,algorithm,high,frontier,definition from generation where id=?1",
            [g],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    let start = Instant::now();
    c.execute_batch("begin immediate").unwrap();
    let docs = if let Some(hi) = high {
        fixture::read(c, frontier, hi, row_limit)
    } else {
        vec![]
    };
    let mut rows = 0;
    let mut copied = 0;
    let mut last = frontier;
    for d in &docs {
        let seen = algo == "seen"
            && c.query_row(
                "select exists(select 1 from seen where generation=?1 and id=?2)",
                params![g, d.id],
                |r| r.get::<_, bool>(0),
            )
            .unwrap();
        if !seen {
            native::add(c, g, &kind, d, definition);
            if algo == "seen" {
                c.execute("insert into seen values(?1,?2)", params![g, d.id])
                    .unwrap();
            }
        }
        last = Some(d.id);
        rows += 1;
        copied += (d.body.len() + d.terms.len()) as u64;
        if time_us > 0 && us(start) >= time_us {
            break;
        }
    }
    let done = docs.is_empty();
    c.execute(
        "update generation set frontier=?2 where id=?1",
        params![g, if done { high } else { last }],
    )
    .unwrap();
    c.execute_batch("commit").unwrap();
    (done, rows, copied)
}
fn drain(c: &Connection, g: i64) -> u64 {
    let (kind, _, definition) = c
        .query_row(
            "select kind,algorithm,definition from generation where id=?1",
            [g],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    let ids: Vec<i64> = c
        .prepare("select id from dirty order by id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for id in &ids {
        native::remove(c, g, &kind, *id);
        let docs = fixture::read(
            c,
            if *id == i64::MIN { None } else { Some(*id - 1) },
            *id,
            1,
        );
        if let Some(d) = docs.first() {
            native::add(c, g, &kind, d, definition);
        }
    }
    c.execute("delete from dirty", []).unwrap();
    ids.len() as u64
}
fn complete(c: &Connection, g: i64) -> u64 {
    c.execute_batch("begin immediate").unwrap();
    let algo: String = c
        .query_row("select algorithm from generation where id=?1", [g], |r| {
            r.get(0)
        })
        .unwrap();
    let reconciled = if algo == "delta" { drain(c, g) } else { 0 };
    finish(c, g);
    c.execute_batch("commit").unwrap();
    reconciled
}
fn abandon(c: &Connection, g: i64) {
    c.execute_batch("begin immediate").unwrap();
    for suffix in ["ins", "upd", "del"] {
        c.execute_batch(&format!("drop trigger if exists g{g}_{suffix}"))
            .unwrap();
    }
    c.execute("update generation set state='aborted' where id=?1", [g])
        .unwrap();
    c.execute("delete from seen where generation=?1", [g])
        .unwrap();
    c.execute("delete from dirty", []).unwrap();
    c.execute_batch("commit").unwrap();
}
fn verify(c: &Connection, shape: &str) -> Value {
    let (active, kind, definition) = native::active(c);
    c.execute_batch("begin immediate").unwrap();
    native::create(c, 9999, &kind);
    native::build(c, 9999, &kind, definition);
    c.execute_batch("commit").unwrap();
    let mut checks = 0;
    for q in 0..120 {
        assert_eq!(
            native::query(c, active, &kind, q, shape),
            native::query(c, 9999, &kind, q, shape),
            "search differs from clean rebuild for query {q}"
        );
        checks += 1;
    }
    if kind == "fts" {
        let all: i64 = c
            .query_row("select count(*) from documents", [], |r| r.get(0))
            .unwrap();
        let rows: i64 = c
            .query_row(&format!("select count(*) from g{active}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(all, rows);
        if definition > 1 {
            let matched: i64 = c
                .query_row(
                    &format!("select count(*) from g{active} where g{active} match ?1"),
                    [format!("def{definition}")],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(all, matched);
        }
        c.execute_batch(&format!("insert into g{active}(g{active}) values('integrity-check');insert into g9999(g9999) values('integrity-check')")).unwrap();
    } else {
        let diff: i64 = c
            .query_row(
                &format!(
                    "select count(*) from (select * from g{active} except select * from g9999)"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(diff, 0);
        let reverse: i64 = c
            .query_row(
                &format!(
                    "select count(*) from (select * from g9999 except select * from g{active})"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reverse, 0);
    }
    validate(c);
    json!({"queries_equal":checks,"active":active,"definition":definition,"integrity":"ok"})
}

pub fn run(a: &[String]) {
    let kind = &a[0];
    let algo = &a[1];
    let n: usize = a[2].parse().unwrap();
    let shape = &a[3];
    let time_budget: u64 = a[4].parse().unwrap();
    let page_budget: u64 = a[5].parse().unwrap();
    let guard = a[6] == "selective";
    let tx_rows: usize = a[7].parse().unwrap();
    let indexed_percent: u64 = a[8].parse().unwrap();
    let ops: usize = a.get(9).map(|v| v.parse().unwrap()).unwrap_or(5000);
    let path = path();
    let c = open(&path);
    if !a.get(10).is_some_and(|s| s == "prepared") {
        fixture::seed(&c, n, shape, "ordered", 1000);
        native::initialize_active(&c, kind, guard);
    }
    // The template's initial generation is equivalent; only the update guard is changed per candidate.
    for suffix in ["ins", "upd", "del"] {
        c.execute_batch(&format!("drop trigger if exists g0_{suffix}"))
            .unwrap();
    }
    native::triggers(&c, 0, kind, "active", guard);
    checkpoint(&c);
    observe_wal(&c);
    let baseline_memory = memory();
    let cpu0 = cpu();
    let writer = Arc::new(Mutex::new(c));
    let stop = Arc::new(AtomicBool::new(false));
    let peak_file = Arc::new(AtomicU64::new(0));
    let rss_peak = Arc::new(AtomicU64::new(0));
    let (s, p, r, filename) = (
        stop.clone(),
        peak_file.clone(),
        rss_peak.clone(),
        path.clone(),
    );
    let monitor = std::thread::spawn(move || {
        while !s.load(Ordering::Relaxed) {
            p.fetch_max(bytes(&filename) + wal_bytes(&filename), Ordering::Relaxed);
            r.fetch_max(memory()["rss_kib"].as_u64().unwrap(), Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let origin = Instant::now();
    let (w, shape_owned) = (writer.clone(), shape.clone());
    let foreground = std::thread::spawn(move || {
        let f = Fixture::new(&shape_owned);
        let mut latencies = vec![];
        let mut blocked = vec![];
        let mut service = vec![];
        let mut changed = 0u64;
        for seq in 0..ops {
            let target = origin + Duration::from_micros(seq as u64 * 1000);
            if target > Instant::now() {
                std::thread::sleep(target.saturating_duration_since(Instant::now()));
            }
            let asked = Instant::now();
            let c = w.lock().unwrap();
            let held = Instant::now();
            blocked.push(us(asked));
            c.execute_batch("begin immediate").unwrap();
            for row in 0..tx_rows {
                let serial = (seq * tx_rows + row) as u64;
                let id = (fixture::mix(serial) % n as u64 + 1) as i64;
                match serial % 10 {
                    0 | 1 => {
                        let d = f.doc(n as i64 + serial as i64 + 1);
                        fixture::insert(&c, &d);
                        changed += 1;
                    }
                    2 => {
                        changed += c
                            .execute("delete from documents where id=?1", [id])
                            .unwrap() as u64;
                    }
                    3 => {
                        changed += c
                            .execute(
                                "update documents set id=?2 where id=?1",
                                params![id, n as i64 * 100 + serial as i64 + 1],
                            )
                            .unwrap() as u64;
                    }
                    _ => {
                        let is_indexed = fixture::mix(serial + 77) % 100 < indexed_percent;
                        if is_indexed {
                            changed+=c.execute("update documents set terms=terms||' revised',x=x+0.0000001,score=score+1,category=(category+1)%100 where id=?1",[id]).unwrap() as u64;
                        } else {
                            changed += c
                                .execute("update documents set other=other+1 where id=?1", [id])
                                .unwrap() as u64;
                        }
                    }
                }
            }
            c.execute_batch("commit").unwrap();
            maybe_checkpoint(&c);
            service.push(us(held));
            drop(c);
            latencies.push(target.elapsed().as_micros() as u64);
        }
        json!({"transactions":ops,"affected_rows":changed,"scheduled_latency":distribution(&mut latencies),
            "writer_queue":distribution(&mut blocked),"writer_service":distribution(&mut service)})
    });
    let mut readers = vec![];
    for index in 0..2 {
        let (p, s, shape) = (path.clone(), stop.clone(), shape.clone());
        readers.push(std::thread::spawn(move|| {
            let c=open(&p);c.execute_batch("pragma query_only=on").unwrap();let mut times=vec![];let mut count=0;let mut checksum=0u64;
            while !s.load(Ordering::Relaxed) {
                let t=Instant::now();c.execute_batch("begin").unwrap();let (g,k,_)=native::active(&c);
                let ids=native::query(&c,g,&k,count*2+index,&shape);checksum^=hash_ids(&ids);c.execute_batch("rollback").unwrap();
                if count%16==0&&times.len()<65536{times.push(us(t));}count+=1;
            }json!({"queries":count,"latency_sample_every":16,"latency":distribution(&mut times),"checksum":checksum})
        }));
    }
    std::thread::sleep(Duration::from_millis(100));
    let rebuilding = Instant::now();
    let mut b = BuildStats {
        row_limit: 64,
        ..Default::default()
    };
    let mut reconciled = 0;
    if algo == "monolithic" {
        let c = writer.lock().unwrap();
        let held = Instant::now();
        c.execute_batch("begin immediate").unwrap();
        native::generation(&c, 1, kind, "monolithic", 2, false, guard);
        native::build(&c, 1, kind, 2);
        finish(&c, 1);
        c.execute_batch("commit").unwrap();
        b.holds.push(us(held));
        b.chunks = 1;
        b.rows = n as u64;
        b.frames = WAL_COMMIT_FRAMES.load(Ordering::Relaxed);
    } else if algo != "none" {
        {
            let c = writer.lock().unwrap();
            start(&c, 1, kind, algo, guard, 2);
        }
        loop {
            let c = writer.lock().unwrap();
            let held = Instant::now();
            let (done, rows, copied) = chunk(&c, 1, b.row_limit, time_budget);
            let elapsed = us(held);
            let frames = WAL_COMMIT_FRAMES.load(Ordering::Relaxed);
            b.chunks += 1;
            b.rows += rows;
            b.input_bytes += copied;
            b.frames += frames;
            b.holds.push(elapsed);
            b.commit_frames.push(frames);
            if time_budget > 0 && elapsed > time_budget {
                b.time_overruns += 1;
            }
            if page_budget > 0 && frames > page_budget {
                b.page_overruns += 1;
            }
            if page_budget > 0 && rows > 0 {
                b.row_limit = ((page_budget as f64 * rows as f64 / frames.max(1) as f64) * 0.8)
                    .clamp(1., 1000.) as usize;
            }
            maybe_checkpoint(&c);
            drop(c);
            if done {
                break;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        {
            let c = writer.lock().unwrap();
            let held = Instant::now();
            reconciled = complete(&c, 1);
            b.holds.push(us(held));
        }
    }
    let rebuild_us = us(rebuilding);
    let fg = foreground.join().unwrap();
    let total_s = origin.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let reader_stats: Vec<Value> = readers.into_iter().map(|h| h.join().unwrap()).collect();
    monitor.join().unwrap();
    let c = writer.lock().unwrap();
    let workload_memory = memory();
    let workload_wal = wal_stats();
    let workload_objects = objects(&c);
    let before_validation_bytes = bytes(&path);
    let workload_cpu = cpu() - cpu0;
    let validation = verify(&c, shape);
    println!(
        "{}",
        json!({"experiment":"rebuild","kind":kind,"algorithm":algo,"rows":n,"shape":shape,"guard":if guard{"selective"}else{"full"},
        "time_budget_us":time_budget,"wal_frame_budget":page_budget,"tx_rows":tx_rows,"indexed_update_percent":indexed_percent,
        "rebuild_us":rebuild_us,"rebuild_rows_per_s":b.rows as f64*1e6/rebuild_us.max(1) as f64,"chunks":b.chunks,
        "backfill_rows":b.rows,"backfill_input_bytes_lower_bound":b.input_bytes,"builder_wal_frames":b.frames,"reconciled_dirty_ids":reconciled,
        "time_budget_overruns":b.time_overruns,"frame_budget_overruns":b.page_overruns,"builder_holds":distribution(&mut b.holds),
        "builder_commit_frames":distribution(&mut b.commit_frames),"foreground":fg,"readers":reader_stats,"duration_s":total_s,
        "query_per_s":reader_stats.iter().map(|r|r["queries"].as_u64().unwrap()).sum::<u64>() as f64/total_s,
        "baseline_memory":baseline_memory,"workload_memory":workload_memory,"sampled_rss_peak_kib":rss_peak.load(Ordering::Relaxed),
        "workload_wal":workload_wal,"peak_db_plus_wal_bytes":peak_file.load(Ordering::Relaxed),"db_bytes_before_validation":before_validation_bytes,
        "objects_before_validation":workload_objects,"cpu_s":workload_cpu,"cpu_s_including_validation":cpu()-cpu0,"validation":validation,"allocations":allocations(),"status":"pass"})
    );
}

pub fn check(a: &[String]) {
    let kind = &a[0];
    let algo = &a[1];
    let path = path();
    let c = open(&path);
    c.execute_batch(fixture::SCHEMA).unwrap();
    let f = Fixture::new("uniform");
    for id in [i64::MIN, -20, -2, 0, 2, 7, 11, 50] {
        fixture::insert(&c, &f.doc(id));
    }
    native::initialize_active(&c, kind, true);
    start(&c, 1, kind, algo, true, 2);
    chunk(&c, 1, 3, 0);
    // Changes on both sides, insertion into holes, rowid moves and reuse are all transactional.
    c.execute_batch(
        "begin immediate;
        update documents set terms='changed past frontier',x=179.99999999 where id=-20;
        update documents set terms='changed future frontier',y=-89.99999999 where id=50;
        delete from documents where id=-2; delete from documents where id=11;
        update documents set id=-30 where id=7; update documents set id=1000 where id=0; commit",
    )
    .unwrap();
    fixture::insert(&c, &f.doc(-1));
    fixture::insert(&c, &f.doc(100));
    fixture::insert(&c, &f.doc(-2));
    while !chunk(&c, 1, 3, 0).0 {}
    complete(&c, 1);
    // Re-request a different definition midway; the old build is retired, never relabelled in place.
    start(&c, 2, kind, algo, true, 3);
    chunk(&c, 2, 2, 0);
    abandon(&c, 2);
    start(&c, 3, kind, algo, true, 4);
    while !chunk(&c, 3, 2, 0).0 {}
    complete(&c, 3);
    let validation = verify(&c, "uniform");
    // Prepared direct SQL uses its physical table; registry switching does not retarget it.
    let mut old = c
        .prepare_cached(if kind == "fts" {
            "select count(*) from g0"
        } else {
            "select count(*) from g0"
        })
        .unwrap();
    let old_count: i64 = old.query_row([], |r| r.get(0)).unwrap();
    let active_count: i64 = c
        .query_row("select count(*) from g3", [], |r| r.get(0))
        .unwrap();
    println!(
        "{}",
        json!({"experiment":"rebuild-check","kind":kind,"algorithm":algo,"validation":validation,
        "old_physical_table_count":old_count,"active_count":active_count,"prepared_statement_auto_retarget":false,
        "checks":["minimum i64 id","holes","past/future update/delete","rowid crossing frontier both directions","insert above H","delete-reinsert","definition superseded"],"status":"pass"})
    );
}

pub fn crash(a: &[String]) {
    use std::io::Write;
    let kind = &a[0];
    let algo = &a[1];
    let stage = &a[2];
    let path = path();
    let c = open(&path);
    fixture::seed(&c, 300, "uniform", "ordered", 100);
    native::initialize_active(&c, kind, true);
    start(&c, 1, kind, algo, true, 2);
    match stage.as_str() {
        "prepared" => {}
        "chunk-uncommitted" => {
            c.execute_batch("begin immediate").unwrap();
            let d = Fixture::new("uniform").doc(1);
            native::add(&c, 1, kind, &d, 2);
        }
        "chunk-committed" => {
            chunk(&c, 1, 50, 0);
        }
        "switch-uncommitted" | "switched" | "retire-uncommitted" => {
            while !chunk(&c, 1, 100, 0).0 {}
            if stage == "switch-uncommitted" {
                c.execute_batch("begin immediate").unwrap();
                finish(&c, 1);
            } else {
                complete(&c, 1);
                if stage == "retire-uncommitted" {
                    c.execute_batch("begin immediate;drop trigger g0_ins;drop trigger g0_del;drop trigger g0_upd;drop table g0").unwrap();
                }
            }
        }
        _ => panic!("unknown crash stage"),
    }
    println!("{}", json!({"ready":true,"stage":stage}));
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}
pub fn recover(_a: &[String]) {
    let start = Instant::now();
    let path = path();
    let c = open(&path);
    let pending: Option<i64> = c
        .query_row(
            "select id from generation where state='building' order by id desc limit 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .unwrap();
    if let Some(g) = pending {
        while !chunk(&c, g, 100, 0).0 {}
        complete(&c, g);
    }
    let recovery_us = us(start);
    let validation = verify(&c, "uniform");
    println!(
        "{}",
        json!({"experiment":"recovery","recovery_us":recovery_us,"resumed":pending,"validation":validation,"status":"pass"})
    );
}
