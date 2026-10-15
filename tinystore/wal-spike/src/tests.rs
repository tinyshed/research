use super::capture::{Mode, Source};
use super::*;
use std::path::Path;
use std::sync::Arc;
use tinystore::capture_experiment::{Registration, register};
use tinystore::{Options, Store};

struct Fixture {
    store: Store,
    db: tinystore::sql::Database,
    source: Arc<Source>,
    _registration: Registration,
}
fn fixture(path: &Path, mode: Mode) -> Result<Fixture> {
    let source = Source::new(path, mode)?;
    let registration = register(path, source.clone())?;
    let store = Store::open(
        path,
        Options {
            background: false,
            ..Default::default()
        },
    )?;
    let db = store.database("app").open()?;
    db.exec("create table if not exists items(id integer primary key,value blob)")?;
    Ok(Fixture {
        store,
        db,
        source,
        _registration: registration,
    })
}

fn verify(f: &Fixture) -> Result<()> {
    let path = f.store.dir().join("sql/app.db");
    let image = pages::Image::read(&mut pages::reader(&path)?)?;
    let mut tree = tree::Tree::default();
    tree.apply(&image.pages, image.pages.len() as u32, image.page_size)?;
    assert_eq!(
        tree.root(),
        f.source.feed(&path)?.lock().unwrap().tree.root()
    );
    Ok(())
}

#[test]
fn every_commit_reaches_the_feed_without_explicit_capture() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = fixture(dir.path(), Mode::WalGuarded)?;
    for _ in 0..100 {
        f.db.exec("insert into items(value) values(randomblob(128))")?;
    }
    verify(&f)?;
    let feed = f.source.feed(&f.store.dir().join("sql/app.db"))?;
    assert_eq!(feed.lock().unwrap().entries.len(), 102);
    Ok(())
}

#[test]
fn exclusive_transactions_and_rollbacks_preserve_the_commit_boundary() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = fixture(dir.path(), Mode::LogFull)?;
    let before = f
        .source
        .feed(&f.store.dir().join("sql/app.db"))?
        .lock()
        .unwrap()
        .entries
        .len();
    f.db.tx::<_, tinystore::Error>(|tx| {
        tx.exec("insert into items(value) values('one')")?;
        tx.exec("insert into items(value) values('two')")?;
        Ok(())
    })?;
    let _: tinystore::Result<()> = f.db.tx(|tx| {
        tx.exec("insert into items(value) values('rollback')")?;
        Err(tinystore::Error::invalid("rollback"))
    });
    verify(&f)?;
    assert_eq!(f.db.scalar::<u32>("select count(*) from items")?, 2);
    assert_eq!(
        f.source
            .feed(&f.store.dir().join("sql/app.db"))?
            .lock()
            .unwrap()
            .entries
            .len(),
        before + 1
    );
    Ok(())
}

#[test]
fn capture_survives_controlled_wal_generations() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = fixture(dir.path(), Mode::WalGuarded)?;
    for _ in 0..200 {
        f.db.exec("insert into items(value) values(zeroblob(4096))")?;
    }
    verify(&f)?;
    let feed = f.source.feed(&f.store.dir().join("sql/app.db"))?;
    let locked = feed.lock().unwrap();
    let generations: std::collections::BTreeSet<_> =
        locked.entries.iter().map(|e| e.cursor.salts).collect();
    assert!(generations.len() > 2);
    drop(locked);
    drop(feed);
    f.store.close()?;
    drop(f);
    let reopened = fixture(dir.path(), Mode::WalGuarded)?;
    verify(&reopened)?;
    assert_eq!(
        reopened.db.scalar::<u32>("select count(*) from items")?,
        200
    );
    Ok(())
}

#[test]
fn merkle_root_has_a_canonical_width_after_shrink() -> Result<()> {
    let mut pages = pages::Pages::new();
    for number in 1..=9 {
        pages.insert(number, vec![number as u8; 512]);
    }
    let mut tree = tree::Tree::default();
    tree.apply(&pages, 9, 512)?;
    pages.retain(|n, _| *n <= 3);
    tree.apply(&pages, 3, 512)?;
    let mut rebuilt = tree::Tree::default();
    rebuilt.apply(&pages, 3, 512)?;
    assert_eq!(tree.root(), rebuilt.root());
    Ok(())
}

#[test]
fn capture_prefix_is_recovered_after_process_death() -> Result<()> {
    for mode in ["full", "guarded"] {
        for phase in [
            "after_sqlite_commit",
            "after_log_append",
            "after_log_sync",
            "after_ack",
            "before_close_checkpoint",
        ] {
            if mode == "guarded" && phase == "after_log_sync" {
                continue;
            }
            let dir = tempfile::tempdir()?;
            let status = std::process::Command::new(std::env::current_exe()?)
                .args(["--ignored", "--exact", "tests::crash_worker"])
                .env("WAL_RESEARCH_ROOT", dir.path())
                .env("WAL_RESEARCH_MODE", mode)
                .env("WAL_RESEARCH_PHASE", phase)
                .stdout(std::process::Stdio::null())
                .status()?;
            assert_eq!(status.code(), Some(86), "{mode} {phase}");
            let f = fixture(
                dir.path(),
                if mode == "full" {
                    Mode::LogFull
                } else {
                    Mode::WalGuarded
                },
            )?;
            verify(&f)?;
            assert_eq!(
                f.db.scalar::<u32>("select count(*) from items")?,
                1,
                "{mode} {phase}"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "subprocess fault-injection worker"]
fn crash_worker() -> Result<()> {
    let root = std::env::var_os("WAL_RESEARCH_ROOT").ok_or("missing root")?;
    let phase = std::env::var("WAL_RESEARCH_PHASE")?;
    let mode = if std::env::var("WAL_RESEARCH_MODE")? == "full" {
        Mode::LogFull
    } else {
        Mode::WalGuarded
    };
    let f = fixture(Path::new(&root), mode)?;
    f.source.arm(&phase);
    if phase == "before_checkpoint" || phase == "after_checkpoint" {
        for _ in 0..1000 {
            f.db.exec("insert into items(value) values(zeroblob(4096))")?;
        }
        return Err("checkpoint fault not reached".into());
    }
    f.db.exec("insert into items(value) values('survives')")?;
    if phase == "after_ack" {
        std::fs::write(Path::new(&root).join("ack"), b"1")?;
        std::process::exit(86);
    }
    f.store.close()?;
    Err("fault point was not reached".into())
}

#[test]
fn guarded_rebuilds_a_lost_unsynced_feed_tail_from_durable_wal() -> Result<()> {
    for lost_bytes in [0, 1, 37, 4096, usize::MAX] {
        let dir = tempfile::tempdir()?;
        let status = std::process::Command::new(std::env::current_exe()?)
            .args(["--ignored", "--exact", "tests::crash_worker"])
            .env("WAL_RESEARCH_ROOT", dir.path())
            .env("WAL_RESEARCH_MODE", "guarded")
            .env("WAL_RESEARCH_PHASE", "after_ack")
            .stdout(std::process::Stdio::null())
            .status()?;
        assert_eq!(status.code(), Some(86));
        let path = std::fs::read_dir(dir.path().join("feed"))?
            .next()
            .ok_or("feed missing")??
            .path();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        use std::io::Read;
        let mut bytes = Vec::new();
        (&file).read_to_end(&mut bytes)?;
        let mut offset = 8;
        let mut boundaries = Vec::new();
        while offset < bytes.len() {
            boundaries.push(offset);
            let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into()?) as usize;
            offset += 36 + len;
        }
        let start = if lost_bytes == usize::MAX {
            boundaries[1]
        } else {
            *boundaries.last().ok_or("record missing")?
        };
        if lost_bytes == 0 {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = &file;
            file.seek(SeekFrom::End(-1))?;
            file.write_all(&[bytes[bytes.len() - 1] ^ 1])?;
        } else {
            file.set_len(bytes.len().saturating_sub(lost_bytes).max(start) as u64)?;
        }
        file.sync_all()?;
        drop(file);
        let f = fixture(dir.path(), Mode::WalGuarded)?;
        verify(&f)?;
        assert_eq!(f.db.scalar::<u32>("select count(*) from items")?, 1);
    }
    Ok(())
}

#[test]
fn checkpoint_crash_retains_a_complete_feed_prefix() -> Result<()> {
    for mode in ["full", "guarded"] {
        for phase in ["before_checkpoint", "after_checkpoint"] {
            let dir = tempfile::tempdir()?;
            let status = std::process::Command::new(std::env::current_exe()?)
                .args(["--ignored", "--exact", "tests::crash_worker"])
                .env("WAL_RESEARCH_ROOT", dir.path())
                .env("WAL_RESEARCH_MODE", mode)
                .env("WAL_RESEARCH_PHASE", phase)
                .stdout(std::process::Stdio::null())
                .status()?;
            assert_eq!(status.code(), Some(86));
            let f = fixture(
                dir.path(),
                if mode == "full" {
                    Mode::LogFull
                } else {
                    Mode::WalGuarded
                },
            )?;
            verify(&f)?;
            assert!(f.db.scalar::<u32>("select count(*) from items")? > 0);
        }
    }
    Ok(())
}

#[test]
fn durable_receiver_replays_after_death_before_apply_and_accepts_retries() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = fixture(&dir.path().join("primary"), Mode::LogFull)?;
    for _ in 0..40 {
        f.db.exec("insert into items(value) values(randomblob(128))")?;
    }
    let feed = f.source.feed(&f.store.dir().join("sql/app.db"))?;
    let bytes = replica::pack(&feed.lock().unwrap().entries)?;
    let root = dir.path().join("child");
    let mut status = std::process::Command::new(std::env::current_exe()?);
    status
        .args(["--ignored", "--exact", "tests::receiver_crash_worker"])
        .env("WAL_RECEIVER_ROOT", &root)
        .env("WAL_RECEIVER_BATCH", dir.path().join("batch"))
        .stdout(std::process::Stdio::null());
    std::fs::write(dir.path().join("batch"), &bytes)?;
    let status = status.status()?;
    assert_eq!(status.code(), Some(86));
    let mut child = replica::Replica::open(&root)?;
    child.verify()?;
    assert_eq!(
        pages::reader(&child.path)?
            .query_row("select count(*) from items", [], |r| r.get::<_, u32>(0))?,
        40
    );
    let count = child.feed.entries.len();
    child.receive(&bytes)?;
    assert_eq!(child.feed.entries.len(), count);
    assert_eq!(child.feed.tree.root(), feed.lock().unwrap().tree.root());
    Ok(())
}

#[test]
#[ignore = "subprocess receiver fault worker"]
fn receiver_crash_worker() -> Result<()> {
    let mut replica = replica::Replica::open(Path::new(
        &std::env::var_os("WAL_RECEIVER_ROOT").ok_or("root")?,
    ))?;
    replica.receive(&std::fs::read(
        std::env::var_os("WAL_RECEIVER_BATCH").ok_or("batch")?,
    )?)?;
    std::process::exit(86);
}

#[test]
fn durable_standby_survives_primary_directory_loss() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("primary");
    let f = fixture(&root, Mode::WalGuarded)?;
    f.db.exec("insert into items(value) values('ack-after-standby-sync')")?;
    let feed = f.source.feed(&f.store.dir().join("sql/app.db"))?;
    let bytes = replica::pack(&feed.lock().unwrap().entries)?;
    let mut child = replica::Replica::open(&dir.path().join("standby"))?;
    child.receive(&bytes)?; // A synchronous client may ACK only after this durable receipt.
    drop(f);
    std::fs::remove_dir_all(root)?;
    drop(child);
    let child = replica::Replica::open(&dir.path().join("standby"))?;
    child.verify()?;
    assert_eq!(
        pages::reader(&child.path)?
            .query_row("select count(*) from items", [], |r| r.get::<_, u32>(0))?,
        1
    );
    Ok(())
}

#[test]
fn wal_validation_rejects_damaged_committed_bytes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = fixture(dir.path(), Mode::WalGuarded)?;
    f.db.exec("insert into items(value) values('checksum')")?;
    let path = dir.path().join("sql/app.db");
    let wal_path = path.with_file_name("app.db-wal");
    let bytes = std::fs::read(&wal_path)?;
    let count = ((bytes.len() - 32) / (24 + 4096)) as u32;
    for offset in [4, 24, 48, 100] {
        let mut damaged = bytes.clone();
        damaged[offset] ^= 1;
        std::fs::write(&wal_path, &damaged)?;
        assert!(wal::scan(&path, wal::Cursor::default(), Some(count)).is_err());
    }
    std::fs::write(wal_path, bytes)?;
    verify(&f)?;
    Ok(())
}

#[test]
fn grouped_callers_are_captured_and_materialized_with_schema_changes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = fixture(&dir.path().join("primary"), Mode::WalGuarded)?;
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..64)
            .map(|_| {
                scope.spawn(|| {
                    for _ in 0..8 {
                        f.db.exec("insert into items(value) values('grouped')")
                            .unwrap();
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    });
    for index in 0..100 {
        f.db.exec(format!(
            "create table new_{index}(id integer primary key,value blob)"
        ))?;
    }
    verify(&f)?;
    let feed = f.source.feed(&f.store.dir().join("sql/app.db"))?;
    let entries = feed.lock().unwrap().entries.clone();
    let mut child = replica::Replica::open(&dir.path().join("child"))?;
    for batch in entries.chunks(32) {
        let before = child.feed.entries.len();
        child.receive(&replica::pack(batch)?)?;
        child.apply_since(before)?;
    }
    child.verify()?;
    assert_eq!(
        pages::reader(&child.path)?
            .query_row("select count(*) from items", [], |r| r.get::<_, u32>(0))?,
        512
    );
    assert_eq!(
        pages::reader(&child.path)?.query_row(
            "select count(*) from sqlite_schema where name like 'new_%'",
            [],
            |r| r.get::<_, u32>(0)
        )?,
        100
    );
    Ok(())
}

#[test]
fn missing_published_wal_and_gapped_or_conflicting_batches_fail_closed() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(
        wal::scan(
            &dir.path().join("missing.db"),
            wal::Cursor::default(),
            Some(1)
        )
        .is_err()
    );
    let f = fixture(&dir.path().join("primary"), Mode::LogFull)?;
    f.db.exec("insert into items(value) values('one')")?;
    let feed = f.source.feed(&f.store.dir().join("sql/app.db"))?;
    let entries = feed.lock().unwrap().entries.clone();
    let mut child = replica::Replica::open(&dir.path().join("child"))?;
    assert!(child.receive(&replica::pack(&entries[1..])?).is_err());
    drop(child);
    let mut child = replica::Replica::open(&dir.path().join("valid-child"))?;
    child.receive(&replica::pack(&entries)?)?;
    let mut conflicting = feed::decode(entries[0].raw.as_ref().clone())?;
    conflicting.raw = std::sync::Arc::new({
        let mut raw = entries[0].raw.as_ref().clone();
        raw[0] ^= 1;
        raw
    });
    assert!(
        child
            .receive(&replica::pack(&[std::sync::Arc::new(conflicting)])?)
            .is_err()
    );
    Ok(())
}

#[test]
fn weaker_sqlite_durability_is_rejected() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source = Source::new(dir.path(), Mode::WalGuarded)?;
    let _registration = register(dir.path(), source)?;
    let store = Store::open(
        dir.path(),
        Options {
            background: false,
            durability: Some(tinystore::Durability::Os),
            ..Default::default()
        },
    )?;
    assert!(store.database("app").open().is_err());
    Ok(())
}

#[test]
fn sigkill_never_loses_the_acknowledged_prefix() -> Result<()> {
    use std::io::{BufRead, BufReader};
    for mode in ["full", "guarded"] {
        for target in [1, 8, 31, 64] {
            let dir = tempfile::tempdir()?;
            let mut child = std::process::Command::new(std::env::current_exe()?)
                .args([
                    "--ignored",
                    "--exact",
                    "tests::sigkill_worker",
                    "--nocapture",
                ])
                .env("WAL_RESEARCH_ROOT", dir.path())
                .env("WAL_RESEARCH_MODE", mode)
                .stdout(std::process::Stdio::piped())
                .spawn()?;
            let mut output = BufReader::new(child.stdout.take().ok_or("missing output")?);
            loop {
                let mut line = String::new();
                require(
                    output.read_line(&mut line)? > 0,
                    "worker stopped before ACK",
                )?;
                if line.trim().ends_with(&format!("ACK {target}")) {
                    break;
                }
            }
            child.kill()?;
            let status = child.wait()?;
            assert!(!status.success());
            let f = fixture(
                dir.path(),
                if mode == "full" {
                    Mode::LogFull
                } else {
                    Mode::WalGuarded
                },
            )?;
            verify(&f)?;
            let rows = f.db.scalar::<u32>("select count(*) from items")?;
            assert!(
                rows >= target,
                "{mode}: acknowledged {target}, recovered {rows}"
            );
            for id in 1..=target {
                assert_eq!(
                    f.db.scalar::<u32>(
                        tinystore::sql::Sql::new("select count(*) from items where id=?1")
                            .bind(&id)
                    )?,
                    1
                );
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "SIGKILL worker controlled by parent"]
fn sigkill_worker() -> Result<()> {
    use std::io::Write;
    let root = std::env::var_os("WAL_RESEARCH_ROOT").ok_or("missing root")?;
    let mode = if std::env::var("WAL_RESEARCH_MODE")? == "full" {
        Mode::LogFull
    } else {
        Mode::WalGuarded
    };
    let f = fixture(Path::new(&root), mode)?;
    for id in 1..=100000 {
        f.db.exec(
            tinystore::sql::Sql::new("insert into items(id,value) values(?1,'acknowledged')")
                .bind(&id),
        )?;
        println!("ACK {id}");
        std::io::stdout().flush()?;
    }
    Err("parent did not stop worker".into())
}
