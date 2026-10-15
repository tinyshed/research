use crate::capture::{Config, Source};
use crate::{Result, pages, require, tree};
use serde_json::json;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
use tinystore::capture_experiment::{Registration, register};
use tinystore::sql::Sql;
use tinystore::{Options, Store};

pub(super) fn options() -> Options {
    Options {
        background: false,
        ..Default::default()
    }
}
pub(super) fn seed(root: &Path, rows: usize) -> Result<()> {
    let store = Store::open(root, options())?;
    let db = store.database("app").open()?;
    db.exec("create table documents(id integer primary key,version integer not null,body blob not null)")?;
    let values = [vec![b'a'; 256], vec![b'b'; 256]];
    db.tx::<_, tinystore::Error>(|tx| {
        for i in 0..rows {
            tx.exec(
                Sql::new("insert into documents values(?1,0,?2)")
                    .bind(&i)
                    .bind(&values[i % 2]),
            )?;
        }
        Ok(())
    })?;
    store.close()?;
    Ok(())
}
pub(super) fn configuration(mode: &str) -> Result<Option<Config>> {
    let mut c = Config::default();
    match mode {
        "off" => return Ok(None),
        "legacy" => {
            c.batched = false;
            c.owned = false;
            c.cache_bytes = usize::MAX;
            c.passive = false;
        }
        "bounded" => {
            c.batched = false;
            c.owned = false;
            c.passive = false;
        }
        "hash" => {
            c.owned = false;
            c.passive = false;
        }
        "copy" => {
            c.batched = false;
            c.passive = false;
        }
        "combined" => {
            c.passive = false;
        }
        "optimized" => {}
        "full" => {
            c.log_full = true;
        }
        _ => return Err("unknown mode".into()),
    }
    Ok(Some(c))
}
pub(super) struct Opened {
    pub store: Store,
    pub db: tinystore::sql::Database,
    pub source: Option<Arc<Source>>,
    pub _registration: Option<Registration>,
}
pub(super) fn open(root: &Path, config: Option<Config>) -> Result<Opened> {
    let source = config.map(|c| Source::new(root, c)).transpose()?;
    let registration = source
        .as_ref()
        .map(|s| register(root, s.clone()))
        .transpose()?;
    let store = Store::open(root, options())?;
    let db = store.database("app").open()?;
    Ok(Opened {
        store,
        db,
        source,
        _registration: registration,
    })
}
pub(super) fn verify(f: &Opened) -> Result<()> {
    if let Some(source) = &f.source {
        let path = f.store.dir().join("sql/app.db");
        let image = pages::Image::read(&mut pages::reader(&path)?)?;
        let mut actual = tree::Tree::default();
        actual.apply(&image.pages, image.pages.len() as u32, image.page_size)?;
        require(
            actual.root() == source.feed(&path)?.lock().unwrap().tree.root(),
            "source root mismatch",
        )?;
    }
    require(
        f.db.scalar::<String>("pragma integrity_check")? == "ok",
        "SQLite integrity",
    )
}
#[repr(align(64))]
#[derive(Default)]
struct Counters {
    ok: AtomicU64,
    denied: AtomicU64,
}
struct Histo(Vec<u64>);
impl Histo {
    fn new() -> Self {
        Self(vec![0; 4353])
    }
    fn put(&mut self, start: Instant) {
        let us = start.elapsed().as_micros() as usize;
        let i = if us < 4096 {
            us
        } else {
            (4096 + (us - 4096) / 4096).min(4352)
        };
        self.0[i] += 1;
    }
    fn add(&mut self, other: &Self) {
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            *a += b;
        }
    }
    fn percentile(&self, p: u64) -> u64 {
        let n = self.0.iter().sum::<u64>() * p;
        let mut sum = 0;
        for (i, v) in self.0.iter().enumerate() {
            sum += v * 100;
            if sum >= n {
                return if i < 4096 {
                    i as u64
                } else {
                    4096 + (i as u64 - 4096) * 4096
                };
            }
        }
        0
    }
}
fn proc_value(name: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(name))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0)
        * 1024
}
fn cpu_ticks() -> u64 {
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            s.rsplit_once(") ").map(|(_, v)| {
                v.split_whitespace()
                    .skip(11)
                    .take(2)
                    .filter_map(|s| s.parse::<u64>().ok())
                    .sum()
            })
        })
        .unwrap_or(0)
}
fn cgroup_memory() -> u64 {
    std::fs::read_to_string("/sys/fs/cgroup/memory.current")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub(super) fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    require(
        args.len() >= 3,
        "usage: tinystore-wal-load MODE SECONDS [long-reader-ms]",
    )?;
    let mode = &args[1];
    let seconds: u64 = args[2].parse()?;
    let long_ms: u64 = args.get(3).map(|v| v.parse()).transpose()?.unwrap_or(100);
    require(seconds > 0 && long_ms <= 1000, "invalid duration")?;
    let config = configuration(mode)?;
    let dir = tempfile::tempdir()?;
    seed(dir.path(), 32768)?;
    let f = open(dir.path(), config.clone())?;
    let path = f.store.dir().join("sql/app.db");
    let counters: Vec<_> = (0..72).map(|_| Counters::default()).collect();
    let barrier = Barrier::new(74);
    let stop = std::sync::atomic::AtomicBool::new(false);
    let warm_end = Instant::now() + Duration::from_secs(2);
    let result = std::thread::scope(|scope| -> Result<()> {
        let mut handles = Vec::new();
        for worker in 0..72 {
            let db = &f.db;
            let barrier = &barrier;
            let counters = &counters;
            let stop = &stop;
            handles.push(scope.spawn(move || -> Result<(bool, Histo)> {
                let writer = worker < 64;
                let mut state = worker as u64 + 1;
                let values = [vec![b'a'; 256], vec![b'b'; 256]];
                let mut hist = Histo::new();
                for timed in [false, true] {
                    if timed {
                        barrier.wait();
                        barrier.wait();
                    }
                    loop {
                        if if timed {
                            stop.load(Ordering::Relaxed)
                        } else {
                            Instant::now() >= warm_end
                        } {
                            break;
                        }
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let start = Instant::now();
                        if writer {
                            let id = (state >> 32) as usize % 32;
                            let result = db.exec(
                                Sql::new(
                                    "update documents set version=version+1,body=?1 where id=?2",
                                )
                                .bind(&values[(state & 1) as usize])
                                .bind(&id),
                            );
                            match result {
                                Ok(_) => {
                                    if timed {
                                        counters[worker].ok.fetch_add(1, Ordering::Relaxed);
                                        hist.put(start);
                                    }
                                }
                                Err(e) if e.kind() == tinystore::ErrorKind::Unavailable => {
                                    if timed {
                                        counters[worker].denied.fetch_add(1, Ordering::Relaxed);
                                    }
                                    std::thread::sleep(Duration::from_millis(1));
                                }
                                Err(e) => return Err(e.into()),
                            }
                        } else {
                            let id = (state >> 32) as usize % 32768;
                            if state.is_multiple_of(64) {
                                let _: u32 = db.scalar(
                                    Sql::new(
                                        "select sum(version) from documents where id>=?1 and id<?2",
                                    )
                                    .bind(&id)
                                    .bind(&(id + 128)),
                                )?;
                            } else {
                                let bytes: u32 = db.scalar(
                                    Sql::new("select length(body) from documents where id=?1")
                                        .bind(&id),
                                )?;
                                require(bytes == 256, "torn value")?;
                            }
                            if timed {
                                counters[worker].ok.fetch_add(1, Ordering::Relaxed);
                                hist.put(start);
                            }
                        }
                    }
                }
                Ok((writer, hist))
            }));
        }
        let barrier_ref = &barrier;
        let stop_ref = &stop;
        let path_ref = &path;
        let long = scope.spawn(move || -> Result<()> {
            for timed in [false, true] {
                if timed {
                    barrier_ref.wait();
                    barrier_ref.wait();
                }
                let mut next = Instant::now();
                loop {
                    if if timed {
                        stop_ref.load(Ordering::Relaxed)
                    } else {
                        Instant::now() >= warm_end
                    } {
                        break;
                    }
                    if long_ms > 0 && Instant::now() >= next {
                        let reader = pages::reader(path_ref)?;
                        reader.execute_batch("begin")?;
                        let _: u32 =
                            reader.query_row("select count(*) from documents", [], |r| r.get(0))?;
                        std::thread::sleep(Duration::from_millis(long_ms));
                        reader.execute_batch("rollback")?;
                        next = Instant::now() + Duration::from_secs(2);
                    } else {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            Ok(())
        });
        barrier.wait();
        if let Some(source) = &f.source {
            source.feed(&path)?.lock().unwrap().reset_stats();
        }
        let warm_versions = f.db.scalar::<u32>("select sum(version) from documents")?;
        std::fs::write(dir.path().join("timed-begin"), [])?;
        let cpu_start = cpu_ticks();
        let started = Instant::now();
        barrier.wait();
        let mut last = (0, 0);
        let mut max_rss = 0;
        for second in 1..=seconds {
            let target = started + Duration::from_secs(second);
            if let Some(delay) = target.checked_duration_since(Instant::now()) {
                std::thread::sleep(delay);
            }
            let writes = counters[..64]
                .iter()
                .map(|c| c.ok.load(Ordering::Relaxed))
                .sum::<u64>();
            let reads = counters[64..]
                .iter()
                .map(|c| c.ok.load(Ordering::Relaxed))
                .sum::<u64>();
            let rss = proc_value("VmRSS:");
            max_rss = max_rss.max(rss);
            let (cache, log, stats) = if let Some(source) = &f.source {
                let h = source.feed(&path)?;
                let h = h.lock().unwrap();
                (h.cache, h.length, h.stats.json())
            } else {
                (0, 0, json!(null))
            };
            println!(
                "{}",
                json!({"case":"sample","mode":mode,"second":second,"elapsed_seconds":started.elapsed().as_secs_f64(),"write_delta":writes-last.0,"read_delta":reads-last.1,
                "rss_bytes":rss,"hwm_bytes":proc_value("VmHWM:"),"cgroup_bytes":cgroup_memory(),"wal_bytes":Source::wal_bytes(&path),"cache_reserved":cache,"feed_bytes":log,"stats":stats})
            );
            last = (writes, reads);
        }
        stop.store(true, Ordering::Relaxed);
        let mut wh = Histo::new();
        let mut rh = Histo::new();
        for h in handles {
            let (writer, hist) = h.join().map_err(|_| "worker panic")??;
            if writer { wh.add(&hist) } else { rh.add(&hist) }
        }
        long.join().map_err(|_| "reader panic")??;
        let elapsed = started.elapsed().as_secs_f64();
        let cpu = cpu_ticks() - cpu_start;
        std::fs::write(dir.path().join("timed-end"), [])?;
        let writes = counters[..64]
            .iter()
            .map(|c| c.ok.load(Ordering::Relaxed))
            .sum::<u64>();
        let reads = counters[64..]
            .iter()
            .map(|c| c.ok.load(Ordering::Relaxed))
            .sum::<u64>();
        let denied = counters
            .iter()
            .map(|c| c.denied.load(Ordering::Relaxed))
            .sum::<u64>();
        require(
            f.db.scalar::<u32>("select sum(version) from documents")? as u64
                == warm_versions as u64 + writes,
            "ACK count mismatch",
        )?;
        verify(&f)?;
        let (stats, leaves, branches, tree_bytes, cache_payload, log) =
            if let Some(source) = &f.source {
                let h = source.feed(&path)?;
                let h = h.lock().unwrap();
                (
                    h.stats.json(),
                    h.tree.leaves,
                    h.tree.branches,
                    h.tree.bytes(),
                    h.cached_payload(),
                    h.length,
                )
            } else {
                (json!(null), 0, 0, 0, 0, 0)
            };
        let objects: Vec<_> = pages::reader(&path)?
            .prepare("select name,sum(pgsize) from dbstat group by name")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        println!(
            "{}",
            json!({"case":"result","mode":mode,"seconds":elapsed,"requested_seconds":seconds,"warm_seconds":2,"long_reader_ms":long_ms,
            "writers":64,"readers":8,"fixture_rows":32768,"hot_keys":32,"body_bytes":256,"writes":writes,"reads":reads,"denied":denied,
            "write_rate":writes as f64/elapsed,"read_rate":reads as f64/elapsed,"write_p50_us":wh.percentile(50),"write_p95_us":wh.percentile(95),"write_p99_us":wh.percentile(99),
            "read_p50_us":rh.percentile(50),"read_p95_us":rh.percentile(95),"read_p99_us":rh.percentile(99),"cpu_ticks":cpu,"clock_ticks_per_second":100,
            "max_rss_bytes":max_rss,"hwm_bytes":proc_value("VmHWM:"),"stats":stats,"leaf_hashes":leaves,"branch_hashes":branches,"tree_bytes":tree_bytes,
            "cache_payload_bytes":cache_payload,"feed_bytes":log,"objects":objects,"verified":true,"durability":"SQLite FULL"})
        );
        Ok(())
    });
    f.store.close()?;
    result
}
