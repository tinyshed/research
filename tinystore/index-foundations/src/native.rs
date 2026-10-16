use crate::{
    common::*,
    fixture::{self, Doc, Fixture},
};
use rusqlite::{Connection, params};
use serde_json::json;
use std::time::Instant;

pub fn table(g: i64) -> String {
    format!("g{g}")
}
pub fn projected(terms: &str, definition: i64) -> String {
    if definition == 1 {
        terms.to_owned()
    } else {
        format!("{terms} def{definition}")
    }
}
pub fn create(c: &Connection, g: i64, kind: &str) {
    if kind.starts_with("scan-") {
        return;
    }
    let name = table(g);
    let sql = match kind {
        "fts" => format!(
            "create virtual table {name} using fts5(terms,tokenize='unicode61 remove_diacritics 2')"
        ),
        "fts-external" => format!(
            "create virtual table {name} using fts5(terms,content='documents',content_rowid='id',tokenize='unicode61 remove_diacritics 2')"
        ),
        "fts-set" => format!(
            "create virtual table {name} using fts5(terms,content='documents',content_rowid='id',detail=none,columnsize=0,tokenize='unicode61 remove_diacritics 2')"
        ),
        "geo" => format!("create virtual table {name} using rtree(id,minx,maxx,miny,maxy)"),
        "grid" => format!(
            "create table {name}(id integer primary key,cx integer,cy integer); create index {name}_cell on {name}(cx,cy,id)"
        ),
        "btree" => format!("create index {name} on documents(category,score,id)"),
        "postings" => format!(
            "create table {name}(category integer,id integer,primary key(category,id)) without rowid"
        ),
        _ => panic!("unknown native index kind"),
    };
    c.execute_batch(&sql).unwrap();
}
pub fn add(c: &Connection, g: i64, kind: &str, d: &Doc, definition: i64) {
    let name = table(g);
    match kind {
        k if k.starts_with("scan-") => {}
        "fts" | "fts-external" | "fts-set" => {
            c.prepare_cached(&format!("insert into {name}(rowid,terms) values(?1,?2)"))
                .unwrap()
                .execute(params![d.id, projected(&d.terms, definition)])
                .unwrap();
        }
        "geo" => {
            c.prepare_cached(&format!("insert into {name} values(?1,?2,?2,?3,?3)"))
                .unwrap()
                .execute(params![d.id, d.x, d.y])
                .unwrap();
        }
        "grid" => {
            c.prepare_cached(&format!("insert into {name} values(?1,?2,?3)"))
                .unwrap()
                .execute(params![d.id, d.x.floor() as i64, d.y.floor() as i64])
                .unwrap();
        }
        "postings" => {
            c.prepare_cached(&format!("insert into {name} values(?1,?2)"))
                .unwrap()
                .execute(params![d.category, d.id])
                .unwrap();
        }
        "btree" => {}
        _ => panic!("unknown kind"),
    }
}
pub fn remove(c: &Connection, g: i64, kind: &str, id: i64) {
    let name = table(g);
    let key = if kind == "fts" { "rowid" } else { "id" };
    assert_ne!(
        kind, "fts-external",
        "external content needs OLD terms, not current source text"
    );
    c.prepare_cached(&format!("delete from {name} where {key}=?1"))
        .unwrap()
        .execute([id])
        .unwrap();
}

fn expr_terms(which: &str, g: i64) -> String {
    format!(
        "{which}.terms || case when (select definition from generation where id={g})=1 then '' else ' def'||(select definition from generation where id={g}) end"
    )
}
pub fn triggers(c: &Connection, g: i64, kind: &str, algorithm: &str, guard: bool) {
    if kind == "btree" || kind.starts_with("scan-") {
        return;
    }
    let name = table(g);
    let covered = |which: &str| {
        match algorithm {
        "frontier"=>format!("((select state from generation where id={g})='active' or
            ((select frontier from generation where id={g}) is not null and {which}.id<=(select frontier from generation where id={g})) or
            (select high from generation where id={g}) is null or {which}.id>(select high from generation where id={g}))"),
        "seen" if which=="old"=>format!("((select state from generation where id={g})='active' or exists(select 1 from seen where generation={g} and id=old.id))"),
        "delta"=>format!("(select state from generation where id={g})='active'"),
        _=>"1".to_owned(),
    }
    };
    let enabled = format!("(select state from generation where id={g}) in ('active','building')");
    let old_where = format!("{enabled} and {}", covered("old"));
    let new_where = format!("{enabled} and {}", covered("new"));
    let del = match kind {
        "fts-external" | "fts-set" => format!(
            "insert into {name}({name},rowid,terms) select 'delete',old.id,{} where {old_where};",
            expr_terms("old", g)
        ),
        "fts" => format!("delete from {name} where rowid=old.id and {old_where};"),
        "postings" => {
            format!("delete from {name} where category=old.category and id=old.id and {old_where};")
        }
        _ => format!("delete from {name} where id=old.id and {old_where};"),
    };
    let ins = match kind {
        "fts" | "fts-external" | "fts-set" => format!(
            "insert into {name}(rowid,terms) select new.id,{} where {new_where};",
            expr_terms("new", g)
        ),
        "geo" => {
            format!("insert into {name} select new.id,new.x,new.x,new.y,new.y where {new_where};")
        }
        "grid" => format!(
            "insert into {name} select new.id,cast(floor(new.x) as integer),cast(floor(new.y) as integer) where {new_where};"
        ),
        "postings" => format!("insert into {name} select new.category,new.id where {new_where};"),
        _ => unreachable!(),
    };
    let remember = if algorithm == "seen" {
        format!("insert or ignore into seen select {g},new.id where {new_where};")
    } else {
        String::new()
    };
    let forget = if algorithm == "seen" {
        format!("delete from seen where generation={g} and id=old.id;")
    } else {
        String::new()
    };
    let dirty = |which: &str| {
        if algorithm == "delta" {
            format!(
                "insert or ignore into dirty select {which}.id where (select state from generation where id={g})='building';"
            )
        } else {
            String::new()
        }
    };
    let guard_expr = if !guard {
        "1".to_owned()
    } else {
        match kind {
            "fts" | "fts-external" | "fts-set" => {
                "old.id is not new.id or old.terms is not new.terms".to_owned()
            }
            "geo" | "grid" => {
                "old.id is not new.id or old.x is not new.x or old.y is not new.y".to_owned()
            }
            _ => "old.id is not new.id or old.category is not new.category".to_owned(),
        }
    };
    c.execute_batch(&format!("create trigger {name}_ins after insert on documents begin {ins}{remember}{} end;
        create trigger {name}_del after delete on documents begin {del}{forget}{} end;
        create trigger {name}_upd after update on documents when {guard_expr} begin {del}{forget}{ins}{remember}{}{} end;",
        dirty("new"),dirty("old"),dirty("old"),dirty("new"))).unwrap();
}
pub fn build(c: &Connection, g: i64, kind: &str, definition: i64) {
    if kind.starts_with("fts") && std::env::var_os("BENCH_SQL_BULK").is_some() {
        c.execute(
            &format!("insert into g{g}(rowid,terms) select id,terms || case when ?1=1 then '' else ' def'||?1 end from documents order by id"),
            [definition],
        ).unwrap();
        return;
    }
    if kind == "geo" && std::env::var_os("BENCH_SQL_BULK").is_some() {
        let order = std::env::var("BENCH_BUILD_ORDER").unwrap_or("rowid".to_owned());
        c.create_scalar_function(
            "mixid",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
                | rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            |ctx| Ok(fixture::mix(ctx.get::<i64>(0)? as u64) as i64),
        )
        .unwrap();
        let by = match order.as_str() {
            "rowid" => "id",
            "spatial" => "x,y,id",
            "shuffled" => "mixid(id),id",
            _ => panic!("unknown build order"),
        };
        c.execute_batch(&format!(
            "insert into g{g} select id,x,x,y,y from documents order by {by}"
        ))
        .unwrap();
        return;
    }

    let mut cursor = None;
    loop {
        let docs = fixture::read(c, cursor, i64::MAX, 1000);
        if docs.is_empty() {
            break;
        }
        for d in &docs {
            add(c, g, kind, d, definition);
        }
        cursor = Some(docs.last().unwrap().id);
    }
}
pub fn active(c: &Connection) -> (i64, String, i64) {
    c.query_row(
        "select id,kind,definition from generation where active=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

pub fn query(c: &Connection, g: i64, kind: &str, q: usize, shape: &str) -> Vec<i64> {
    let name = table(g);
    match kind {
        "scan-text" => c
            .prepare_cached(
                "select id from documents where instr(' '||terms||' ',' '||?1||' ')>0 order by id",
            )
            .unwrap()
            .query_map([query_term(q, shape)], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect(),
        "scan-geo" => {
            let (a, b, d, e) = rect(q, shape);
            c.prepare_cached("select id from documents where x between ?1 and ?2 and y between ?3 and ?4 order by id").unwrap().query_map(params![a,b,d,e],|r|r.get(0)).unwrap().map(Result::unwrap).collect()
        }
        "scan-secondary" => c
            .prepare_cached("select id from documents where category=?1 and score>=?2 order by id")
            .unwrap()
            .query_map(params![(q % 100) as i64, (q % 8 * 100) as f64], |r| {
                r.get(0)
            })
            .unwrap()
            .map(Result::unwrap)
            .collect(),

        "fts" | "fts-external" | "fts-set" => {
            let term = query_term(q, shape);
            c.prepare_cached(&format!(
                "select rowid from {name} where {name} match ?1 order by rowid"
            ))
            .unwrap()
            .query_map([term], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
        }
        "geo" | "grid" => {
            let (x0, x1, y0, y1) = rect(q, shape);
            let sql = if kind == "geo" {
                format!(
                    "select d.id from {name} g join documents d on d.id=g.id
                where g.minx<=?2 and g.maxx>=?1 and g.miny<=?4 and g.maxy>=?3
                and d.x>=?1 and d.x<=?2 and d.y>=?3 and d.y<=?4 order by d.id"
                )
            } else {
                format!(
                    "select d.id from {name} g join documents d on d.id=g.id where
                g.cx between cast(floor(?1) as integer) and cast(floor(?2) as integer)
                and g.cy between cast(floor(?3) as integer) and cast(floor(?4) as integer)
                and d.x>=?1 and d.x<=?2 and d.y>=?3 and d.y<=?4 order by d.id"
                )
            };
            c.prepare_cached(&sql)
                .unwrap()
                .query_map(params![x0, x1, y0, y1], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        }
        "btree" | "postings" => {
            let sql = if kind == "btree" {
                "select id from documents where category=?1 and score>=?2 order by id".to_owned()
            } else {
                format!(
                    "select d.id from {name} g join documents d on d.id=g.id where g.category=?1 and d.score>=?2 order by d.id"
                )
            };
            c.prepare_cached(&sql)
                .unwrap()
                .query_map(params![(q % 100) as i64, (q % 8 * 100) as f64], |r| {
                    r.get(0)
                })
                .unwrap()
                .map(Result::unwrap)
                .collect()
        }
        _ => panic!("unknown query kind"),
    }
}
pub fn query_term(q: usize, shape: &str) -> String {
    match shape {
        "russian" => ["человек", "работ", "город", "заказ", "документ", "год"][q % 6].to_owned(),
        "zipf" => format!("term{}", [1, 10, 99, 1000, 9999, 99999][q % 6]),
        _ => format!("tenant{}", q % 100),
    }
}
pub fn rect(q: usize, shape: &str) -> (f64, f64, f64, f64) {
    if shape == "clustered" {
        let widths = [0.00001, 0.0001, 0.001, 0.01];
        let w = widths[q % 4];
        (37.6, 37.6 + w, 55.7, 55.7 + w)
    } else {
        let w = [0.1, 1., 10., 90.][q % 4];
        let x = (q % 100) as f64 - 100.;
        let y = (q % 50) as f64 - 30.;
        (x, x + w, y, y + w)
    }
}
pub fn generation(
    c: &Connection,
    g: i64,
    kind: &str,
    algo: &str,
    definition: i64,
    is_active: bool,
    guard: bool,
) {
    let high: Option<i64> = c
        .query_row("select max(id) from documents", [], |r| r.get(0))
        .unwrap();
    c.execute(
        "insert into generation values(?1,?2,?3,?4,?5,null,?6,?7)",
        params![
            g,
            kind,
            if is_active { "active" } else { "building" },
            algo,
            high,
            definition,
            i64::from(is_active)
        ],
    )
    .unwrap();
    create(c, g, kind);
    triggers(c, g, kind, algo, guard);
}
pub fn initialize_active(c: &Connection, kind: &str, guard: bool) {
    c.execute_batch("begin immediate").unwrap();
    generation(c, 0, kind, "active", 1, true, guard);
    if kind != "btree" {
        build(c, 0, kind, 1);
    }
    c.execute_batch("commit").unwrap();
    checkpoint(c);
}
pub fn run(a: &[String]) {
    let kind = &a[0];
    let n: usize = a[1].parse().unwrap();
    let shape = &a[2];
    let order = &a[3];
    let batch: usize = a[4].parse().unwrap();
    let path = path();
    let c = open(&path);
    if !a.get(5).is_some_and(|s| s == "prepared") {
        fixture::seed(&c, n, shape, order, batch);
    }
    let initial = memory();
    let cpu0 = cpu();
    observe_wal(&c);
    let build_only = std::env::var_os("BENCH_BUILD_ONLY").is_some();
    let watching = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let temp_peak = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let monitor = if build_only {
        let (s, p) = (watching.clone(), temp_peak.clone());
        Some(std::thread::spawn(move || {
            while s.load(std::sync::atomic::Ordering::Relaxed) {
                p.fetch_max(temporary_bytes(), std::sync::atomic::Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }))
    } else {
        None
    };
    let begin = Instant::now();
    c.execute_batch("begin immediate").unwrap();
    generation(&c, 0, kind, "active", 1, true, true);
    if kind != "btree" {
        build(&c, 0, kind, 1);
    }
    c.execute_batch("commit").unwrap();
    let build_us = us(begin);
    let build_wal = wal_stats();
    let build_objects = objects(&c);
    let before_checkpoint_db = bytes(&path);
    let before_checkpoint_wal = wal_bytes(&path);
    checkpoint(&c);
    watching.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Some(h) = monitor {
        h.join().unwrap();
    }
    // Optional correctness observer runs outside the captured build time/WAL.
    // Both paired SQL-native controls enable it, preserving query warmup parity.
    if kind.starts_with("fts") && std::env::var_os("BENCH_VERIFY_FTS").is_some() {
        c.execute_batch("insert into g0(g0,rank) values('integrity-check',1)")
            .unwrap();
    }
    if build_only {
        let counts:(i64,i64)=c.query_row("select sum(length(cast(body as blob))),sum(length(cast(terms as blob))) from documents",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        println!(
            "{}",
            json!({"experiment":"build-only","kind":kind,"rows":n,"shape":shape,"source_order":order,"build_order":std::env::var("BENCH_BUILD_ORDER").unwrap_or("rowid".to_owned()),"sql_bulk":std::env::var_os("BENCH_SQL_BULK").is_some(),"cache_kib":cache_kib(),"build_us_including_diagnostic_observer":build_us,"build_wal":build_wal,"build_objects":build_objects,"initial_memory":initial,"final_memory":memory(),"db_bytes_before_checkpoint":before_checkpoint_db,"wal_file_before_checkpoint":before_checkpoint_wal,"db_bytes":bytes(&path),"checkpoint_peak_db_plus_wal_upper_bound":bytes(&path)+before_checkpoint_wal,"sampled_temp_file_peak_bytes":temp_peak.load(std::sync::atomic::Ordering::Relaxed),"source_body_utf8_bytes":counts.0,"source_terms_utf8_bytes":counts.1,"numeric_logical_bytes_per_row":48,"cpu_s_including_payload_sum":cpu()-cpu0,"allocations":allocations(),"status":"pass"})
        );
        return;
    }
    let mut query_times = vec![];
    let mut query_samples = vec![];
    let mut checksum = 0u64;
    let qstart = Instant::now();
    for q in 0..300 {
        let t = Instant::now();
        let ids = query(&c, 0, kind, q, shape);
        checksum = checksum.wrapping_mul(0x100000001b3) ^ hash_ids(&ids);
        let elapsed = us(t);
        query_times.push(elapsed);
        query_samples.push(json!({"q":q,"bucket":q % if kind.starts_with("fts")||kind=="scan-text"{6}else if ["geo","grid","scan-geo"].contains(&kind.as_str()){4}else{8},"us":elapsed,"matched":ids.len()}));
    }
    let query_seconds = qstart.elapsed().as_secs_f64();
    let f = Fixture::new(shape);
    let mutations = (n / 100).clamp(1000, 10000);

    let mut phases = vec![];
    for phase in ["insert", "update", "delete"] {
        let before = WAL_FRAMES.load(std::sync::atomic::Ordering::Relaxed);
        let begin = Instant::now();
        let mut phase_times = vec![];
        for base in (1..=mutations).step_by(batch) {
            let t = Instant::now();
            c.execute_batch("begin immediate").unwrap();
            for i in base..=(base + batch - 1).min(mutations) {
                match phase {
                    "insert" => fixture::insert(&c, &f.doc((n + i) as i64)),
                    "update" => {
                        c.prepare_cached("update documents set terms=terms||' revised',x=x+0.000001,category=(category+1)%100 where id=?1").unwrap().execute([i as i64]).unwrap();
                    }
                    _ => {
                        c.prepare_cached("delete from documents where id=?1")
                            .unwrap()
                            .execute([(n / 2 + i) as i64])
                            .unwrap();
                    }
                }
            }
            c.execute_batch("commit").unwrap();
            phase_times.push(us(t));
            maybe_checkpoint(&c);
        }
        phases.push(json!({"operation":phase,"rows":mutations,"duration_us":us(begin),"batches":distribution(&mut phase_times),"wal_frames":WAL_FRAMES.load(std::sync::atomic::Ordering::Relaxed)-before}));
    }
    validate(&c);
    println!(
        "{}",
        json!({"experiment":"native","kind":kind,"rows":n,"shape":shape,"order":order,"batch":batch,
        "build_us":build_us,"build_rows_per_s":n as f64*1e6/build_us.max(1) as f64,"build_wal":build_wal,
        "build_objects":build_objects,"queries":distribution(&mut query_times),"query_per_s":300./query_seconds,
        "query_checksum":checksum,"query_samples":query_samples,"mutations_each":mutations,"mutation_phases":phases,
        "all_wal":wal_stats(),"initial_memory":initial,"final_memory":memory(),
        "disk_bytes":bytes(&path),"wal_file_bytes":wal_bytes(&path),"objects":objects(&c),"cpu_s":cpu()-cpu0,
        "allocations":allocations(),"sqlite":rusqlite::version(),"status":"pass"})
    );
}

pub fn certified_box(c: &Connection, g: i64, rect: (f64, f64, f64, f64)) -> Vec<i64> {
    let (x0, x1, y0, y1) = rect;
    c.prepare_cached(&format!("select g.id from g{g} g
    where g.minx<=?2 and g.maxx>=?1 and g.miny<=?4 and g.maxy>=?3
    and case when g.minx>=?1 and g.maxx<=?2 and g.miny>=?3 and g.maxy<=?4 then 1
    else exists(select 1 from documents d where d.id=g.id and d.x>=?1 and d.x<=?2 and d.y>=?3 and d.y<=?4) end
    order by g.id")).unwrap().query_map(params![x0,x1,y0,y1],|r|r.get(0)).unwrap().map(Result::unwrap).collect()
}
pub fn geo_probe(a: &[String]) {
    let n: usize = a[0].parse().unwrap();
    let shape = &a[1];
    let c = open(&path());
    if !a.get(2).is_some_and(|v| v == "prepared") {
        fixture::seed(&c, n, shape, "ordered", 1000);
        initialize_active(&c, "geo", true);
    }
    let mut samples = vec![];
    let cpu0 = cpu();
    for q in 0..32 {
        let (plain, certified, plain_us, certified_us) = if (q / 4) % 2 == 0 {
            let t = Instant::now();
            let a = query(&c, 0, "geo", q, shape);
            let au = us(t);
            let t = Instant::now();
            let b = certified_box(&c, 0, rect(q, shape));
            let bu = us(t);
            (a, b, au, bu)
        } else {
            let t = Instant::now();
            let b = certified_box(&c, 0, rect(q, shape));
            let bu = us(t);
            let t = Instant::now();
            let a = query(&c, 0, "geo", q, shape);
            let au = us(t);
            (a, b, au, bu)
        };
        assert_eq!(plain, certified);
        samples.push(json!({"q":q,"bucket":q%4,"plain_us":plain_us,"certified_us":certified_us,"matched":plain.len(),"checksum":hash_ids(&plain)}));
    }
    println!(
        "{}",
        json!({"experiment":"geo-certified","rows":n,"shape":shape,"cache_kib":cache_kib(),"samples":samples,"samples_per_selectivity":8,"memory":memory(),"cpu_s":cpu()-cpu0,"status":"pass"})
    );
}
