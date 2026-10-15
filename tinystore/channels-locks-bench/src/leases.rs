use crate::{cpu_seconds, memory, percentile, us};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Notify, mpsc, oneshot};

const SCHEMA: &str = "
create table resources (name integer primary key, token integer not null);
create table leases (name integer, slot integer, owner integer, until integer, token integer,
    primary key(name,slot)) without rowid;
create table kv (name integer primary key, value text not null);";

const ACQUIRE: &str =
    "update leases set owner=?2, until=?4, token=(select token+1 from resources where name=?1)
where name=?1 and slot=(select slot from leases where name=?1 and until<=?3 order by slot limit 1)
and until<=?3 returning slot,token";
const RELEASE: &str =
    "update leases set owner=0,until=0 where name=?1 and slot=?2 and owner=?3 and token=?4";
const RENEW: &str = "update leases set until=max(until,?6)
where name=?1 and slot=?2 and owner=?3 and token=?4 and until>?5";

fn open(path: &str, durability: &str) -> Connection {
    let c = Connection::open(path).unwrap();
    c.execute_batch(&format!(
        "pragma journal_mode=wal; pragma synchronous={durability};
        pragma wal_autocheckpoint=0; pragma cache_size=-2048; pragma busy_timeout=5000;"
    ))
    .unwrap();
    c
}

fn seed(c: &Connection, keys: usize, permits: usize) {
    c.execute_batch(SCHEMA).unwrap();
    c.execute_batch("begin immediate").unwrap();
    for name in 0..keys {
        c.execute("insert into resources values (?1,0)", [name as i64])
            .unwrap();
        let mut slots = vec![];
        for slot in 0..permits {
            c.execute(
                "insert into leases values (?1,?2,0,0,0)",
                params![name as i64, slot as i64],
            )
            .unwrap();
            slots.push(json!({"owner":0,"until":0,"token":0}));
        }
        let v = json!({"token":0,"slots":slots}).to_string();
        c.execute("insert into kv values (?1,?2)", params![name as i64, v])
            .unwrap();
    }
    c.execute_batch("commit; pragma wal_checkpoint(truncate)")
        .unwrap();
}

#[derive(Clone, Copy, Debug)]
struct Lease {
    slot: i64,
    token: i64,
}
enum Op {
    Acquire,
    Release(Lease),
    Renew(Lease),
}
struct Request {
    key: i64,
    owner: i64,
    op: Op,
    done: oneshot::Sender<Option<Lease>>,
}
#[derive(Default)]
struct WriterStats {
    requests: u64,
    acquire_attempts: u64,
    successes: u64,
    mutations: u64,
    commits: u64,
    max_batch: usize,
}

fn acquire(c: &Connection, key: i64, owner: i64, now: i64, until: i64) -> Option<Lease> {
    let lease = c
        .prepare_cached(ACQUIRE)
        .unwrap()
        .query_row(params![key, owner, now, until], |r| {
            Ok(Lease {
                slot: r.get(0)?,
                token: r.get(1)?,
            })
        })
        .optional()
        .unwrap();
    if lease.is_some() {
        c.prepare_cached("update resources set token=token+1 where name=?1")
            .unwrap()
            .execute([key])
            .unwrap();
    }
    lease
}

fn kv_apply(c: &Connection, r: &Request, now: i64, until: i64) -> (Option<Lease>, bool) {
    let text: String = c
        .prepare_cached("select value from kv where name=?1")
        .unwrap()
        .query_row([r.key], |row| row.get(0))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut changed = false;
    let answer = match r.op {
        Op::Acquire => {
            let slot = v["slots"]
                .as_array()
                .unwrap()
                .iter()
                .position(|s| s["until"].as_i64().unwrap() <= now);
            slot.map(|slot| {
                let token = v["token"].as_i64().unwrap() + 1;
                v["token"] = json!(token);
                v["slots"][slot] = json!({"owner":r.owner,"until":until,"token":token});
                changed = true;
                Lease {
                    slot: slot as i64,
                    token,
                }
            })
        }
        Op::Release(lease) | Op::Renew(lease) => {
            let s = &mut v["slots"][lease.slot as usize];
            if s["owner"] == r.owner && s["token"] == lease.token {
                if matches!(r.op, Op::Release(_)) {
                    s["owner"] = json!(0);
                    s["until"] = json!(0);
                    changed = true;
                } else if s["until"].as_i64().unwrap() > now {
                    s["until"] = json!(until.max(s["until"].as_i64().unwrap()));
                    changed = true;
                }
            }
            changed.then_some(lease)
        }
    };
    if changed {
        c.prepare_cached("update kv set value=?2 where name=?1")
            .unwrap()
            .execute(params![r.key, v.to_string()])
            .unwrap();
    }
    (answer, changed)
}

fn apply(
    c: &Connection,
    r: &Request,
    now: i64,
    ttl: i64,
    kv: bool,
    stats: &mut WriterStats,
) -> Option<Lease> {
    stats.requests += 1;
    if matches!(r.op, Op::Acquire) {
        stats.acquire_attempts += 1;
    }
    let answer = if kv {
        let (answer, changed) = kv_apply(c, r, now, now + ttl);
        stats.mutations += u64::from(changed);
        answer
    } else {
        match r.op {
            Op::Acquire => {
                let answer = acquire(c, r.key, r.owner, now, now + ttl);
                stats.mutations += if answer.is_some() { 2 } else { 0 };
                answer
            }
            Op::Release(l) => {
                let n = c
                    .prepare_cached(RELEASE)
                    .unwrap()
                    .execute(params![r.key, l.slot, r.owner, l.token])
                    .unwrap();
                stats.mutations += n as u64;
                (n == 1).then_some(l)
            }
            Op::Renew(l) => {
                let n = c
                    .prepare_cached(RENEW)
                    .unwrap()
                    .execute(params![r.key, l.slot, r.owner, l.token, now, now + ttl])
                    .unwrap();
                stats.mutations += n as u64;
                (n == 1).then_some(l)
            }
        }
    };
    if matches!(r.op, Op::Acquire) && answer.is_some() {
        stats.successes += 1;
    }
    answer
}

async fn call(tx: &mpsc::Sender<Request>, key: i64, owner: i64, op: Op) -> Option<Lease> {
    let (done, rx) = oneshot::channel();
    tx.send(Request {
        key,
        owner,
        op,
        done,
    })
    .await
    .unwrap();
    rx.await.unwrap()
}

struct ClientStats {
    cycles: u64,
    attempts: u64,
    waits: Vec<u64>,
    service: Vec<u64>,
    renewals: Vec<u64>,
}

pub fn run(a: &[String]) {
    let kind = a[0].clone();
    let clients: usize = a[1].parse().unwrap();
    let seconds: u64 = a[2].parse().unwrap();
    let topology = &a[3];
    let permits: usize = a[4].parse().unwrap();
    let durability = &a[5];
    let max_batch: usize = a[6].parse().unwrap();
    let renew = a.get(7).is_some_and(|s| s == "renew");
    let keys = if topology == "spread" { clients } else { 1 };
    let path = std::env::var("BENCH_DB").unwrap();
    let c = open(&path, durability);
    seed(&c, keys, permits);
    let initial = memory();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .unwrap();
    let origin = Instant::now();
    let generation = Arc::new(AtomicU64::new(0));
    let changed = Arc::new(Notify::new());
    let (tx, mut rx) = mpsc::channel::<Request>(2048);
    let kv = kind == "kv-poll";
    let (g, n) = (generation.clone(), changed.clone());
    let writer = std::thread::spawn(move || {
        let mut stats = WriterStats::default();
        let mut batch = vec![];
        while let Some(first) = rx.blocking_recv() {
            batch.push(first);
            while batch.len() < max_batch {
                match rx.try_recv() {
                    Ok(r) => batch.push(r),
                    Err(_) => break,
                }
            }
            stats.max_batch = stats.max_batch.max(batch.len());
            c.execute_batch("begin immediate").unwrap();
            let mut answers = vec![];
            let mut wakes = false;
            for r in batch.drain(..) {
                // TTL begins when the writer executes, not when the request entered a queue.
                let now = origin.elapsed().as_millis() as i64 + 1;
                let answer = apply(&c, &r, now, 1000, kv, &mut stats);
                wakes |= matches!(r.op, Op::Release(_)) && answer.is_some();
                answers.push((r.done, answer));
            }
            c.execute_batch("commit").unwrap();
            stats.commits += 1;
            for (done, answer) in answers {
                let _ = done.send(answer);
            }
            if wakes {
                g.fetch_add(1, Ordering::Release);
                n.notify_waiters();
            }
        }
        let objects: Vec<serde_json::Value> = c
            .prepare("select name,sum(pgsize) from dbstat group by name order by name")
            .unwrap()
            .query_map([], |r| {
                Ok(json!({"name":r.get::<_,String>(0)?,"bytes":r.get::<_,i64>(1)?}))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        (stats, objects, c)
    });
    let idle = memory();
    let cpu = cpu_seconds();
    let start = Instant::now();
    let results = rt.block_on(async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
        let gates: Vec<_> = (0..keys)
            .map(|_| Arc::new(tokio::sync::Semaphore::new(permits)))
            .collect();
        let mut handles = vec![];
        for id in 0..clients {
            let (tx, g, n) = (tx.clone(), generation.clone(), changed.clone());
            let hybrid = kind == "hybrid";
            let gate = (kind == "admitted").then(|| gates[if keys == 1 { 0 } else { id }].clone());
            handles.push(tokio::spawn(async move {
                let key = if keys == 1 { 0 } else { id as i64 };
                let mut stats = ClientStats {
                    cycles: 0,
                    attempts: 0,
                    waits: vec![],
                    service: vec![],
                    renewals: vec![],
                };
                while tokio::time::Instant::now() < deadline {
                    let waiting = Instant::now();
                    let _admission = if let Some(gate) = &gate {
                        match tokio::time::timeout_at(deadline, gate.clone().acquire_owned()).await
                        {
                            Ok(p) => Some(p.unwrap()),
                            Err(_) => break,
                        }
                    } else {
                        None
                    };
                    let lease = loop {
                        // Enable the wait before looking at availability. A timeout also covers owner death.
                        let notified = n.notified();
                        tokio::pin!(notified);
                        notified.as_mut().enable();
                        let seen = g.load(Ordering::Acquire);
                        let began = Instant::now();
                        let got = call(&tx, key, id as i64 + 1, Op::Acquire).await;
                        stats.service.push(us(began.elapsed()));
                        stats.attempts += 1;
                        if let Some(l) = got {
                            break Some(l);
                        }
                        if tokio::time::Instant::now() >= deadline {
                            break None;
                        }
                        if hybrid {
                            if seen == g.load(Ordering::Acquire) {
                                let _ =
                                    tokio::time::timeout(Duration::from_millis(10), notified).await;
                            }
                        } else {
                            tokio::time::sleep(Duration::from_millis(1)).await;
                        }
                    };
                    let Some(lease) = lease else {
                        break;
                    };
                    stats.waits.push(us(waiting.elapsed()));
                    if renew {
                        let began = Instant::now();
                        assert!(
                            call(&tx, key, id as i64 + 1, Op::Renew(lease))
                                .await
                                .is_some()
                        );
                        stats.renewals.push(us(began.elapsed()));
                    }
                    if keys == 1 {
                        tokio::time::sleep(Duration::from_millis(2)).await;
                    }
                    assert!(
                        call(&tx, key, id as i64 + 1, Op::Release(lease))
                            .await
                            .is_some()
                    );
                    stats.cycles += 1;
                }
                stats
            }));
        }
        let mut results = vec![];
        for h in handles {
            results.push(h.await.unwrap());
        }
        results
    });
    drop(tx);
    let (s, objects, c) = writer.join().unwrap();
    let elapsed = start.elapsed().as_secs_f64();
    let (mut waits, mut service) = (vec![], vec![]);
    let mut renewals = vec![];
    let mut completed = 0;
    let mut attempts = 0;
    let mut per_client = vec![];
    for r in results {
        completed += r.cycles;
        attempts += r.attempts;
        per_client.push(r.cycles);
        waits.extend(r.waits);
        service.extend(r.service);
        renewals.extend(r.renewals);
    }
    assert_eq!(s.successes, completed);
    assert_eq!(s.acquire_attempts, attempts);
    assert_eq!(
        c.query_row("pragma integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    let db_bytes = std::fs::metadata(&path).unwrap().len();
    let wal_bytes = std::fs::metadata(format!("{path}-wal"))
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "{}",
        json!({"experiment":"leases","kind":kind,"clients":clients,"topology":topology,
        "permits":permits,"durability":durability,"batch_limit":max_batch,"max_batch":s.max_batch,
        "duration_s":elapsed,"cycles":completed,"cycles_per_s":completed as f64/elapsed,
        "acquire_attempts":attempts,"attempts_per_success":attempts as f64/completed.max(1) as f64,
        "requests":s.requests,"mutations":s.mutations,"commits":s.commits,
        "wait_p50_us":percentile(&mut waits,0.5),"wait_p99_us":percentile(&mut waits,0.99),
        "service_p50_us":percentile(&mut service,0.5),"service_p99_us":percentile(&mut service,0.99),
        "renew_per_cycle":renew,"renewals":renewals.len(),
        "renew_p50_us":percentile(&mut renewals,0.5),"renew_p99_us":percentile(&mut renewals,0.99),
        "clients_with_zero_success":per_client.iter().filter(|n|**n==0).count(),
        "min_client_success":per_client.iter().min(),"max_client_success":per_client.iter().max(),
        "db_bytes":db_bytes,"wal_bytes":wal_bytes,"dbstat":objects,
        "cpu_s":cpu_seconds()-cpu,"initial_memory":initial,"idle_memory":idle,"final_memory":memory(),
        "sqlite":rusqlite::version(),"status":"pass"})
    );
}

pub fn check() {
    let path = std::env::var("BENCH_DB").unwrap();
    let c = open(&path, "FULL");
    seed(&c, 2, 3);
    let old = acquire(&c, 0, 1, 100, 110).unwrap();
    assert!(
        c.execute(RENEW, params![0, old.slot, 1, old.token, 110, 120])
            .unwrap()
            == 0
    );
    let next = acquire(&c, 0, 2, 110, 130).unwrap();
    assert!(next.token > old.token);
    assert_eq!(
        c.execute(RELEASE, params![0, old.slot, 1, old.token])
            .unwrap(),
        0
    );
    assert_eq!(
        c.execute(RENEW, params![0, old.slot, 1, old.token, 111, 140])
            .unwrap(),
        0
    );
    assert_eq!(
        c.execute(RENEW, params![0, next.slot, 2, next.token, 111, 125])
            .unwrap(),
        1
    );
    let until: i64 = c
        .query_row(
            "select until from leases where name=0 and slot=?1",
            [next.slot],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(until, 130);
    assert_eq!(
        c.execute(RENEW, params![0, next.slot, 2, next.token, 112, 150])
            .unwrap(),
        1
    );
    let mut slots = HashSet::new();
    for owner in 10..13 {
        assert!(slots.insert(acquire(&c, 1, owner, 100, 120).unwrap().slot));
    }
    assert!(acquire(&c, 1, 13, 100, 120).is_none());
    let stolen = acquire(&c, 1, 13, 120, 140).unwrap();
    assert_eq!(stolen.token, 4);
    // Delete/recreate of a KV key must not reset fencing state.
    let token_before = next.token;
    c.execute(RELEASE, params![0, next.slot, 2, next.token])
        .unwrap();
    drop(c);
    let reopened = open(&path, "FULL");
    let after = acquire(&reopened, 0, 3, 200, 250).unwrap();
    assert!(after.token > token_before);
    println!(
        "{}",
        json!({"experiment":"lease-check","status":"pass","checks":[
        "expiry boundary", "expired owner cannot renew before takeover", "stale release rejected",
        "stale renewal rejected", "renew does not shorten TTL", "renew extends live lease",
        "three permits only", "expired semaphore slot reclaimed", "fence persists after reopen"]})
    );
}

pub fn commit_check() {
    let path = std::env::var("BENCH_DB").unwrap();
    let c = open(&path, "FULL");
    c.execute_batch("create table events(n integer primary key); begin immediate")
        .unwrap();
    let mut pending = vec![];
    c.execute("insert into events values (1)", []).unwrap();
    pending.push(1);
    c.execute_batch("savepoint call").unwrap();
    let mark = pending.len();
    c.execute("insert into events values (2)", []).unwrap();
    pending.push(2);
    c.execute_batch("rollback to call; release call").unwrap();
    pending.truncate(mark);
    c.execute_batch("commit").unwrap();
    let delivered = pending.clone();
    assert_eq!(delivered, vec![1]);
    pending.clear();
    c.execute_batch("begin immediate; insert into events values (3)")
        .unwrap();
    pending.push(3);
    c.execute_batch("rollback").unwrap();
    pending.clear();
    assert!(pending.is_empty());
    // COMMIT can fail after statements succeeded. Deferred FK makes this deterministic.
    c.execute_batch(
        "pragma foreign_keys=on; create table parent(id primary key);
        create table child(id references parent(id) deferrable initially deferred);
        begin immediate; insert into child values(99)",
    )
    .unwrap();
    pending.push(99);
    assert!(c.execute_batch("commit").is_err());
    c.execute_batch("rollback").unwrap();
    pending.clear();
    // Post-return callbacks can be overtaken: A commits, stalls, B commits and publishes first.
    let committed = Arc::new(std::sync::Mutex::new(c));
    let delivered_order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (a_done, a_wait) = std::sync::mpsc::channel();
    let (b_done, b_wait) = std::sync::mpsc::channel();
    let (writer, out) = (committed.clone(), delivered_order.clone());
    let a = std::thread::spawn(move || {
        writer
            .lock()
            .unwrap()
            .execute_batch("begin; insert into events values (4); commit")
            .unwrap();
        a_done.send(()).unwrap();
        b_wait.recv().unwrap();
        out.lock().unwrap().push(4);
    });
    a_wait.recv().unwrap();
    committed
        .lock()
        .unwrap()
        .execute_batch("begin; insert into events values (5); commit")
        .unwrap();
    delivered_order.lock().unwrap().push(5);
    b_done.send(()).unwrap();
    a.join().unwrap();
    let callback_order = delivered_order.lock().unwrap().clone();
    assert_eq!(callback_order, vec![5, 4]);
    println!(
        "{}",
        json!({"experiment":"commit-check","status":"pass","committed_delivered":delivered,
        "savepoint_rollback_suppressed":true,"transaction_rollback_suppressed":true,
        "failed_commit_suppressed":true,"post_return_reordering_witness":callback_order,
        "note":"prototype staging checks, not tests of the product transaction hook"})
    );
}

pub fn crash_owner(stage: &str) {
    use std::io::Write;
    let path = std::env::var("BENCH_DB").unwrap();
    let c = open(&path, "FULL");
    seed(&c, 1, 1);
    c.execute_batch("begin immediate").unwrap();
    let l = acquire(&c, 0, 1, 100, 110).unwrap();
    if stage == "committed" {
        c.execute_batch("commit").unwrap();
    }
    println!("{}", json!({"ready":true,"stage":stage,"token":l.token}));
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

pub fn crash_recover(stage: &str) {
    let path = std::env::var("BENCH_DB").unwrap();
    let c = open(&path, "FULL");
    c.execute_batch("begin immediate").unwrap();
    let early = acquire(&c, 0, 2, 105, 120);
    if stage == "committed" {
        assert!(early.is_none());
    } else {
        assert_eq!(early.unwrap().token, 1);
        c.execute_batch("rollback; begin immediate").unwrap();
    }
    let after = acquire(&c, 0, 2, 110, 130).unwrap();
    assert_eq!(after.token, if stage == "committed" { 2 } else { 1 });
    c.execute_batch("commit").unwrap();
    assert_eq!(
        c.query_row("pragma integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    println!(
        "{}",
        json!({"experiment":"lease-crash","stage":stage,"status":"pass",
        "token_after_recovery":after.token,"early_rejected":stage=="committed"})
    );
}
