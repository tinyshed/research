use crate::capture::{Mode, Source};
use crate::{Result, pages, replica, require};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};
use tinystore::capture_experiment::register;
use tinystore::sql::Sql;
use tinystore::{Options, Store, TestClock};

fn options() -> Options {
    Options {
        background: false,
        clock: Some(Arc::new(TestClock::new(
            UNIX_EPOCH + Duration::from_secs(1_800_000_000),
        ))),
        ..Default::default()
    }
}
fn body(index: usize) -> String {
    (0..4)
        .map(|block| {
            Sha256::digest(format!("replication-fixture/{index}/{block}"))
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        })
        .collect()
}
fn seed(path: &Path, rows: usize) -> Result<()> {
    let store = Store::open(path, options())?;
    let db = store.database("app").open()?;
    db.exec("create table documents(id integer primary key,body text not null)")?;
    db.tx::<_, tinystore::Error>(|tx| {
        for index in 0..rows {
            tx.exec(
                Sql::new("insert into documents values(?1,?2)")
                    .bind(&index)
                    .bind(&body(index)),
            )?;
        }
        Ok(())
    })?;
    store.close()?;
    Ok(())
}
fn percentile(v: &[f64], p: usize) -> f64 {
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    v[(v.len() * p).div_ceil(100).saturating_sub(1)]
}

pub(super) fn run() -> Result<()> {
    println!(
        "{}",
        json!({"case":"configuration","sqlite":rusqlite::version(),"passes":3,"body_bytes":256,"updates":256,"durability":"SQLite FULL","rows":[4096,32768],"workers":[1,64],"ordering":"rotate modes each pass","corpus":"deterministic SHA256 hex"})
    );
    for pass in 1..=3 {
        for rows in [4096, 32768] {
            for workers in [1, 64] {
                let modes = ["off", "snapshot", "full", "guarded"];
                for offset in 0..4 {
                    cost(pass, rows, workers, modes[(offset + pass - 1) % 4], 256)?;
                }
            }
        }
    }
    Ok(())
}
pub(super) fn grouped() -> Result<()> {
    for pass in 1..=3 {
        for rows in [4096, 32768] {
            for offset in 0..3 {
                cost(
                    pass,
                    rows,
                    64,
                    ["off", "full", "guarded"][(offset + pass - 1) % 3],
                    4096,
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn checkpoint() -> Result<()> {
    for pass in 1..=3 {
        for rows in [4096, 32768] {
            for offset in 0..3 {
                cost(
                    pass,
                    rows,
                    64,
                    ["off", "guarded", "guarded4m"][(offset + pass - 1) % 3],
                    4096,
                )?;
            }
        }
    }
    Ok(())
}

fn cost(pass: usize, rows: usize, workers: usize, mode: &str, updates: usize) -> Result<()> {
    let dir = tempfile::tempdir()?;
    seed(dir.path(), rows)?;
    let source = if mode == "full" || mode.starts_with("guarded") {
        Some(Source::with_flush_limit(
            dir.path(),
            if mode == "full" {
                Mode::LogFull
            } else {
                Mode::WalGuarded
            },
            if mode == "guarded4m" {
                4 * 1024 * 1024
            } else {
                256 * 1024
            },
        )?)
    } else {
        None
    };
    let _registration = source
        .as_ref()
        .map(|s| register(dir.path(), s.clone()))
        .transpose()?;
    let store = Store::open(dir.path(), options())?;
    let db = store.database("app").open()?;
    let snapshot = if mode == "snapshot" {
        let mut node = crate::baseline::node::Node::open(dir.path())?;
        node.publish("app", "sql/app.db")?;
        node.capture("app")?;
        Some(Mutex::new(node))
    } else {
        None
    };
    let values: Vec<_> = (0..updates).map(|i| body(10000 + i)).collect();
    let barrier = Barrier::new(workers);
    let started = Instant::now();
    let times = std::thread::scope(|scope| -> Result<Vec<f64>> {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let db = &db;
                let values = &values;
                let barrier = &barrier;
                let snapshot = &snapshot;
                scope.spawn(move || -> Result<Vec<f64>> {
                    barrier.wait();
                    let mut times = Vec::new();
                    for index in (worker..updates).step_by(workers) {
                        // The old explicit scanner serializes write+capture, as in the previous harness.
                        let started = Instant::now();
                        let mut old = snapshot.as_ref().map(|s| s.lock().unwrap());
                        db.exec(
                            Sql::new("update documents set body=?1 where id=?2")
                                .bind(&values[index])
                                .bind(&index),
                        )?;
                        if let Some(node) = old.as_mut() {
                            node.capture("app")?;
                        }
                        times.push(started.elapsed().as_secs_f64() * 1e6);
                    }
                    Ok(times)
                })
            })
            .collect();
        let mut times = Vec::new();
        for h in handles {
            times.extend(h.join().map_err(|_| "writer panicked")??);
        }
        Ok(times)
    })?;
    let elapsed = started.elapsed().as_secs_f64();
    let path = dir.path().join("sql/app.db");
    let image = pages::Image::read(&mut pages::reader(&path)?)?;
    let (commits, captured_bytes, dirty_pages) = if let Some(source) = source {
        let f = source.feed(&path)?;
        let f = f.lock().unwrap();
        let mut actual = crate::tree::Tree::default();
        actual.apply(&image.pages, image.pages.len() as u32, image.page_size)?;
        require(
            actual.root() == f.tree.root(),
            "capture benchmark root mismatch",
        )?;
        (
            f.entries.len() - 1,
            f.entries.iter().skip(1).map(|e| e.raw.len()).sum::<usize>(),
            f.entries
                .iter()
                .skip(1)
                .map(|e| e.packet.pages.len())
                .sum::<usize>(),
        )
    } else {
        (0, 0, 0)
    };
    let objects: Vec<_> = pages::reader(&path)?
        .prepare("select name,sum(pgsize) from dbstat group by name")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut storage = std::collections::BTreeMap::new();
    for name in [
        "sql/app.db",
        "sql/app.db-wal",
        "replication.db",
        "replication.db-wal",
    ] {
        if let Ok(metadata) = std::fs::metadata(dir.path().join(name)) {
            storage.insert(name.to_owned(), metadata.len());
        }
    }
    if let Ok(files) = std::fs::read_dir(dir.path().join("feed")) {
        for file in files {
            let file = file?;
            storage.insert(
                format!("feed/{}", file.file_name().to_string_lossy()),
                file.metadata()?.len(),
            );
        }
    }
    println!(
        "{}",
        json!({"case":"capture","pass":pass,"mode":mode,"rows":rows,"workers":workers,"writes":updates,"seconds":elapsed,"writes_per_second":updates as f64/elapsed,"p50_us":percentile(&times,50),"p95_us":percentile(&times,95),"p99_us":percentile(&times,99),"file_bytes":image.pages.len()*image.page_size as usize,"objects":objects,"captured_commits":commits,"feed_raw_bytes":captured_bytes,"dirty_pages":dirty_pages,"storage_logical_bytes":storage,"verified":true})
    );
    store.close()?;
    Ok(())
}

fn transmit(stream: &mut TcpStream, bytes: &[u8]) -> Result<()> {
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(bytes)?;
    Ok(())
}
fn receive(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut len = [0; 4];
    stream.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    require(len <= 32 * 1024 * 1024, "network byte bound")?;
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub(super) fn network() -> Result<()> {
    for pass in 1..=3 {
        for hops in [1, 3] {
            for batch in [1, 32] {
                for durable in [false, true] {
                    for delay_ms in [0, 5] {
                        network_case(pass, hops, batch, durable, delay_ms, false, false)?;
                    }
                }
            }
        }
    }
    Ok(())
}
pub(super) fn pipeline() -> Result<()> {
    let modes = [(false, false), (false, true), (true, true)];
    for pass in 1..=3 {
        for batch in [1, 32] {
            for delay in [0, 5] {
                if batch == 32 && delay == 5 {
                    continue;
                }
                for offset in 0..3 {
                    let (early, background) = modes[(offset + pass - 1) % 3];
                    network_case(pass, 3, batch, true, delay, early, background)?;
                }
            }
        }
    }
    Ok(())
}

fn network_case(
    pass: usize,
    hops: usize,
    batch: usize,
    durable: bool,
    delay_ms: u64,
    early: bool,
    background: bool,
) -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("primary");
    seed(&root, 4096)?;
    let source = Source::new(&root, Mode::WalGuarded)?;
    let _registration = register(&root, source.clone())?;
    let store = Store::open(&root, options())?;
    let db = store.database("app").open()?;
    let feed = source.feed(&root.join("sql/app.db"))?;
    let listeners: Vec<_> = (0..hops)
        .map(|_| TcpListener::bind("127.0.0.1:0"))
        .collect::<std::io::Result<_>>()?;
    let addresses: Vec<_> = listeners
        .iter()
        .map(|l| l.local_addr())
        .collect::<std::io::Result<_>>()?;
    let mut workers = Vec::new();
    for (index, listener) in listeners.into_iter().enumerate() {
        let root = dir.path().join(format!("replica-{index}"));
        let next = addresses.get(index + 1).copied();
        workers.push(std::thread::spawn(move || -> Result<()> {
            let mut replica = replica::Replica::open(&root)?;
            let applier = background.then(|| replica::Applier::start(replica.path.clone()));
            let (mut upstream, _) = listener.accept()?;
            upstream.set_nodelay(true)?;
            let mut downstream = next.map(TcpStream::connect).transpose()?;
            if let Some(stream) = &downstream {
                stream.set_nodelay(true)?;
            }
            loop {
                let bytes = receive(&mut upstream)?;
                std::thread::sleep(Duration::from_millis(delay_ms));
                if bytes.is_empty() {
                    if let Some(stream) = &mut downstream {
                        transmit(stream, &[])?;
                    }
                    break;
                }
                if bytes == b"echo" {
                    if let Some(stream) = &mut downstream {
                        transmit(stream, &bytes)?;
                        let _ = receive(stream)?;
                    }
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    transmit(&mut upstream, b"echo")?;
                    continue;
                }
                let before = replica.feed.entries.len();
                if early {
                    replica.stage(&bytes)?;
                } else {
                    replica.receive(&bytes)?;
                }
                // Both schedules forward the original compressed bytes without recompression.
                if let Some(stream) = &mut downstream {
                    transmit(stream, &bytes)?;
                }
                if early {
                    replica.feed.sync()?;
                }
                if let Some(applier) = &applier {
                    applier.submit(replica.feed.entries[before..].to_vec())?;
                }
                if !durable {
                    replica.apply_since(before)?;
                }
                if let Some(stream) = &mut downstream {
                    let _ = receive(stream)?;
                }
                std::thread::sleep(Duration::from_millis(delay_ms));
                transmit(&mut upstream, b"ack")?;
                if durable && !background {
                    replica.apply_since(before)?;
                }
            }
            if let Some(applier) = applier {
                applier.finish()?;
            }
            replica.verify()?;
            Ok(())
        }));
    }
    let mut stream = TcpStream::connect(addresses[0])?;
    stream.set_nodelay(true)?;
    let mut echo = Vec::new();
    for _ in 0..128 {
        let start = Instant::now();
        transmit(&mut stream, b"echo")?;
        let _ = receive(&mut stream)?;
        echo.push(start.elapsed().as_secs_f64() * 1e6);
    }
    let initial = replica::pack(&feed.lock().unwrap().entries)?;
    transmit(&mut stream, &initial)?;
    let _ = receive(&mut stream)?;
    let values: Vec<_> = (0..128).map(|i| body(i + 20000)).collect();
    let mut capture = Vec::new();
    let mut encode = Vec::new();
    let mut ship = Vec::new();
    let mut endtoend = Vec::new();
    let mut wire = 0;
    let mut raw = 0;
    for base in (0..128).step_by(batch) {
        let before = feed.lock().unwrap().entries.len();
        let started = Instant::now();
        for (index, value) in values.iter().enumerate().skip(base).take(batch) {
            db.exec(
                Sql::new("update documents set body=?1 where id=?2")
                    .bind(value)
                    .bind(&index),
            )?;
        }
        capture.push(started.elapsed().as_secs_f64() * 1e6);
        let entries = feed.lock().unwrap().entries[before..].to_vec();
        raw += entries.iter().map(|e| e.raw.len()).sum::<usize>();
        let start = Instant::now();
        let bytes = replica::pack(&entries)?;
        encode.push(start.elapsed().as_secs_f64() * 1e6);
        wire += bytes.len() + 4;
        let start = Instant::now();
        transmit(&mut stream, &bytes)?;
        require(receive(&mut stream)? == b"ack", "replica ACK")?;
        ship.push(start.elapsed().as_secs_f64() * 1e6);
        endtoend.push(started.elapsed().as_secs_f64() * 1e6);
    }
    transmit(&mut stream, &[])?;
    for worker in workers {
        worker.join().map_err(|_| "replica panicked")??;
    }
    println!(
        "{}",
        json!({"case":"network","background_applier":background,"forward":if early {"after_append"} else {"after_sync"},"pass":pass,"hops":hops,"batch":batch,"updates":128,"tcp":"persistent loopback TCP_NODELAY","ack":if durable {"all hops durable"} else {"all hops durable and applied"},"injected_one_way_delay_ms":delay_ms,"echo_p50_us":percentile(&echo,50),"echo_p95_us":percentile(&echo,95),"capture_batch_p50_us":percentile(&capture,50),"encode_batch_p50_us":percentile(&encode,50),"ship_apply_batch_p50_us":percentile(&ship,50),"ship_apply_batch_p95_us":percentile(&ship,95),"end_to_end_batch_p50_us":percentile(&endtoend,50),"raw_bytes":raw,"wire_bytes_per_edge":wire,"all_edges_bytes":wire*hops,"all_replicas_verified":true})
    );
    store.close()?;
    Ok(())
}
