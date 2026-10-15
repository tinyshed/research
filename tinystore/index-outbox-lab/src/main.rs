#![recursion_limit = "512"]
#[path = "../../index-foundations/src/common.rs"]
mod common;
#[path = "../../index-foundations/src/fixture.rs"]
mod fixture;
#[path = "../../index-foundations/src/native.rs"]
mod native;
use common::*;
use rusqlite::{Connection, params};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tantivy::{
    Index, IndexReader, IndexWriter, Searcher, Term, collector::DocSetCollector, doc,
    query::TermQuery, schema::*, tokenizer::WhitespaceTokenizer,
};
const OUTBOX:&str="create table pending(seq integer primary key autoincrement,id integer not null);
create table progress(id integer primary key,revision integer);insert into progress values(0,0);
create trigger pending_i after insert on documents begin insert into pending(id) values(new.id);end;
create trigger pending_d after delete on documents begin insert into pending(id) values(old.id);end;
create trigger pending_u after update on documents when old.id is not new.id or old.terms is not new.terms begin
insert into pending(id) values(old.id);insert into pending(id) select new.id where old.id is not new.id;end;";
struct External {
    index: Index,
    writer: IndexWriter,
    reader: IndexReader,
    id: Field,
    text: Field,
    revision: i64,
}
fn io() -> serde_json::Value {
    let text = std::fs::read_to_string("/proc/self/io").unwrap();
    let mut m = serde_json::Map::new();
    for line in text.lines() {
        let (k, v) = line.split_once(':').unwrap();
        m.insert(k.to_owned(), json!(v.trim().parse::<u64>().unwrap()));
    }
    json!(m)
}
fn directory() -> String {
    format!("{}.tantivy", path())
}
fn dir_size(p: &std::path::Path) -> u64 {
    std::fs::read_dir(p)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            if p.is_dir() {
                dir_size(&p)
            } else {
                bytes(p.to_str().unwrap())
            }
        })
        .sum()
}
fn revision(c: &Connection) -> i64 {
    c.query_row(
        "select coalesce((select seq from sqlite_sequence where name='pending'),0)",
        [],
        |r| r.get(0),
    )
    .unwrap()
}
impl External {
    fn open(new: bool) -> Self {
        let index = if new {
            std::fs::create_dir(directory()).unwrap();
            let mut b = Schema::builder();
            b.add_u64_field("id", INDEXED | FAST);
            b.add_text_field(
                "terms",
                TextOptions::default().set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("shared")
                        .set_index_option(IndexRecordOption::Basic),
                ),
            );
            Index::create_in_dir(directory(), b.build()).unwrap()
        } else {
            Index::open_in_dir(directory()).unwrap()
        };
        index
            .tokenizers()
            .register("shared", WhitespaceTokenizer::default());
        let schema = index.schema();
        let id = schema.get_field("id").unwrap();
        let text = schema.get_field("terms").unwrap();
        let writer = index.writer_with_num_threads(1, 32_000_000).unwrap();
        let reader = index.reader().unwrap();
        let revision = index
            .load_metas()
            .unwrap()
            .payload
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        Self {
            index,
            writer,
            reader,
            id,
            text,
            revision,
        }
    }
    fn add(&mut self, d: &fixture::Doc) {
        self.writer
            .add_document(doc!(self.id=>d.id as u64,self.text=>d.terms.as_str()))
            .unwrap();
    }
    fn commit(&mut self, rev: i64) {
        let mut p = self.writer.prepare_commit().unwrap();
        p.set_payload(&rev.to_string());
        p.commit().unwrap();
        self.reader.reload().unwrap();
        self.revision = rev;
    }
    fn build(&mut self, snapshot: &Connection) {
        let high = revision(snapshot);
        let mut cursor = None;
        loop {
            let docs = fixture::read(snapshot, cursor, i64::MAX, 1000);
            if docs.is_empty() {
                break;
            }
            for d in &docs {
                self.add(d);
            }
            cursor = Some(docs.last().unwrap().id);
        }
        self.commit(high);
    }
    fn replay(&mut self, snapshot: &Connection, commit: bool) -> usize {
        let high = revision(snapshot);
        assert!(high >= self.revision);
        if high == self.revision {
            return 0;
        }
        let ids: Vec<i64> = snapshot
            .prepare("select distinct id from pending where seq>?1 and seq<=?2 order by id")
            .unwrap()
            .query_map(params![self.revision, high], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for id in &ids {
            self.writer
                .delete_term(Term::from_field_u64(self.id, *id as u64));
            let doc = fixture::read(snapshot, id.checked_sub(1), *id, 1);
            if let Some(d) = doc.first() {
                self.add(d);
            }
        }
        if commit {
            self.commit(high);
        }
        ids.len()
    }
    fn ack(&self, c: &Connection, cleanup: bool) {
        c.execute_batch("begin immediate").unwrap();
        c.execute(
            "update progress set revision=?1 where id=0",
            [self.revision],
        )
        .unwrap();
        if cleanup {
            c.execute("delete from pending where seq<=?1", [self.revision])
                .unwrap();
        }
        c.execute_batch("commit").unwrap();
    }
    fn search(&self, s: &Searcher, q: usize) -> Vec<i64> {
        let term = Term::from_field_text(self.text, &native::query_term(q, "russian"));
        let found = s
            .search(
                &TermQuery::new(term, IndexRecordOption::Basic),
                &DocSetCollector,
            )
            .unwrap();
        let cols: Vec<_> = s
            .segment_readers()
            .iter()
            .map(|s| s.fast_fields().u64("id").unwrap())
            .collect();
        let mut ids: Vec<i64> = found
            .iter()
            .map(|a| cols[a.segment_ord as usize].first(a.doc_id).unwrap() as i64)
            .collect();
        ids.sort_unstable();
        ids
    }
}
fn changes(c: &Connection, phase: u64, seq: u64, n: usize) {
    let id = (fixture::mix(seq + phase * 10000) % n as u64 + 1) as i64;
    match seq % 10 {
        0 => fixture::insert(
            c,
            &fixture::Fixture::new("uniform")
                .doc(n as i64 + phase as i64 * 100000 + seq as i64 + 1),
        ),
        1 => {
            c.execute("delete from documents where id=?1", [id])
                .unwrap();
        }
        2 => {
            c.execute(
                "update documents set id=?2 where id=?1",
                params![id, n as i64 * 100 + phase as i64 * 100000 + seq as i64 + 1],
            )
            .unwrap();
        }
        _ => {
            c.execute(
                "update documents set terms=terms||' changed',other=other+1 where id=?1",
                [id],
            )
            .unwrap();
        }
    }
}
fn foreground(
    w: Arc<Mutex<Connection>>,
    phase: u64,
    count: usize,
    n: usize,
    delay: Duration,
) -> std::thread::JoinHandle<serde_json::Value> {
    std::thread::spawn(move || {
        let origin = Instant::now() + delay;
        let mut queue = vec![];
        let mut service = vec![];
        let mut scheduled = vec![];
        for seq in 0..count {
            let target = origin + Duration::from_millis(seq as u64);
            std::thread::sleep(target.saturating_duration_since(Instant::now()));
            let asked = Instant::now();
            let c = w.lock().unwrap();
            queue.push(us(asked));
            let held = Instant::now();
            c.execute_batch("begin immediate").unwrap();
            changes(&c, phase, seq as u64, n);
            c.execute_batch("commit").unwrap();
            maybe_checkpoint(&c);
            service.push(us(held));
            scheduled.push(target.elapsed().as_micros() as u64);
        }
        json!({"transactions":count,"queue":distribution(&mut queue),"service":distribution(&mut service),"scheduled":distribution(&mut scheduled)})
    })
}
fn validate_external(c: &Connection, e: &External) -> serde_json::Value {
    c.execute_batch("begin immediate").unwrap();
    native::create(c, 9999, "fts-set");
    native::build(c, 9999, "fts-set", 1);
    c.execute_batch("commit").unwrap();
    let s = e.reader.searcher();
    let total: i64 = c
        .query_row("select count(*) from documents", [], |r| r.get(0))
        .unwrap();
    assert_eq!(s.num_docs(), total as u64);
    for q in 0..60 {
        assert_eq!(
            e.search(&s, q),
            native::query(c, 9999, "fts-set", q, "russian")
        );
    }
    validate(c);
    json!({"equal_queries":60,"documents":total,"revision":e.revision,"status":"pass"})
}
fn run(a: &[String]) {
    let mode = &a[0];
    let n: usize = a[1].parse().unwrap();
    let c = open(&path());
    c.busy_timeout(Duration::from_secs(60)).unwrap();
    if !a.get(2).is_some_and(|s| s == "prepared") {
        fixture::seed(&c, n, "russian", "ordered", 1000);
    }
    if mode == "outbox" {
        c.execute_batch(OUTBOX).unwrap();
    }
    checkpoint(&c);
    observe_wal(&c);
    let idle = memory();
    let io_before = io();
    let cpu0 = cpu();
    let w = Arc::new(Mutex::new(c));
    let read = open(&path());
    let build_start = Instant::now();
    let f = foreground(w.clone(), 1, 1000, n, Duration::from_millis(100));
    let mut external = None;
    if mode == "native" {
        native::initialize_active(&w.lock().unwrap(), "fts-set", true);
    } else {
        read.execute_batch("begin").unwrap();
        let mut e = External::open(true);
        e.build(&read);
        read.execute_batch("rollback").unwrap();
        external = Some(e);
    }
    let build_us = us(build_start);
    let build_foreground = f.join().unwrap();
    let t = Instant::now();
    let mut catchup_rows = 0;
    if let Some(e) = &mut external {
        read.execute_batch("begin").unwrap();
        catchup_rows = e.replay(&read, true);
        read.execute_batch("rollback").unwrap();
        e.ack(&w.lock().unwrap(), true);
    }
    let catchup_us = us(t);
    let build_memory = memory();
    let phase_start = Instant::now();
    let f = foreground(w.clone(), 2, 2000, n, Duration::ZERO);
    let mut queries = vec![];
    let mut scheduled = vec![];
    let mut barriers = vec![];
    let mut checksum = 0u64;
    for q in 0..200 {
        let target = phase_start + Duration::from_millis(q as u64 * 10);
        std::thread::sleep(target.saturating_duration_since(Instant::now()));
        let t = Instant::now();
        read.execute_batch("begin").unwrap();
        let ids = if let Some(e) = &mut external {
            let changes = e.replay(&read, true);
            barriers.push(changes);
            let s = e.reader.searcher();
            let ids = e.search(&s, q);
            read.execute_batch("rollback").unwrap();
            if changes > 0 {
                e.ack(&w.lock().unwrap(), true);
            }
            ids
        } else {
            let ids = native::query(&read, 0, "fts-set", q, "russian");
            read.execute_batch("rollback").unwrap();
            ids
        };
        checksum = checksum.wrapping_mul(0x100000001b3) ^ hash_ids(&ids);
        queries.push(us(t));
        scheduled.push(target.elapsed().as_micros() as u64);
    }
    let foreground = f.join().unwrap();
    let duration_s = phase_start.elapsed().as_secs_f64();
    if let Some(e) = &mut external {
        read.execute_batch("begin").unwrap();
        e.replay(&read, true);
        read.execute_batch("rollback").unwrap();
        e.ack(&w.lock().unwrap(), true);
    }
    let c = w.lock().unwrap();
    let workload_memory = memory();
    let io_after = io();
    let workload_cpu = cpu() - cpu0;
    let wal = wal_stats();
    let obj = objects(&c);
    let db_bytes = bytes(&path());
    let wal_file = wal_bytes(&path());
    let index_bytes = if external.is_some() {
        dir_size(std::path::Path::new(&directory()))
    } else {
        0
    };
    let validation = if let Some(e) = &external {
        validate_external(&c, e)
    } else {
        validate(&c);
        json!({"integrity":"ok"})
    };
    println!(
        "{}",
        json!({"experiment":"outbox","mode":mode,"rows":n,"build_us":build_us,"build_foreground":build_foreground,"initial_catchup_us":catchup_us,"initial_dirty_ids":catchup_rows,"steady_foreground":foreground,"query_service":distribution(&mut queries),"query_scheduled":distribution(&mut scheduled),"query_count":200,"phase_duration_s":duration_s,"query_per_s":200./duration_s,"barrier_dirty_ids":barriers,"query_checksum_changes_over_snapshots":checksum,"idle_memory":idle,"build_memory":build_memory,"workload_memory":workload_memory,"cpu_s":workload_cpu,"process_io_before":io_before,"process_io_after":io_after,"wal":wal,"objects":obj,"db_bytes":db_bytes,"wal_file_bytes":wal_file,"external_bytes":index_bytes,"validation":validation,"status":"pass"})
    );
}
fn crash(stage: &str) {
    use std::io::Write;
    let c = open(&path());
    fixture::seed(&c, 300, "russian", "ordered", 100);
    c.execute_batch(OUTBOX).unwrap();
    let mut e = External::open(true);
    c.execute_batch("begin").unwrap();
    e.build(&c);
    c.execute_batch("rollback").unwrap();
    e.ack(&c, true);
    c.execute_batch("begin immediate;update documents set terms='changed человек' where id=1;delete from documents where id=2;update documents set id=5000 where id=3;commit").unwrap();
    match stage {
        "source-only" => {}
        "external-uncommitted" => {
            c.execute_batch("begin").unwrap();
            e.replay(&c, false);
        }
        "external-before-ack" => {
            c.execute_batch("begin").unwrap();
            e.replay(&c, true);
            c.execute_batch("rollback").unwrap();
        }
        "ack-before-cleanup" | "cleanup-committed" => {
            c.execute_batch("begin").unwrap();
            e.replay(&c, true);
            c.execute_batch("rollback").unwrap();
            e.ack(&c, stage == "cleanup-committed");
        }
        _ => panic!("unknown crash stage"),
    }
    println!("{}", json!({"ready":true,"stage":stage}));
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}
fn recover() {
    let t = Instant::now();
    let c = open(&path());
    let mut e = External::open(false);
    let external_before = e.revision;
    let sql_before: i64 = c
        .query_row("select revision from progress", [], |r| r.get(0))
        .unwrap();
    c.execute_batch("begin").unwrap();
    let dirty = e.replay(&c, true);
    c.execute_batch("rollback").unwrap();
    e.ack(&c, true);
    let recovery_us = us(t);
    let validation = validate_external(&c, &e);
    println!(
        "{}",
        json!({"experiment":"outbox-recovery","external_before":external_before,"sql_before":sql_before,"replayed_ids":dirty,"recovery_us":recovery_us,"validation":validation,"status":"pass"})
    );
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a[0].as_str() {
        "run" => run(&a[1..]),
        "crash" => crash(&a[1]),
        "recover" => recover(),
        _ => panic!("unknown mode"),
    }
}
