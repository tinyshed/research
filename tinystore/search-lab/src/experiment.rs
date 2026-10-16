use crate::{analyzer::Analyzer, fts};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};
#[derive(Clone, Deserialize, Serialize)]
pub struct Doc {
    pub id: u64,
    #[serde(default)]
    pub key: String,
    pub title: String,
    pub body: String,
}
#[derive(Deserialize)]
pub struct Query {
    pub key: String,
    pub text: String,
    pub relevant: Vec<u64>,
    pub judged: BTreeMap<String, i32>,
    #[serde(default)]
    pub language: String,
}
#[derive(Deserialize)]
pub struct Corpus {
    pub docs: Vec<Doc>,
    pub queries: Vec<Query>,
}
fn us(t: Instant) -> u64 {
    t.elapsed().as_micros() as u64
}
fn distribution(v: &mut [u64]) -> Value {
    v.sort_unstable();
    let p = |f: f64| {
        if v.is_empty() {
            0
        } else {
            v[((v.len() - 1) as f64 * f).ceil() as usize]
        }
    };
    json!({"n":v.len(),"p50_us":p(0.5),"p95_us":p(0.95),"p99_us":p(0.99),"max_us":p(1.)})
}
fn memory() -> Value {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let get = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    json!({"rss_kib":get("VmRSS:"),"hwm_kib":get("VmHWM:")})
}
fn cpu() -> f64 {
    unsafe {
        let mut r: libc::rusage = std::mem::zeroed();
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, &mut r), 0);
        (r.ru_utime.tv_sec + r.ru_stime.tv_sec) as f64
            + (r.ru_utime.tv_usec + r.ru_stime.tv_usec) as f64 / 1e6
    }
}
fn bytes(p: &str) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}
fn directory(p: &Path) -> u64 {
    std::fs::read_dir(p)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            if p.is_dir() {
                directory(&p)
            } else {
                p.metadata().unwrap().len()
            }
        })
        .sum()
}
fn open(p: &str, mode: &str) -> Connection {
    let c = Connection::open(p).unwrap();
    fts::register(&c, mode);
    c.execute_batch("pragma journal_mode=wal;pragma synchronous=FULL;pragma cache_size=-1024;pragma wal_autocheckpoint=0;pragma trusted_schema=off;pragma busy_timeout=10000;").unwrap();
    c
}
fn schema(c: &Connection) {
    c.execute_batch("create table documents(id integer primary key,key text unique,title text not null,body text not null,other integer default 0);").unwrap();
}
fn create_index(c: &Connection, mode: &str) {
    let tokenizer = if mode == "unicode" {
        "unicode61"
    } else {
        "tiny_auto_v1"
    };
    c.execute_batch(&format!("create virtual table search using fts5(title,body,content='documents',content_rowid='id',tokenize='{tokenizer}');
create trigger doc_i after insert on documents begin insert into search(rowid,title,body)values(new.id,new.title,new.body);end;
create trigger doc_d after delete on documents begin insert into search(search,rowid,title,body)values('delete',old.id,old.title,old.body);end;
create trigger doc_u after update on documents when old.id is not new.id or old.title is not new.title or old.body is not new.body begin
insert into search(search,rowid,title,body)values('delete',old.id,old.title,old.body);insert into search(rowid,title,body)values(new.id,new.title,new.body);end;")).unwrap();
}
fn insert(c: &Connection, ds: &[Doc]) {
    c.execute_batch("begin immediate").unwrap();
    {
        let mut s = c
            .prepare_cached("insert into documents(id,key,title,body) values(?1,?2,?3,?4)")
            .unwrap();
        for d in ds {
            s.execute(params![d.id as i64, d.key, d.title, d.body])
                .unwrap();
        }
    }
    c.execute_batch("commit").unwrap();
}
fn search(c: &Connection, q: &str, mode: &str, all: bool) -> Vec<u64> {
    search_filtered(c, q, mode, all, None)
}
fn search_filtered(
    c: &Connection,
    q: &str,
    mode: &str,
    all: bool,
    filter: Option<Vec<u64>>,
) -> Vec<u64> {
    let q = fts::query_string(q, mode);
    if q.is_empty() {
        return vec![];
    }
    let sql = if all {
        "select rowid from search where search match ?1 order by rowid"
    } else {
        "select rowid from search where search match ?1 order by rank limit 10"
    };
    let sql = if let Some(ids) = filter {
        sql.replace(
            "where search match ?1",
            &format!(
                "where search match ?1 and rowid in ({})",
                ids.into_iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )
    } else {
        sql.to_owned()
    };
    c.prepare_cached(&sql)
        .unwrap()
        .query_map([q], |r| {
            r.get::<_, i64>(0).map(|i| u64::try_from(i).unwrap())
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
fn scores(q: &Query, ids: &[u64]) -> Value {
    let relevant: BTreeSet<_> = q.relevant.iter().copied().collect();
    let mut hit = 0;
    let mut rr = 0.;
    let mut dcg = 0.;
    let mut judged = 0;
    let mut negative = 0;
    for (i, id) in ids.iter().enumerate() {
        if q.judged.contains_key(&id.to_string()) {
            judged += 1;
        }
        if q.judged.get(&id.to_string()) == Some(&0) {
            negative += 1;
        }
        if relevant.contains(id) {
            hit += 1;
            if rr == 0. {
                rr = 1. / (i + 1) as f64;
            }
            dcg += 1. / ((i + 2) as f64).log2();
        }
    }
    let ideal = (0..relevant.len().min(10))
        .map(|i| 1. / ((i + 2) as f64).log2())
        .sum::<f64>();
    json!({"key":q.key,"language":q.language,"hits":hit,"returned":ids.len(),"relevant":relevant.len(),"recall10":if relevant.is_empty(){0.}else{hit as f64/relevant.len()as f64},"mrr10":rr,"ndcg10":if ideal==0.{0.}else{dcg/ideal},"judged_at10":judged,"known_negative_at10":negative,"ids":ids})
}
fn aggregate(ss: &[Value]) -> Value {
    let avg = |k: &str| ss.iter().map(|s| s[k].as_f64().unwrap()).sum::<f64>() / ss.len() as f64;
    json!({"queries":ss.len(),"recall10":avg("recall10"),"mrr10":avg("mrr10"),"ndcg10":avg("ndcg10"),"known_negative_per_query":avg("known_negative_at10"),"judged_per_query":avg("judged_at10")})
}
fn objects(c: &Connection) -> Value {
    json!(
        c.prepare("select name,sum(pgsize) from dbstat group by name order by name")
            .unwrap()
            .query_map([], |r| Ok(
                json!({"name":r.get::<_,String>(0)?,"bytes":r.get::<_,i64>(1)?})
            ))
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
    )
}
fn run(engine: &str, mode: &str, file: &str, scale: usize) {
    let idle = memory();
    let mut corpus: Corpus = serde_json::from_reader(std::fs::File::open(file).unwrap()).unwrap();
    if let Ok(limit) = std::env::var("BENCH_QUERY_LIMIT") {
        corpus.queries.truncate(limit.parse().unwrap());
    }
    let loaded = memory();
    let db = std::env::var("BENCH_DB").unwrap();
    let c = open(&db, if engine == "fts" { mode } else { "unicode" });
    schema(&c);
    let docs: Vec<_> = if scale == 0 {
        corpus.docs
    } else {
        (0..scale)
            .map(|i| {
                let mut d = corpus.docs[i % corpus.docs.len()].clone();
                d.id = i as u64 + 1;
                d.key = format!("scale-{i}");
                d
            })
            .collect()
    };
    let seed = Instant::now();
    for ds in docs.chunks(500) {
        insert(&c, ds);
    }
    let seed_us = us(seed);
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    let source_objects = objects(&c);
    let source_bytes = bytes(&db);
    let base = memory();
    let cpu0 = cpu();
    let t = Instant::now();
    #[cfg(feature = "text")]
    let mut external = None;
    let dir = format!("{db}.tantivy");
    if engine == "fts" {
        c.execute_batch("begin immediate").unwrap();
        create_index(&c, mode);
        c.execute_batch("insert into search(rowid,title,body)select id,title,body from documents order by id;commit;").unwrap();
    } else {
        #[cfg(feature = "text")]
        {
            let mut e = crate::tant::TextIndex::new(&dir, mode);
            for d in &docs {
                e.add(d);
            }
            e.commit();
            external = Some(e);
        }
        #[cfg(not(feature = "text"))]
        panic!("text feature required");
    }
    let build_us = us(t);
    let build_memory = memory();
    let build_external_bytes = if engine == "fts" {
        0
    } else {
        directory(Path::new(&dir))
    };
    let build_wal = bytes(&format!("{db}-wal"));
    let build_objects = objects(&c);
    let mut times = vec![];
    let mut samples = vec![];
    let mut set_checks = vec![];
    let begin_queries = Instant::now();
    for q in &corpus.queries {
        let t = Instant::now();
        let ids = if engine == "fts" {
            search(&c, &q.text, mode, false)
        } else {
            #[cfg(feature = "text")]
            {
                external.as_ref().unwrap().search(&q.text, false)
            }
            #[cfg(not(feature = "text"))]
            {
                unreachable!()
            }
        };
        times.push(us(t));
        samples.push(scores(q, &ids));
    }
    let query_us = us(begin_queries);
    let query_memory = memory();
    for q in corpus.queries.iter().take(12) {
        let ids = if engine == "fts" {
            search(&c, &q.text, mode, true)
        } else {
            #[cfg(feature = "text")]
            {
                external.as_ref().unwrap().search(&q.text, true)
            }
            #[cfg(not(feature = "text"))]
            {
                unreachable!()
            }
        };
        let h = ids.iter().fold(0xcbf29ce484222325u64, |h, x| {
            (h ^ x).wrapping_mul(0x100000001b3)
        });
        set_checks.push(json!({"key":q.key,"count":ids.len(),"hash":h}));
    }
    let query_cpu = cpu() - cpu0;
    let mut reranked = vec![];
    if scale == 0 {
        for q in &corpus.queries {
            let filter = q
                .judged
                .keys()
                .map(|i| i.parse().unwrap())
                .collect::<Vec<u64>>();
            let ids = if engine == "fts" {
                search_filtered(&c, &q.text, mode, false, Some(filter))
            } else {
                #[cfg(feature = "text")]
                {
                    external
                        .as_ref()
                        .unwrap()
                        .search_filtered(&q.text, false, Some(filter))
                }
                #[cfg(not(feature = "text"))]
                {
                    unreachable!()
                }
            };
            reranked.push(scores(q, &ids));
        }
    }
    // Same 100 source updates/commit; external native commit is explicitly separate.
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    let mut write_times = vec![];
    let mut mutation = vec![];
    for op in ["insert", "update", "unrelated", "delete"] {
        let mut ws = vec![];
        let mut ext_us = 0;
        for batch in 0..10 {
            let t = Instant::now();
            c.execute_batch("begin immediate").unwrap();
            for i in 0..100 {
                let original = &docs[(batch * 100 + i) % docs.len()];
                let mut d = original.clone();
                d.id = docs.len() as u64 + 1 + (batch * 100 + i) as u64;
                d.key = format!("mutation-{}", d.id);
                match op {
                    "insert" => {
                        c.execute(
                            "insert into documents(id,key,title,body)values(?1,?2,?3,?4)",
                            params![d.id as i64, d.key, d.title, d.body],
                        )
                        .unwrap();
                    }
                    "update" => {
                        c.execute(
                            "update documents set title=title||' revised' where id=?1",
                            [d.id as i64],
                        )
                        .unwrap();
                        d.title.push_str(" revised");
                    }
                    "unrelated" => {
                        c.execute(
                            "update documents set other=other+1 where id=?1",
                            [d.id as i64],
                        )
                        .unwrap();
                    }
                    _ => {
                        c.execute("delete from documents where id=?1", [d.id as i64])
                            .unwrap();
                    }
                }
                #[cfg(feature = "text")]
                if let Some(e) = &mut external {
                    if op != "unrelated" {
                        if op != "insert" {
                            e.delete(d.id);
                        }
                        if op != "delete" {
                            e.add(&d);
                        }
                    }
                }
            }
            c.execute_batch("commit").unwrap();
            let sql_us = us(t);
            let te = Instant::now();
            #[cfg(feature = "text")]
            if let Some(e) = &mut external {
                if op != "unrelated" {
                    e.commit();
                }
            }
            ext_us += us(te);
            ws.push(sql_us);
            write_times.push(sql_us);
        }
        mutation.push(json!({"operation":op,"transactions":10,"rows_per_transaction":100,"source_commit":distribution(&mut ws),"external_commit_reload_us":ext_us,"wal_file_bytes":bytes(&format!("{db}-wal"))}));
        c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    }
    let reopen = Instant::now();
    if engine == "fts" {
        c.execute(
            "insert into search(search,rank)values('integrity-check',1)",
            [],
        )
        .unwrap();
        let r = open(&db, mode);
        let q = &corpus.queries[0].text;
        assert_eq!(search(&c, q, mode, true), search(&r, q, mode, true));
    } else {
        #[cfg(feature = "text")]
        external.as_mut().unwrap().reopen(&dir);
    }
    let reopen_us = us(reopen);
    let cpu_s = cpu() - cpu0;
    println!(
        "{}",
        json!({"engine":engine,"mode":mode,"rows":docs.len(),"source_seed_us":seed_us,"source_bytes":source_bytes,"source_objects":source_objects,"build_us":build_us,"build_rows_per_s":docs.len()as f64*1e6/build_us as f64,"build_wal_file_bytes":build_wal,"build_external_bytes":build_external_bytes,"build_objects":build_objects,"external_bytes":if engine=="fts"{0}else{directory(Path::new(&dir))},"queries":distribution(&mut times),"query_per_s":corpus.queries.len()as f64*1e6/query_us as f64,"quality":aggregate(&samples),"query_samples":samples,"judged_pool_quality":if reranked.is_empty(){json!(null)}else{aggregate(&reranked)},"judged_pool_samples":reranked,"set_checks":set_checks,"mutations":mutation,"reopen_and_integrity_us":reopen_us,"idle_memory":idle,"loaded_memory":loaded,"source_memory":base,"build_memory":build_memory,"query_memory":query_memory,"end_memory":memory(),"cpu_through_queries_s":query_cpu,"cpu_total_s":cpu_s,"status":"pass"})
    );
}
fn analyze(mode: &str, file: &str) {
    let c: Corpus = serde_json::from_reader(std::fs::File::open(file).unwrap()).unwrap();
    let idle = memory();
    let setup = Instant::now();
    let a = Analyzer::new(mode);
    let setup_us = us(setup);
    let ready = memory();
    let mut tokens = 0;
    let mut bytes = 0;
    let mut checksum = 0;
    let cpu0 = cpu();
    let t = Instant::now();
    for d in c.docs.iter().step_by((c.docs.len() / 5000).max(1)) {
        for text in [&d.title, &d.body] {
            let ts = a.tokens(text);
            tokens += ts.len();
            bytes += text.len();
            for x in ts {
                checksum ^= x.text.len();
            }
        }
    }
    let elapsed = us(t);
    let probes = [
        "Кошки спят на диване",
        "Developers are running tests",
        "我喜欢数据库系统",
        "SQLite базы 数据库 running",
        "Die Häuser stehen am Fluss",
        "Les chevaux courent",
        "реплицировать базу",
        "こんにちは世界",
        "ทุกสองสัปดาห์",
        "Straße STRASSE",
        "İSTANBUL",
    ];
    println!(
        "{}",
        json!({"mode":mode,"sampling":"all fields of every floor(N/5000)-th document","setup_us":setup_us,"bytes":bytes,"tokens":tokens,"us":elapsed,"mib_per_s":bytes as f64*1e6/elapsed as f64/1048576.,"checksum":checksum,"cpu_s":cpu()-cpu0,"idle_memory":idle,"ready_memory":ready,"end_memory":memory(),"probes":probes.into_iter().map(|s|json!({"text":s,"tokens":a.tokens(s).into_iter().map(|t|json!({"text":t.text,"start":t.start,"end":t.end,"position":t.position})).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    );
}
fn check() {
    let db = std::env::var("BENCH_DB").unwrap();
    let c = open(&db, "auto");
    schema(&c);
    create_index(&c, "auto");
    let mut d = Doc {
        id: 1,
        key: "external/string/42".into(),
        title: "Кошки Developers".into(),
        body: "running tests 我喜欢数据库系统".into(),
    };
    insert(&c, &[d.clone()]);
    assert_eq!(search(&c, "кошка", "auto", true), vec![1]);
    assert_eq!(search(&c, "run test", "auto", true), vec![1]);
    assert_eq!(search(&c, "数据库", "auto", true), vec![1]);
    let initial = search(&c, "кошка", "auto", true);
    c.execute_batch(
        "begin;update documents set title='Собаки',body='different text' where id=1;rollback;",
    )
    .unwrap();
    assert_eq!(search(&c, "кошка", "auto", true), initial);
    c.execute_batch(
        "begin;update documents set id=-7,title='Собаки',body='updated text' where id=1;commit;",
    )
    .unwrap();
    assert!(search(&c, "кошка", "auto", true).is_empty());
    // Read the signed SQL identity directly; the Search collection maps keys to positive ids.
    let signed: i64 = c
        .query_row(
            "select rowid from search where search match 'собака'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(signed, -7);
    c.execute_batch("update documents set id=1 where id=-7;")
        .unwrap();
    assert_eq!(search(&c, "собака", "auto", true), vec![1]);
    c.execute_batch("delete from documents where id=1;")
        .unwrap();
    assert!(search(&c, "собака", "auto", true).is_empty());
    d.title = "ＡＢＣ Cafe\u{301} ёлки".into();
    d.body = "SQLite".into();
    insert(&c, &[d]);
    let snippet: String = c
        .query_row(
            "select highlight(search,0,'[',']') from search where search match 'abc'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(snippet.contains("[ＡＢＣ]"));
    c.execute(
        "insert into search(search,rank)values('integrity-check',1)",
        [],
    )
    .unwrap();
    let r = open(&db, "auto");
    assert_eq!(search(&r, "елка", "auto", true), vec![1]);
    let bare = Connection::open(&db).unwrap();
    let missing = bare
        .query_row(
            "select count(*) from search where search match 'SQLite'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .err()
        .map(|e| e.to_string());
    assert!(missing.is_some());
    println!(
        "{}",
        json!({"status":"pass","checks":["actual C tokenizer callbacks","RU/EN forms","Chinese segmentation","source/FTS rollback","OLD-token delete/update","signed id moves via direct SQL","string identity mapping","NFKC highlight original UTF8 offsets","rank-1 external-content integrity","reopen with registration","missing per-connection registration fails"],"highlight":snippet,"missing_registration":missing})
    );
}
fn concurrent(file: &str, readers: usize) {
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicBool, Ordering},
    };
    let corpus: Corpus = serde_json::from_reader(std::fs::File::open(file).unwrap()).unwrap();
    let db = std::env::var("BENCH_DB").unwrap();
    let c = open(&db, "auto");
    schema(&c);
    create_index(&c, "auto");
    let build = Instant::now();
    for batch in 0..200 {
        let ds: Vec<_> = (0..500)
            .map(|j| {
                let i = batch * 500 + j;
                let mut d = corpus.docs[i % corpus.docs.len()].clone();
                d.id = i as u64 + 1;
                d.key = format!("{i}");
                d
            })
            .collect();
        insert(&c, &ds);
    }
    let ingest_us = us(build);
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    let baseline = memory();
    let start = Arc::new(Barrier::new(readers + 1));
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles = vec![];
    let queries = Arc::new(
        corpus
            .queries
            .into_iter()
            .map(|q| q.text)
            .collect::<Vec<_>>(),
    );
    for _ in 0..readers {
        let db = db.clone();
        let start = start.clone();
        let stop = stop.clone();
        let queries = queries.clone();
        handles.push(std::thread::spawn(move || {
            let r = open(&db, "auto");
            let mut ts = vec![];
            let mut i = 0;
            start.wait();
            while !stop.load(Ordering::Relaxed) {
                let t = Instant::now();
                let _ids = search(&r, &queries[i % queries.len()], "auto", false);
                ts.push(us(t));
                i += 1;
            }
            distribution(&mut ts)
        }));
    }
    let cpu0 = cpu();
    start.wait();
    let origin = Instant::now();
    let mut service = vec![];
    let mut scheduled = vec![];
    let mut rollbacks = 0;
    for i in 0..1000 {
        let target = origin + std::time::Duration::from_micros(i * 1000);
        std::thread::sleep(target.saturating_duration_since(Instant::now()));
        let t = Instant::now();
        c.execute_batch("begin immediate").unwrap();
        if i % 10 == 0 {
            c.execute(
                "update documents set title=title||' modified' where id=?1",
                [i as i64 + 1],
            )
            .unwrap();
        } else {
            c.execute(
                "update documents set other=other+1 where id=?1",
                [i as i64 + 1],
            )
            .unwrap();
        }
        if i % 33 == 0 {
            c.execute_batch("rollback").unwrap();
            rollbacks += 1;
        } else {
            c.execute_batch("commit").unwrap();
        }
        service.push(us(t));
        scheduled.push(target.elapsed().as_micros() as u64);
    }
    stop.store(true, Ordering::Relaxed);
    let samples: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let duration = origin.elapsed().as_secs_f64();
    let cpu_s = cpu() - cpu0;
    let work_memory = memory();
    let wal = bytes(&format!("{db}-wal"));
    let integrity = Instant::now();
    c.execute(
        "insert into search(search,rank)values('integrity-check',1)",
        [],
    )
    .unwrap();
    println!(
        "{}",
        json!({"rows":100000,"readers":readers,"initial_indexed_ingest_us":ingest_us,"writer_transactions":1000,"indexed_update_fraction":0.1,"rollback_transactions":rollbacks,"writer_service":distribution(&mut service),"writer_scheduled":distribution(&mut scheduled),"reader_samples":samples,"duration_s":duration,"cpu_s":cpu_s,"baseline_memory":baseline,"work_memory":work_memory,"wal_file_bytes":wal,"post_workload_rank1_integrity_us":us(integrity),"status":"pass"})
    );
}
pub fn main() {
    let a: Vec<_> = std::env::args().skip(1).collect();
    match a[0].as_str() {
        "run" => run(
            &a[1],
            &a[2],
            &a[3],
            a.get(4).map(|s| s.parse().unwrap()).unwrap_or(0),
        ),
        "analyze" => analyze(&a[1], &a[2]),
        "check" => check(),
        "concurrent" => concurrent(&a[1], a[2].parse().unwrap()),
        _ => panic!("run ENGINE MODE CORPUS [SCALE] | analyze MODE CORPUS | check"),
    }
}
