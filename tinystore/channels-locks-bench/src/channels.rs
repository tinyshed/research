use crate::{cpu_seconds, memory, percentile, us};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{Notify, broadcast, mpsc};

#[derive(Clone, Debug)]
struct Event {
    seq: u64,
    sent: Instant,
    bytes: Arc<[u8]>,
}

struct State {
    slots: Vec<Option<Arc<Event>>>,
    next: u64,
    closed: bool,
}
struct Ring {
    state: Mutex<State>,
    notify: Notify,
}

impl Ring {
    fn new(cap: usize) -> Self {
        Self {
            state: Mutex::new(State {
                slots: vec![None; cap],
                next: 0,
                closed: false,
            }),
            notify: Notify::new(),
        }
    }
    fn publish(&self, e: Arc<Event>) {
        let mut s = self.state.lock().unwrap();
        let slot = s.next as usize % s.slots.len();
        s.slots[slot] = Some(e);
        s.next += 1;
        drop(s);
        self.notify.notify_waiters();
    }
    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.notify.notify_waiters();
    }
    async fn recv(&self, cursor: &mut u64) -> Option<Result<Arc<Event>, u64>> {
        loop {
            // Register before the state check; Notify stores no notify_waiters permit.
            let ready = self.notify.notified();
            tokio::pin!(ready);
            ready.as_mut().enable();
            {
                let s = self.state.lock().unwrap();
                let oldest = s.next.saturating_sub(s.slots.len() as u64);
                if *cursor < oldest {
                    let lost = oldest - *cursor;
                    *cursor = oldest;
                    return Some(Err(lost));
                }
                if *cursor < s.next {
                    let e = s.slots[*cursor as usize % s.slots.len()]
                        .as_ref()
                        .unwrap()
                        .clone();
                    *cursor += 1;
                    return Some(Ok(e));
                }
                if s.closed {
                    return None;
                }
            }
            ready.await;
        }
    }
}

enum Rx {
    Broadcast(broadcast::Receiver<Arc<Event>>),
    Queue(mpsc::Receiver<Arc<Event>>),
    Ring(Arc<Ring>, u64),
}
enum Tx {
    Broadcast(broadcast::Sender<Arc<Event>>),
    Queue(Vec<mpsc::Sender<Arc<Event>>>),
    Ring(Arc<Ring>),
}

#[derive(Default)]
struct Received {
    delivered: u64,
    lost: u64,
    gaps: u64,
    sample: Vec<u64>,
    last: Option<u64>,
    checksum: u64,
}

async fn consume(mut rx: Rx, id: usize, slow: bool) -> Received {
    let mut r = Received::default();
    loop {
        let next = match &mut rx {
            Rx::Broadcast(b) => match b.recv().await {
                Ok(e) => Some(Ok(e)),
                Err(broadcast::error::RecvError::Lagged(n)) => Some(Err(n)),
                Err(broadcast::error::RecvError::Closed) => None,
            },
            Rx::Queue(q) => q.recv().await.map(Ok),
            Rx::Ring(b, cursor) => b.recv(cursor).await,
        };
        let Some(next) = next else {
            break;
        };
        match next {
            Err(n) => {
                r.lost += n;
                r.gaps += 1;
            }
            Ok(e) => {
                if let Some(last) = r.last {
                    assert!(e.seq > last);
                }
                if matches!(rx, Rx::Queue(_)) {
                    let missing = e.seq - r.last.map_or(0, |last| last + 1);
                    if missing > 0 {
                        r.lost += missing;
                        r.gaps += 1;
                    }
                }
                r.last = Some(e.seq);
                r.delivered += 1;
                r.checksum ^= e.bytes[(e.seq as usize) % e.bytes.len()] as u64;
                if (e.seq + id as u64).is_multiple_of(64) {
                    r.sample.push(us(e.sent.elapsed()));
                }
                if slow && id.is_multiple_of(10) && r.delivered % 32 == 0 {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
                // Queue.recv may stay immediately ready; bound uninterrupted consumer work equally.
                if r.delivered % 64 == 0 {
                    tokio::task::yield_now().await;
                }
            }
        }
    }
    r
}

fn pair(kind: &str, n: usize, cap: usize) -> (Tx, Vec<Rx>) {
    match kind {
        "broadcast" => {
            let (tx, rx) = broadcast::channel(cap);
            drop(rx);
            let rxs = (0..n).map(|_| Rx::Broadcast(tx.subscribe())).collect();
            (Tx::Broadcast(tx), rxs)
        }
        "queues" => {
            let mut txs = vec![];
            let rxs = (0..n)
                .map(|_| {
                    let (tx, rx) = mpsc::channel(cap);
                    txs.push(tx);
                    Rx::Queue(rx)
                })
                .collect();
            (Tx::Queue(txs), rxs)
        }
        "ring" => {
            let ring = Arc::new(Ring::new(cap));
            (
                Tx::Ring(ring.clone()),
                (0..n).map(|_| Rx::Ring(ring.clone(), 0)).collect(),
            )
        }
        _ => panic!("unknown channel kind"),
    }
}

pub fn run(a: &[String]) {
    let kind = &a[0];
    let n: usize = a[1].parse().unwrap();
    let count: u64 = a[2].parse().unwrap();
    let size: usize = a[3].parse().unwrap();
    let mode = &a[4];
    let cap: usize = a[5].parse().unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .unwrap();
    rt.block_on(async {
        let initial = memory();
        let (tx, rxs) = pair(kind, n, cap);
        let mut consumers = vec![];
        let pause=Arc::new(tokio::sync::Semaphore::new(if mode=="paused"{0}else{n}));
        for (id, rx) in rxs.into_iter().enumerate() {
            let slow = mode == "slow";
            let gate=pause.clone();
            consumers.push(tokio::spawn(async move {
                let permit=gate.acquire().await.unwrap();permit.forget();
                consume(rx,id,slow).await
            }));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        let idle = memory();
        let before_cpu = cpu_seconds();
        let start = Instant::now();
        let mut publish_times = vec![];
        let mut rejected = 0u64;
        for seq in 0..count {
            if mode == "paced" {
                let target = start + Duration::from_micros(seq * 1000);
                if target > Instant::now() { tokio::time::sleep_until(target.into()).await; }
            }
            let now = Instant::now();
            // Payload construction allocates here; every recipient shares the resulting Arc.
            let e = Arc::new(Event { seq, sent: now, bytes: vec![(seq % 251) as u8; size].into() });
            match &tx {
                Tx::Broadcast(b) => { b.send(e).unwrap(); },
                Tx::Queue(qs) => for q in qs { if q.try_send(e.clone()).is_err() { rejected += 1; } },
                Tx::Ring(b) => b.publish(e),
            }
            if seq % 64 == 0 { publish_times.push(us(now.elapsed())); }
            // Explicitly equal publisher scheduling; saturation is still intentionally open-loop.
            if seq % 32 == 31 { tokio::task::yield_now().await; }
        }
        let publishing = start.elapsed().as_secs_f64();
        let queued_memory = memory();
        if mode=="paused" {pause.add_permits(n);}
        if let Tx::Ring(r) = &tx { r.close(); }
        drop(tx);
        let (mut delivered, mut observed_gaps, mut lost, mut fast_lost, mut slow_lost) = (0u64, 0, 0, 0, 0);
        let mut samples = vec![];
        let mut per_sub = vec![];
        for (id, c) in consumers.into_iter().enumerate() {
            let r = c.await.unwrap();
            assert!(r.delivered <= count);
            // Queue tail loss is known only to the publisher here, not the receiver.
            if kind != "queues" { assert_eq!(r.delivered + r.lost, count); }
            delivered += r.delivered;
            observed_gaps += r.gaps;
            lost += count - r.delivered;
            if mode == "slow" && id % 10 == 0 { slow_lost += count - r.delivered; }
            else { fast_lost += count - r.delivered; }
            samples.extend(r.sample);
            per_sub.push(r.delivered);
        }
        if kind == "queues" { assert_eq!(rejected, lost); }
        let total = start.elapsed().as_secs_f64();
        println!("{}", json!({"experiment":"channels", "kind":kind,"subscribers":n,"messages":count,
            "payload_bytes":size,"capacity":cap,"mode":mode,"publish_s":publishing,"total_s":total,
            "publish_per_s": count as f64 / publishing,"deliveries_per_s":delivered as f64 / total,
            "delivered":delivered,"lost":lost,"fast_lost":fast_lost,"slow_lost":slow_lost,
            "queue_rejected":rejected,"observed_gap_events":observed_gaps,
            "delivery_samples":samples.len(),"delivery_p50_us":percentile(&mut samples,0.50),
            "delivery_p99_us":percentile(&mut samples,0.99),
            "publish_p50_us":percentile(&mut publish_times,0.50),"publish_p99_us":percentile(&mut publish_times,0.99),
            "cpu_s":cpu_seconds()-before_cpu,"initial_memory":initial,"idle_memory":idle,
            "queued_memory":queued_memory,"final_memory":memory(),
            "min_delivered":per_sub.iter().min(),"max_delivered":per_sub.iter().max()}));
    });
}

pub fn check() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut checked = vec![];
        for kind in ["broadcast", "ring", "queues"] {
            let (tx, mut rxs) = pair(kind, 1, 4);
            for seq in 0..10 {
                let e = Arc::new(Event {
                    seq,
                    sent: Instant::now(),
                    bytes: Arc::from([1u8]),
                });
                match &tx {
                    Tx::Broadcast(t) => {
                        t.send(e).unwrap();
                    }
                    Tx::Ring(r) => r.publish(e),
                    Tx::Queue(q) => {
                        let _ = q[0].try_send(e);
                    }
                }
            }
            if let Tx::Ring(r) = &tx {
                r.close();
            }
            drop(tx);
            let r = consume(rxs.remove(0), 0, false).await;
            assert_eq!(r.delivered, 4);
            if kind == "queues" {
                assert_eq!(r.lost, 0);
            } else {
                assert_eq!(r.lost, 6);
                assert_eq!(r.gaps, 1);
            }
            checked.push(
                json!({"kind":kind,"delivered":r.delivered,"observed_lost":r.lost,
                "actual_lost":6,"tail_gap_visible":kind != "queues"}),
            );
        }
        // A reader opened at head has no replay; no cursor starts at zero in the proposed API.
        let ring = Ring::new(4);
        ring.publish(Arc::new(Event {
            seq: 0,
            sent: Instant::now(),
            bytes: Arc::from([1u8]),
        }));
        let mut cursor = ring.state.lock().unwrap().next;
        ring.close();
        assert!(ring.recv(&mut cursor).await.is_none());
        // Force the check/register race repeatedly under a timeout.
        for _ in 0..1000 {
            let r = Arc::new(Ring::new(4));
            let other = r.clone();
            let reader = tokio::spawn(async move {
                let mut c = 0;
                other.recv(&mut c).await.unwrap().unwrap().seq
            });
            tokio::task::yield_now().await;
            r.publish(Arc::new(Event {
                seq: 0,
                sent: Instant::now(),
                bytes: Arc::from([1u8]),
            }));
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), reader)
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        }
        println!(
            "{}",
            json!({"experiment":"channel-check","overflow":checked,"no_reconnect_replay":true,
            "wake_races":1000,"status":"pass"})
        );
    });
}

#[derive(Default)]
struct Node {
    children: HashMap<String, Node>,
    exact: usize,
    prefix: usize,
}
impl Node {
    fn insert(&mut self, path: &str, prefix: bool) {
        let mut node = self;
        for p in path.split('/') {
            node = node.children.entry(p.to_owned()).or_default();
        }
        if prefix {
            node.prefix += 1;
        } else {
            node.exact += 1;
        }
    }
    fn matches(&self, path: &str) -> usize {
        let mut node = self;
        let mut count = node.prefix;
        for p in path.split('/') {
            match node.children.get(p) {
                Some(n) => {
                    node = n;
                    count += node.prefix;
                }
                None => return count,
            }
        }
        count + node.exact
    }
}

pub fn route(a: &[String]) {
    let kind = &a[0];
    let n: usize = a[1].parse().unwrap();
    let count: usize = a[2].parse().unwrap();
    let initial = memory();
    let paths: Vec<String> = (0..n)
        .map(|i| format!("tenant/{}/orders/{}", i % 100, i))
        .collect();
    let mut trie = Node::default();
    let mut map = HashMap::<String, usize>::new();
    for p in &paths {
        trie.insert(p, false);
        *map.entry(p.clone()).or_default() += 1;
    }
    for i in 0..100 {
        trie.insert(&format!("tenant/{i}"), true);
    }
    let prefixes: Vec<String> = (0..100).map(|i| format!("tenant/{i}")).collect();
    let keys: Vec<String> = (0..1024).map(|i| paths[i % n].clone()).collect();
    let idle = memory();
    let start = Instant::now();
    let mut checksum = 0;
    for i in 0..count {
        let p = std::hint::black_box(&keys[i % keys.len()]);
        let result = match kind.as_str() {
            "scan" => {
                paths.iter().filter(|x| *x == p).count()
                    + prefixes
                        .iter()
                        .filter(|x| {
                            p.strip_prefix(x.as_str())
                                .is_some_and(|rest| rest.starts_with('/'))
                        })
                        .count()
            }
            "trie" => trie.matches(p),
            _ => panic!("unknown route kind"),
        };
        assert_eq!(result, 2);
        checksum += result;
    }
    println!(
        "{}",
        json!({"experiment":"route","kind":kind,"subscriptions":n+100,"lookups":count,
        "lookup_per_s":count as f64/start.elapsed().as_secs_f64(),"checksum":checksum,
        "initial_memory":initial,"idle_memory":idle,"final_memory":memory(),
        "note":"all structures allocated in every mode; RSS is not a routing structure comparison"})
    );
}
