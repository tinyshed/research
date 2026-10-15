use crate::common::*;
use serde_json::json;
use std::{
    sync::{
        Arc, Barrier, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tinystore::kv::Bucket;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn acquire(b: &Bucket<String>, key: &str, owner: u64, ttl: u64) -> Option<u64> {
    let e = b.entry(key).unwrap().unwrap();
    let state: Vec<u64> = serde_json::from_str(&e.value).unwrap();
    if state[0] != 0 && state[2] > now() {
        return None;
    }
    let fence = state[1] + 1;
    let value = serde_json::to_string(&vec![owner, fence, now() + ttl]).unwrap();
    b.key(key)
        .if_version(e.version)
        .set(&value)
        .ok()
        .map(|_| fence)
}
fn release(b: &Bucket<String>, key: &str, owner: u64, fence: u64) -> bool {
    let e = b.entry(key).unwrap().unwrap();
    let state: Vec<u64> = serde_json::from_str(&e.value).unwrap();
    if state[0] != owner || state[1] != fence {
        return false;
    }
    b.key(key)
        .if_version(e.version)
        .set(&serde_json::to_string(&vec![0, fence, 0]).unwrap())
        .is_ok()
}
#[derive(Default)]
struct Gate {
    state: Mutex<(u64, u64)>,
    ready: Condvar,
}
impl Gate {
    fn enter(&self) -> u64 {
        let mut s = self.state.lock().unwrap();
        let ticket = s.0;
        s.0 += 1;
        while s.1 != ticket {
            s = self.ready.wait(s).unwrap();
        }
        ticket
    }
    fn leave(&self) {
        let mut s = self.state.lock().unwrap();
        s.1 += 1;
        self.ready.notify_all();
    }
}
pub fn locks(a: &[String]) {
    let algorithm = &a[0];
    let clients: usize = a[1].parse().unwrap();
    let resources: usize = a[2].parse().unwrap();
    let seconds: u64 = a[3].parse().unwrap();
    let store = tinystore::Store::open(path(), Default::default()).unwrap();
    let b = store.bucket::<String>("research-leases").open().unwrap();
    for r in 0..resources {
        b.set(r.to_string(), &"[0,0,0]".to_owned()).unwrap();
    }
    let gates: Arc<Vec<Gate>> = Arc::new((0..resources).map(|_| Gate::default()).collect());
    let barrier = Arc::new(Barrier::new(clients + 1));
    let deadline: Arc<OnceLock<Instant>> = Arc::new(OnceLock::new());
    let attempts = Arc::new(AtomicU64::new(0));
    let idle = memory();
    let cpu0 = cpu();
    let mut handles = vec![];
    for client in 0..clients {
        let deadline = deadline.clone();
        let (b, gates, barrier, attempts, fifo) = (
            b.clone(),
            gates.clone(),
            barrier.clone(),
            attempts.clone(),
            algorithm == "fifo",
        );
        handles.push(
            std::thread::Builder::new()
                .stack_size(256 * 1024)
                .spawn(move || {
                    barrier.wait();
                    let deadline = *deadline.get().unwrap();
                    let mut waits = vec![];
                    let mut grants = 0;
                    let mut canceled = 0;
                    while Instant::now() < deadline {
                        let resource = client % gates.len();
                        let key = resource.to_string();
                        let started = Instant::now();
                        if fifo {
                            gates[resource].enter();
                        }
                        if Instant::now() >= deadline {
                            if fifo {
                                gates[resource].leave();
                            }
                            canceled += 1;
                            break;
                        }
                        let fence = loop {
                            attempts.fetch_add(1, Ordering::Relaxed);
                            if let Some(f) = acquire(&b, &key, client as u64 + 1, 10000) {
                                break Some(f);
                            }
                            if Instant::now() >= deadline {
                                break None;
                            }
                            std::thread::sleep(Duration::from_millis(1));
                        };
                        if let Some(fence) = fence {
                            waits.push(us(started));
                            grants += 1;
                            std::thread::sleep(Duration::from_micros(100));
                            assert!(release(&b, &key, client as u64 + 1, fence));
                        } else {
                            canceled += 1;
                        }
                        if fifo {
                            gates[resource].leave();
                        }
                    }
                    (grants, waits, canceled)
                })
                .unwrap(),
        );
    }
    let admitted_memory = memory();
    let started = Instant::now();
    deadline
        .set(started + Duration::from_secs(seconds))
        .unwrap();
    barrier.wait();
    let mut counts = vec![];
    let mut waits = vec![];
    let mut canceled = 0;
    for h in handles {
        let (g, mut w, c) = h.join().unwrap();
        counts.push(g);
        waits.append(&mut w);
        canceled += c;
    }
    let duration = started.elapsed().as_secs_f64();
    let cpu_s = cpu() - cpu0;
    let grants: u64 = counts.iter().sum();
    let sumsq: u64 = counts.iter().map(|g| g * g).sum();
    let fairness = (grants as f64).powi(2) / (clients as f64 * sumsq.max(1) as f64);
    let workmem = memory();
    let first = acquire(&b, "0", 10001, 20).unwrap();
    assert!(acquire(&b, "0", 10002, 20).is_none());
    std::thread::sleep(Duration::from_millis(25));
    let second = acquire(&b, "0", 10002, 20).unwrap();
    assert!(second > first);
    assert!(!release(&b, "0", 10001, first));
    assert!(release(&b, "0", 10002, second));
    store.close().unwrap();
    let reopened = tinystore::Store::open(path(), Default::default()).unwrap();
    let b2 = reopened.bucket::<String>("research-leases").open().unwrap();
    let third = acquire(&b2, "0", 10003, 1000).unwrap();
    assert!(third > second);
    assert!(release(&b2, "0", 10003, third));
    reopened.close().unwrap();
    println!(
        "{}",
        json!({"experiment":"core-locks","algorithm":algorithm,"clients":clients,"resources":resources,"duration_s":duration,"grants":grants,"grants_per_s":grants as f64/duration,"attempts":attempts.load(Ordering::Relaxed),"attempts_per_grant":attempts.load(Ordering::Relaxed) as f64/grants.max(1) as f64,"wait":distribution(&mut waits),"grants_by_client":counts,"jain_fairness":fairness,"deadline_canceled":canceled,"cpu_s":cpu_s,"idle_memory":idle,"admitted_memory":admitted_memory,"workload_memory":workmem,"fencing_checks":["TTL takeover","stale release rejected","fence persists through restart"],"product_revision":"76b844cdc8c3410e746a5d5dc3e9aafec83c9962","status":"pass"})
    );
}

pub fn check() {
    let store = tinystore::Store::open(path(), Default::default()).unwrap();
    let b = store.bucket::<String>("leases").open().unwrap();
    b.set("r", &"[0,0,0]".to_owned()).unwrap();
    let f = acquire(&b, "r", 1, 1000).unwrap();
    let e = b.entry("r").unwrap().unwrap();
    let state = serde_json::to_string(&vec![1, f, now() + 2000]).unwrap();
    b.key("r").if_version(e.version).set(&state).unwrap();
    assert!(acquire(&b, "r", 2, 100).is_none());
    assert!(release(&b, "r", 1, f));
    let queue = store.queue::<String>("research-jobs").open().unwrap();
    let result = store.tx(|tx| -> tinystore::Result<()> {
        tx.with(&b).set("rollback", &"sentinel".to_owned())?;
        tx.with(&queue).add(&"job".to_owned())?;
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(b.get("rollback").unwrap(), None);
    let barrier = Arc::new(Barrier::new(2));
    let (s, b2, gate) = (store.clone(), b.clone(), barrier.clone());
    let blocked = std::thread::spawn(move || {
        s.tx(|tx| -> tinystore::Result<()> {
            tx.with(&b2).set("hold", &"x".to_owned())?;
            gate.wait();
            std::thread::sleep(Duration::from_millis(100));
            Ok(())
        })
        .unwrap()
    });
    barrier.wait();
    let t = Instant::now();
    let late = acquire(&b, "r", 3, 10).unwrap();
    let delayed_us = us(t);
    blocked.join().unwrap();
    let e = b.entry("r").unwrap().unwrap();
    let state: Vec<u64> = serde_json::from_str(&e.value).unwrap();
    let expired_on_return = state[2] <= now();
    assert!(expired_on_return);
    assert!(release(&b, "r", 3, late));
    // A committed acquire whose response disappeared can be reconciled by the
    // retained owner identity, but a blind retry cannot be made idempotent.
    let lost = acquire(&b, "r", 4, 1000).unwrap();
    assert!(acquire(&b, "r", 4, 1000).is_none());
    let retained = b.entry("r").unwrap().unwrap();
    let owner: Vec<u64> = serde_json::from_str(&retained.value).unwrap();
    assert_eq!(owner[0], 4);
    assert!(release(&b, "r", 4, lost));
    let barrier = Arc::new(Barrier::new(2));
    let (s, q, gate) = (store.clone(), queue.clone(), barrier.clone());
    let jobs = std::thread::spawn(move || {
        s.tx(|tx| -> tinystore::Result<()> {
            tx.with(&q).add(&"held-job".to_owned())?;
            gate.wait();
            std::thread::sleep(Duration::from_millis(100));
            Ok(())
        })
        .unwrap()
    });
    barrier.wait();
    let t = Instant::now();
    let f = acquire(&b, "r", 5, 1000).unwrap();
    let independent_file_us = us(t);
    assert!(release(&b, "r", 5, f));
    jobs.join().unwrap();
    // Diagnostic only: pin a reader so the real product's WAL cannot recycle.
    let kvpath = format!("{}/kv.db", path());
    let reader = rusqlite::Connection::open(&kvpath).unwrap();
    reader
        .execute_batch("pragma wal_checkpoint(truncate);begin;select count(*) from sqlite_schema")
        .unwrap();
    let before = wal_bytes(&kvpath);
    let mut workers = vec![];
    for client in 0..32 {
        let b = b.clone();
        workers.push(std::thread::spawn(move || {
            for i in 0..100 {
                b.set(format!("group-{client}-{i}"), &"x".to_owned())
                    .unwrap();
            }
        }));
    }
    for w in workers {
        w.join().unwrap();
    }
    let wal = std::fs::read(format!("{kvpath}-wal")).unwrap();
    let mut commits = 0;
    let mut frames = 0;
    for f in wal[32..]
        .chunks_exact(4120)
        .skip(((before.saturating_sub(32)) / 4120) as usize)
    {
        frames += 1;
        if u32::from_be_bytes(f[4..8].try_into().unwrap()) != 0 {
            commits += 1;
        }
    }
    reader.execute_batch("rollback").unwrap();
    drop(reader);
    store.close().unwrap();
    println!(
        "{}",
        json!({"experiment":"lease-check","renew":"owner/fence retained; CAS protected","cross_file_transaction_rejected":true,"delayed_commit_us":delayed_us,"lease_expired_on_acquire_return":expired_on_return,"lost_response_owner_reconciliation":true,"blind_retry_idempotent":false,"jobs_independent_file_acquire_us":independent_file_us,"grouped_commit_diagnostic":{"writes":3200,"wal_commit_markers":commits,"wal_frames":frames,"reader_pinned":true},"status":"pass"})
    );
}
pub fn crash() {
    use std::io::Write;
    let store = tinystore::Store::open(path(), Default::default()).unwrap();
    let b = store.bucket::<String>("leases").open().unwrap();
    b.set("r", &"[0,0,0]".to_owned()).unwrap();
    let fence = acquire(&b, "r", 101, 100).unwrap();
    println!("{}", json!({"ready":true,"fence":fence}));
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}
pub fn recover() {
    let t = Instant::now();
    let store = tinystore::Store::open(path(), Default::default()).unwrap();
    let b = store.bucket::<String>("leases").open().unwrap();
    let e = b.entry("r").unwrap().unwrap();
    let old: Vec<u64> = serde_json::from_str(&e.value).unwrap();
    let retained = old[0] == 101;
    if old[2] > now() {
        std::thread::sleep(Duration::from_millis(old[2] - now() + 1));
    }
    let next = acquire(&b, "r", 102, 1000).unwrap();
    assert!(next > old[1]);
    assert!(!release(&b, "r", 101, old[1]));
    println!(
        "{}",
        json!({"experiment":"lease-recover","owner_retained":retained,"old_fence":old[1],"new_fence":next,"recovery_including_ttl_us":us(t),"status":"pass"})
    );
    store.close().unwrap();
}
