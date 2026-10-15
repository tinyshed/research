use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::json;
use sha2::{Digest, Sha256};
use tinystore::sql::{Database, Sql};
use tinystore::{Options, Store, TestClock};

use crate::node::{Limits, Node};
use crate::packet::Packet;
use crate::pages::{self, Image};
use crate::transport::{self, Server};
use crate::{Result, require};

pub(super) fn run() -> Result<()> {
    println!(
        "{}",
        json!({"case":"configuration", "sqlite":rusqlite::version(),
        "passes":3,"updates_per_pass":100,"fixture_rows":4096,"body_bytes":256,
        "children":140,"relays":7,"concurrent_clients":7,"capture":"snapshot_scan_diff",
        "durability":"FULL", "clock":"1800000000 seconds since epoch"})
    );
    for pass in 1..=3 {
        for capture in [false, true] {
            capture_cost(pass, capture)?;
        }
    }
    engine_bytes()?;
    for pass in 1..=3 {
        for tree in [false, true] {
            fanout(pass, tree)?;
        }
    }
    Ok(())
}

pub(super) fn batching() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (_store, db, mut parent) = database(&dir.path().join("primary"), 32)?;
    for index in 0..99 {
        db.exec(
            Sql::new("update documents set body=?1 where id=?2")
                .bind(&body(index + 100))
                .bind(&(index % 32)),
        )?;
        parent.capture("app")?;
    }
    let target = parent.position("app")?;
    let parent = Arc::new(Mutex::new(parent));
    let server = Server::start(Arc::clone(&parent))?;
    for pass in 1..=3 {
        for entries in [1, 32] {
            let mut child = Node::open(&dir.path().join(format!("child-{pass}-{entries}")))?;
            let before = server.counts();
            let started = Instant::now();
            let mut applied = 0;
            while child.position("app")? != target {
                let received = transport::sync(
                    &mut child,
                    server.address,
                    "app",
                    Limits {
                        entries,
                        bytes: 256 * 1024,
                    },
                )?;
                require(received > 0, "upstream stopped before the target position")?;
                applied += received;
            }
            let seconds = started.elapsed().as_secs_f64();
            let after = server.counts();
            println!(
                "{}",
                json!({"case":"batching", "pass":pass,"batch_entries":entries,
                "applied":applied,"requests":after.0-before.0,"response_bytes":after.1-before.1,
                "seconds":seconds,"target_verified":child.position("app")?==target})
            );
        }
    }
    Ok(())
}

fn options() -> Options {
    Options {
        background: false,
        clock: Some(Arc::new(TestClock::new(
            UNIX_EPOCH + Duration::from_secs(1_800_000_000),
        ))),
        ..Options::default()
    }
}

fn body(index: usize) -> String {
    (0..4)
        .map(|block| {
            let bytes = Sha256::digest(format!("replication-fixture/{index}/{block}"));
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        })
        .collect()
}

fn database(root: &Path, rows: usize) -> Result<(Store, Database, Node)> {
    let store = Store::open(root, options())?;
    let db = store.database("app").open()?;
    db.exec("create table documents(id integer primary key, body text not null)")?;
    db.tx::<_, tinystore::Error>(|tx| {
        for index in 0..rows {
            tx.exec(
                Sql::new("insert into documents values(?1, ?2)")
                    .bind(&index)
                    .bind(&body(index)),
            )?;
        }
        Ok(())
    })?;
    let mut node = Node::open(root)?;
    node.publish("app", "sql/app.db")?;
    node.capture("app")?;
    Ok((store, db, node))
}

fn capture_cost(pass: u32, capture: bool) -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (_store, db, mut node) = database(dir.path(), 4096)?;
    let mut total = Vec::new();
    let mut captures = Vec::new();
    for index in 0..100 {
        let value = body(10000 + index);
        let statement = Sql::new("update documents set body=?1 where id=?2")
            .bind(&value)
            .bind(&index);
        let started = Instant::now();
        db.exec(statement)?;
        let written = Instant::now();
        if capture {
            node.capture("app")?;
        }
        total.push(started.elapsed().as_secs_f64() * 1e6);
        captures.push(written.elapsed().as_secs_f64() * 1e6);
    }
    let elapsed: f64 = total.iter().sum();
    println!(
        "{}",
        json!({"case":"capture_cost","pass":pass,"capture":capture,"writes":100,
        "total_us":elapsed,"write_and_capture_p50_us":percentile(&total,50),
        "write_and_capture_p95_us":percentile(&total,95),"capture_p50_us":percentile(&captures,50),
        "file_bytes":Image::read(&mut pages::reader(&dir.path().join("sql/app.db"))?)?.pages.len()*4096})
    );
    Ok(())
}

fn percentile(values: &[f64], percent: usize) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[(sorted.len() * percent).div_ceil(100).saturating_sub(1)]
}

fn engine_bytes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::open(dir.path(), options())?;
    let kv = store.bucket::<String>("sample").open()?;
    let jobs = store.queue::<String>("sample").open()?;
    let sql = store.database("app").open()?;
    sql.exec("create table documents(id integer primary key, body text)")?;
    let events = store.events("sample").open()?;
    let metrics = store.metrics()?;
    let mut node = Node::open(dir.path())?;
    for (stream, path) in [
        ("kv", "kv.db"),
        ("jobs", "jobs.db"),
        ("sql", "sql/app.db"),
        ("records", "records.db"),
        ("metrics", "metrics.db"),
    ] {
        node.publish(stream, path)?;
        node.capture(stream)?;
    }
    for index in 0..10usize {
        let value = body(index);
        kv.set(index.to_string(), &value)?;
        jobs.add(&value)?;
        sql.exec(
            Sql::new("insert into documents values(?1, ?2)")
                .bind(&index)
                .bind(&value),
        )?;
        events.append(
            "request",
            &serde_json::json!({"request":index,"body":value}),
        )?;
        metrics.ingest(&[tinystore::metrics::Series::gauge("cpu")
            .label("host", "a")
            .sample(
                store.now() - Duration::from_secs((10 - index) as u64),
                index as f64,
            )])?;
        for stream in ["kv", "jobs", "sql", "records", "metrics"] {
            let before = node.position(stream)?.unwrap().sequence;
            node.capture(stream)?;
            let bytes = node.changes(stream, before, Limits::default())?.remove(0);
            let packet = Packet::decode(&bytes)?;
            let uncompressed =
                packet.pages.len() * (packet.page_size as usize + 4) + 102 + stream.len();
            println!(
                "{}",
                json!({"case":"engine_bytes","engine":stream,"sample":index+1,
                "changed_pages":packet.pages.len(),"changed_page_bytes":packet.pages.len()*packet.page_size as usize,
                "packet_raw_bytes":uncompressed,"packet_zstd_bytes":bytes.len(),
                "file_bytes":packet.page_count as usize*packet.page_size as usize})
            );
        }
    }
    Ok(())
}

fn fanout(pass: u32, tree: bool) -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (_store, db, mut parent) = database(&dir.path().join("primary"), 32)?;
    for index in 0..4 {
        db.exec(
            Sql::new("update documents set body=?1 where id=?2")
                .bind(&body(index + 100))
                .bind(&index),
        )?;
        parent.capture("app")?;
    }
    let parent = Arc::new(Mutex::new(parent));
    let server = Server::start(Arc::clone(&parent))?;
    let mut children: Vec<Node> = (0..140)
        .map(|i| Node::open(&dir.path().join(format!("child-{i}"))))
        .collect::<Result<_>>()?;
    let mut relays = Vec::new();
    let mut endpoints = vec![server.address; 7];
    let mut relay_nodes = Vec::new();
    if tree {
        for index in 0..7 {
            relay_nodes.push(Node::open(&dir.path().join(format!("relay-{index}")))?);
        }
    }
    let started = Instant::now();
    for (index, mut relay) in relay_nodes.into_iter().enumerate() {
        transport::sync(&mut relay, server.address, "app", Limits::default())?;
        let relay = Server::start(Arc::new(Mutex::new(relay)))?;
        endpoints[index] = relay.address;
        relays.push(relay);
    }
    std::thread::scope(|scope| -> Result<()> {
        let mut workers = Vec::new();
        for (chunk, endpoint) in children.chunks_mut(20).zip(endpoints) {
            workers.push(scope.spawn(move || -> Result<()> {
                for child in chunk {
                    transport::sync(child, endpoint, "app", Limits::default())?;
                }
                Ok(())
            }));
        }
        for worker in workers {
            worker
                .join()
                .map_err(|_| std::io::Error::other("fanout worker panicked"))??;
        }
        Ok(())
    })?;
    let seconds = started.elapsed().as_secs_f64();
    for child in &children {
        require(
            child.position("app")? == parent.lock().unwrap().position("app")?,
            "fanout lost a checkpoint",
        )?;
    }
    let root = server.counts();
    let total_bytes = root.1 + relays.iter().map(|r| r.counts().1).sum::<u64>();
    println!(
        "{}",
        json!({"case":"fanout","pass":pass,"topology":if tree {"7x20"} else {"star"},
        "children":140,"seconds":seconds,"root_requests":root.0,"root_response_bytes":root.1,
        "all_response_bytes":total_bytes,"all_children_verified":true})
    );
    Ok(())
}
