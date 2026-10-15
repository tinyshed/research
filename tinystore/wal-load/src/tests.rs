use crate::*;
#[test]
fn batched_tree_matches_legacy_through_growth_and_shrink() -> Result<()> {
    let mut a = tree::Tree::default();
    let mut b = tree::Tree::default();
    b.batched = true;
    for count in [1, 5, 17, 40, 9, 33, 2, 1] {
        let mut pages = pages::Pages::new();
        for n in 1..=count {
            pages.insert(n, vec![n as u8; 512]);
        }
        a.apply(&pages, count, 512)?;
        b.apply(&pages, count, 512)?;
        assert_eq!(a.root(), b.root());
    }
    assert!(b.branches < a.branches);
    Ok(())
}
#[test]
fn bounded_history_replays_without_retained_seed() -> Result<()> {
    let dir = tempfile::tempdir()?;
    load::seed(dir.path(), 2048)?;
    let cfg = capture::Config {
        cache_bytes: 64 * 1024,
        ..Default::default()
    };
    let f = load::open(dir.path(), Some(cfg.clone()))?;
    for i in 0..300 {
        f.db.exec(
            tinystore::sql::Sql::new("update documents set version=version+1 where id=?1")
                .bind(&(i % 32)),
        )?;
    }
    load::verify(&f)?;
    let h = f
        .source
        .as_ref()
        .unwrap()
        .feed(&f.store.dir().join("sql/app.db"))?;
    assert!(h.lock().unwrap().cache <= cfg.cache_bytes);
    f.store.close()?;
    drop(h);
    drop(f);
    let f = load::open(dir.path(), Some(cfg))?;
    load::verify(&f)?;
    assert_eq!(
        f.db.scalar::<u32>("select sum(version) from documents")?,
        300
    );
    Ok(())
}

#[test]
fn pinned_reader_gets_definite_admission_refusal_then_resumes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    load::seed(dir.path(), 2048)?;
    let cfg = capture::Config {
        wal_limit: 256 * 1024,
        reserve: 64 * 1024,
        frames: 16,
        cache_bytes: 64 * 1024,
        ..Default::default()
    };
    let f = load::open(dir.path(), Some(cfg.clone()))?;
    f.db.exec("update documents set version=version+1 where id=0")?;
    let path = f.store.dir().join("sql/app.db");
    let reader = pages::reader(&path)?;
    reader.execute_batch("begin")?;
    let _: u32 = reader.query_row("select sum(version) from documents", [], |r| r.get(0))?;
    let mut ack = 1;
    let mut refused = false;
    for _ in 0..200 {
        match f
            .db
            .exec("update documents set version=version+1 where id=0")
        {
            Ok(_) => ack += 1,
            Err(e) => {
                assert_eq!(e.kind(), tinystore::ErrorKind::Unavailable);
                refused = true;
                break;
            }
        }
    }
    assert!(refused);
    assert!(capture::Source::wal_bytes(&path) <= cfg.wal_limit);
    assert_eq!(
        f.db.scalar::<u32>("select sum(version) from documents")?,
        ack
    );
    load::verify(&f)?;
    reader.execute_batch("rollback")?;
    drop(reader);
    f.db.exec("update documents set version=version+1 where id=0")?;
    load::verify(&f)?;
    assert_eq!(
        f.db.scalar::<u32>("select sum(version) from documents")?,
        ack + 1
    );
    Ok(())
}

#[test]
fn feed_disk_admission_refuses_without_committing() -> Result<()> {
    let dir = tempfile::tempdir()?;
    load::seed(dir.path(), 64)?;
    let cfg = capture::Config {
        disk_limit: 256 * 1024,
        reserve: 32 * 1024,
        ..Default::default()
    };
    let f = load::open(dir.path(), Some(cfg))?;
    let mut ack = 0;
    let mut refused = false;
    for _ in 0..500 {
        match f
            .db
            .exec("update documents set version=version+1 where id=0")
        {
            Ok(_) => ack += 1,
            Err(e) => {
                assert_eq!(e.kind(), tinystore::ErrorKind::Unavailable);
                refused = true;
                break;
            }
        }
    }
    assert!(refused);
    assert_eq!(
        f.db.scalar::<u32>("select sum(version) from documents")?,
        ack
    );
    load::verify(&f)
}

#[test]
fn oversized_transaction_is_unknown_and_recoverable() -> Result<()> {
    let dir = tempfile::tempdir()?;
    load::seed(dir.path(), 2048)?;
    let f = load::open(
        dir.path(),
        Some(capture::Config {
            reserve: 8192,
            ..Default::default()
        }),
    )?;
    assert_eq!(
        f.db.exec("update documents set version=version+1")
            .unwrap_err()
            .kind(),
        tinystore::ErrorKind::OutcomeUnknown
    );
    assert_eq!(
        f.db.exec("update documents set version=version+1 where id=0")
            .unwrap_err()
            .kind(),
        tinystore::ErrorKind::Unavailable
    );
    drop(f);
    let f = load::open(dir.path(), Some(capture::Config::default()))?;
    load::verify(&f)?;
    assert_eq!(
        f.db.scalar::<u32>("select sum(version) from documents")?,
        2048
    );
    Ok(())
}

#[test]
fn optimized_and_control_recover_across_process_death() -> Result<()> {
    for mode in ["legacy", "optimized", "full"] {
        for phase in [
            "sqlite_commit",
            "append",
            "before_checkpoint",
            "after_checkpoint",
            "ack",
            "sync",
        ] {
            if phase == "sync" && mode != "full" {
                continue;
            }
            let dir = tempfile::tempdir()?;
            let status = std::process::Command::new(std::env::current_exe()?)
                .args(["--ignored", "--exact", "tests::crash_worker"])
                .env("WAL_LOAD_ROOT", dir.path())
                .env("WAL_LOAD_MODE", mode)
                .env("WAL_LOAD_FAULT", phase)
                .stdout(std::process::Stdio::null())
                .status()?;
            assert_eq!(status.code(), Some(86), "{mode} {phase}");
            let f = load::open(dir.path(), load::configuration(mode)?)?;
            load::verify(&f)?;
            assert!(f.db.scalar::<u32>("select sum(version) from documents")? > 0);
        }
    }
    Ok(())
}

#[test]
#[ignore = "controlled abrupt worker"]
fn crash_worker() -> Result<()> {
    let root = std::env::var_os("WAL_LOAD_ROOT").ok_or("root")?;
    let root = std::path::Path::new(&root);
    load::seed(root, 2048)?;
    let mut cfg = load::configuration(&std::env::var("WAL_LOAD_MODE")?)?.ok_or("mode")?;
    cfg.frames = 4;
    let f = load::open(root, Some(cfg))?;
    for _ in 0..16 {
        f.db.exec("update documents set version=version+1 where id=0")?;
        if std::env::var("WAL_LOAD_FAULT")? == "ack" {
            std::process::exit(86);
        }
    }
    Err("fault not reached".into())
}

#[test]
fn owned_codec_and_batched_tree_preserve_exact_feed_bytes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let a_path = dir.path().join("a");
    let b_path = dir.path().join("b");
    let mut a = feed::Feed::open(&a_path, 64 * 1024, false, false)?;
    let mut b = feed::Feed::open(&b_path, 64 * 1024, true, true)?;
    for seq in 1..=30 {
        let mut pages = pages::Pages::new();
        for n in 1..=if seq == 1 { 16 } else { 3 } {
            pages.insert(n, vec![seq as u8; 512]);
        }
        let packet = packet::Packet {
            stream: "file".into(),
            snapshot: seq == 1,
            position: packet::Position {
                epoch: 1,
                sequence: seq,
                checksum: [0; 32],
            },
            previous: a.tree.root(),
            page_size: 512,
            page_count: 16,
            pages,
        };
        let cursor = wal::Cursor {
            salts: [3; 8],
            frame: seq as u32,
            checksum: [4, 5],
        };
        a.append(packet.clone(), cursor)?;
        b.append(packet, cursor)?;
        assert_eq!(a.tree.root(), b.tree.root());
    }
    assert_eq!(std::fs::read(a_path)?, std::fs::read(b_path)?);
    Ok(())
}

#[test]
fn zero_cache_keeps_sequence_and_recovers() -> Result<()> {
    let dir = tempfile::tempdir()?;
    load::seed(dir.path(), 2048)?;
    let cfg = load::configuration("nocache")?.ok_or("config")?;
    let f = load::open(dir.path(), Some(cfg.clone()))?;
    for _ in 0..100 {
        f.db.exec("update documents set version=version+1 where id=0")?;
    }
    let h = f
        .source
        .as_ref()
        .unwrap()
        .feed(&f.store.dir().join("sql/app.db"))?;
    assert_eq!(h.lock().unwrap().cache, 0);
    drop(h);
    f.store.close()?;
    drop(f);
    let f = load::open(dir.path(), Some(cfg))?;
    load::verify(&f)?;
    assert_eq!(
        f.db.scalar::<u32>("select sum(version) from documents")?,
        100
    );
    Ok(())
}
