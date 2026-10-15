use std::path::Path;

use tinystore::{Options, Store};

use super::node::{Limits, Node};
use super::packet::{MAX_PACKET, Packet};
use super::pages::{self, Image};
use super::*;

#[test]
fn outbox_survives_restart_and_lost_answers_without_duplicate_effects() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source = primary(&dir.path().join("source"))?;
    let path = dir.path().join("outbox.db");
    let queue = outbox::Outbox::open(&path)?;
    queue.enqueue("client/request-1", "insert into items values(2, 'queued')")?;
    assert!(
        queue
            .enqueue("client/request-1", "delete from items")
            .is_err()
    );
    assert!(
        queue
            .drain(|_, _| Err(std::io::Error::other("offline").into()))
            .is_err()
    );
    drop(queue);
    let queue = outbox::Outbox::open(&path)?;
    assert_eq!(queue.pending()?, 1);
    assert!(
        queue
            .drain(|id, sql| {
                outbox::Outbox::apply(&source.db, id, sql)?;
                Err(std::io::Error::other("lost response").into())
            })
            .is_err()
    );
    assert_eq!(queue.pending()?, 1);
    queue.drain(|id, sql| outbox::Outbox::apply(&source.db, id, sql))?;
    assert_eq!(queue.pending()?, 0);
    assert_eq!(source.db.scalar::<u32>("select count(*) from items")?, 2);
    Ok(())
}

struct Primary {
    _store: Store,
    db: tinystore::sql::Database,
    node: Node,
}

fn primary(root: &Path) -> Result<Primary> {
    let store = Store::open(
        root,
        Options {
            background: false,
            ..Options::default()
        },
    )?;
    let db = store.database("app").open()?;
    db.exec("create table items(id integer primary key, value text)")?;
    db.exec("insert into items values(1, 'first')")?;
    let mut node = Node::open(root)?;
    node.publish("app", "sql/app.db")?;
    node.capture("app")?;
    Ok(Primary {
        _store: store,
        db,
        node,
    })
}

fn pull(parent: &Node, child: &mut Node) -> Result<usize> {
    let after = child
        .position("app")?
        .map_or(0, |position| position.sequence);
    let packets = parent.changes("app", after, Limits::default())?;
    for bytes in &packets {
        child.receive(bytes)?;
    }
    Ok(packets.len())
}

fn count(node: &Node) -> Result<u32> {
    node.read("app", |db| {
        Ok(db.query_row("select count(*) from items", [], |row| row.get(0))?)
    })
}

#[test]
fn snapshot_and_incremental_apply_match_except_sqlite_change_counters() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    pull(&source.node, &mut copy)?;
    source.db.exec("insert into items values(2, 'second')")?;
    source.node.capture("app")?;
    pull(&source.node, &mut copy)?;
    let original = Image::read(&mut pages::reader(&dir.path().join("source/sql/app.db"))?)?;
    let copied = copy.read("app", |db| {
        Ok(db
            .prepare("select pgno, data from sqlite_dbpage order by pgno")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<pages::Pages>>()?)
    })?;
    for (number, page) in &original.pages {
        for (offset, (a, b)) in page.iter().zip(&copied[number]).enumerate() {
            if *number != 1 || (!(24..28).contains(&offset) && !(92..96).contains(&offset)) {
                assert_eq!(a, b, "page {number}, offset {offset}");
            }
        }
    }
    assert_eq!(count(&copy)?, 2);
    assert_eq!(source.node.position("app")?, copy.position("app")?);
    Ok(())
}

#[test]
fn relay_serves_inherited_and_owned_databases() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source = primary(&dir.path().join("source"))?;
    let mut relay = primary(&dir.path().join("relay"))?;
    // Distinct stream identities allow identical local database names.
    let mut packet = Packet::decode(&source.node.changes("app", 0, Limits::default())?[0])?;
    packet.stream = "upstream".to_owned();
    relay.node.receive(&packet.encode()?)?;
    let mut leaf = Node::open(&dir.path().join("leaf"))?;
    for stream in ["app", "upstream"] {
        for bytes in relay.node.changes(stream, 0, Limits::default())? {
            leaf.receive(&bytes)?;
        }
        assert_eq!(leaf.position(stream)?, relay.node.position(stream)?);
    }
    assert_eq!(count(&leaf)?, 1);
    assert!(relay.node.write("upstream", "delete from items").is_err());
    relay
        .node
        .write("app", "insert into items values(2, 'relay')")?;
    pull(&relay.node, &mut leaf)?;
    assert_eq!(count(&leaf)?, 2);
    Ok(())
}

#[test]
fn duplicate_delivery_does_not_repeat_a_write() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    let first = source.node.changes("app", 0, Limits::default())?.remove(0);
    copy.receive(&first)?;
    copy.receive(&first)?;
    source.db.exec("insert into items values(2, 'next')")?;
    source.node.capture("app")?;
    pull(&source.node, &mut copy)?;
    copy.receive(&first)?;
    assert_eq!(count(&copy)?, 2);
    assert_eq!(pull(&source.node, &mut copy)?, 0);
    Ok(())
}

#[test]
fn gap_and_divergent_predecessor_are_refused() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    pull(&source.node, &mut copy)?;
    for index in 2..=3 {
        source
            .db
            .exec(format!("insert into items values({index}, 'next')").as_str())?;
        source.node.capture("app")?;
    }
    let packets = source.node.changes("app", 1, Limits::default())?;
    assert!(copy.receive(&packets[1]).is_err());
    let mut divergent = Packet::decode(&packets[0])?;
    divergent.previous[0] ^= 1;
    assert!(copy.receive(&divergent.encode()?).is_err());
    assert_eq!(count(&copy)?, 1);
    pull(&source.node, &mut copy)?;
    assert_eq!(count(&copy)?, 3);
    Ok(())
}

#[test]
fn corruption_is_refused_before_changing_the_database() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source = primary(&dir.path().join("source"))?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    let mut packet = Packet::decode(&source.node.changes("app", 0, Limits::default())?[0])?;
    packet.pages.get_mut(&2).unwrap()[4000] ^= 1;
    assert!(copy.receive(&packet.encode()?).is_err());
    assert_eq!(copy.position("app")?, None);
    pull(&source.node, &mut copy)?;
    assert_eq!(count(&copy)?, 1);
    Ok(())
}

#[test]
fn restart_recovers_both_apply_crash_windows() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    for after_pages in [false, true] {
        let path = dir.path().join(format!("copy-{after_pages}"));
        let mut copy = Node::open(&path)?;
        pull(&source.node, &mut copy)?;
        source
            .db
            .exec("insert into items(value) values('survives')")?;
        source.node.capture("app")?;
        let after = copy.position("app")?.unwrap().sequence;
        let packet = source
            .node
            .changes("app", after, Limits::default())?
            .remove(0);
        copy.interrupt_apply(&packet, after_pages)?;
        assert!(count(&copy).is_err(), "pending apply must block reads");
        drop(copy);
        let mut reopened = Node::open(&path)?;
        assert_eq!(reopened.position("app")?, source.node.position("app")?);
        assert_eq!(pull(&source.node, &mut reopened)?, 0);
        assert_eq!(
            count(&reopened)?,
            source.db.scalar::<u32>("select count(*) from items")?
        );
    }
    Ok(())
}

#[test]
fn restart_keeps_the_feed_and_consumers_keep_their_positions() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source_path = dir.path().join("source");
    let mut source = primary(&source_path)?;
    let copy_path = dir.path().join("copy");
    let mut copy = Node::open(&copy_path)?;
    pull(&source.node, &mut copy)?;
    source.db.exec("insert into items values(2, 'queued')")?;
    source.node.capture("app")?;
    drop(copy);
    drop(source.node);
    let parent = Node::open(&source_path)?;
    let mut copy = Node::open(&copy_path)?;
    assert_eq!(count(&copy)?, 1);
    assert_eq!(pull(&parent, &mut copy)?, 1);
    assert_eq!(count(&copy)?, 2);
    Ok(())
}

#[test]
fn pruned_feed_recovers_with_a_current_snapshot() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    pull(&source.node, &mut copy)?;
    for _ in 0..3 {
        source.db.exec("insert into items(value) values('next')")?;
        source.node.capture("app")?;
    }
    source.node.prune("app", 3)?;
    let packets = source.node.changes("app", 1, Limits::default())?;
    assert_eq!(packets.len(), 1);
    assert!(Packet::decode(&packets[0])?.snapshot);
    copy.receive(&packets[0])?;
    assert_eq!(count(&copy)?, 4);
    let mut leaf = Node::open(&dir.path().join("leaf"))?;
    pull(&copy, &mut leaf)?;
    assert_eq!(count(&leaf)?, 4);
    Ok(())
}

#[test]
fn batches_obey_entry_and_byte_limits() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    for _ in 0..4 {
        source.db.exec("insert into items(value) values('next')")?;
        source.node.capture("app")?;
    }
    let first = source.node.changes(
        "app",
        0,
        Limits {
            entries: 2,
            bytes: 256 * 1024,
        },
    )?;
    assert_eq!(first.len(), 2);
    let size = first[0].len();
    let limited = source.node.changes(
        "app",
        0,
        Limits {
            entries: 32,
            bytes: size,
        },
    )?;
    assert_eq!(limited.len(), 1);
    assert!(
        source
            .node
            .changes(
                "app",
                0,
                Limits {
                    entries: 32,
                    bytes: size - 1
                }
            )
            .is_err()
    );
    assert!(
        source
            .node
            .changes(
                "app",
                0,
                Limits {
                    entries: 0,
                    bytes: size
                }
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn promotion_fences_the_old_epoch_for_descendants() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let mut standby = Node::open(&dir.path().join("standby"))?;
    let mut leaf = Node::open(&dir.path().join("leaf"))?;
    pull(&source.node, &mut standby)?;
    pull(&standby, &mut leaf)?;
    assert_eq!(standby.promote("app")?.epoch, 2);
    standby.write("app", "insert into items values(2, 'promoted')")?;
    pull(&standby, &mut leaf)?;
    assert_eq!(count(&leaf)?, 2);
    source.db.exec("insert into items values(2, 'stale')")?;
    source.node.capture("app")?;
    let stale = source.node.changes("app", 1, Limits::default())?.remove(0);
    assert!(standby.receive(&stale).is_err());
    assert!(leaf.receive(&stale).is_err());
    assert_eq!(count(&leaf)?, 2);
    Ok(())
}

#[test]
fn asynchronous_promotion_loses_the_two_unshipped_checkpoints() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let mut standby = Node::open(&dir.path().join("standby"))?;
    pull(&source.node, &mut standby)?;
    for _ in 0..2 {
        source
            .db
            .exec("insert into items(value) values('unshipped')")?;
        source.node.capture("app")?;
    }
    assert_eq!(
        source.node.position("app")?.unwrap().sequence - standby.position("app")?.unwrap().sequence,
        2
    );
    source._store.close()?;
    standby.promote("app")?;
    assert_eq!(count(&standby)?, 1);
    Ok(())
}

#[test]
fn following_a_catalog_discovers_the_relays_new_database() -> Result<()> {
    use std::sync::{Arc, Mutex};
    let dir = tempfile::tempdir()?;
    let source = primary(&dir.path().join("source"))?;
    let node = Arc::new(Mutex::new(source.node));
    let server = transport::Server::start(Arc::clone(&node))?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    transport::follow(&mut copy, server.address, Limits::default())?;
    assert_eq!(copy.streams()?, vec!["app"]);
    let extra = source._store.database("regional").open()?;
    extra.exec("create table regional(value text)")?;
    extra.exec("insert into regional values('local')")?;
    node.lock()
        .unwrap()
        .publish("regional", "sql/regional.db")?;
    node.lock().unwrap().capture("regional")?;
    transport::follow(&mut copy, server.address, Limits::default())?;
    assert_eq!(copy.streams()?, vec!["app", "regional"]);
    copy.read("regional", |db| {
        assert_eq!(
            db.query_row("select value from regional", [], |row| row
                .get::<_, String>(0))?,
            "local"
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn replicas_and_read_connections_refuse_writes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source = primary(&dir.path().join("source"))?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    pull(&source.node, &mut copy)?;
    assert!(copy.write("app", "delete from items").is_err());
    assert!(
        copy.read("app", |db| Ok(db.execute("delete from items", [])?))
            .is_err()
    );
    assert_eq!(count(&copy)?, 1);
    Ok(())
}

#[test]
fn readers_see_whole_snapshots_while_pages_are_applied() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let copy_path = dir.path().join("copy");
    let mut copy = Node::open(&copy_path)?;
    pull(&source.node, &mut copy)?;
    let mut reader = pages::reader(&copy_path.join("replicas/app.db"))?;
    let tx = reader.transaction()?;
    assert_eq!(
        tx.query_row("select count(*) from items", [], |row| row.get::<_, u32>(0))?,
        1
    );
    source
        .db
        .exec("insert into items select null, 'many' from json_each('[1,2,3,4,5]')")?;
    source.node.capture("app")?;
    pull(&source.node, &mut copy)?;
    assert_eq!(
        tx.query_row("select count(*) from items", [], |row| row.get::<_, u32>(0))?,
        1
    );
    tx.commit()?;
    assert_eq!(count(&copy)?, 6);
    Ok(())
}

#[test]
fn schema_changes_and_truncation_replicate() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("source");
    std::fs::create_dir_all(path.join("sql"))?;
    let mut db = pages::writer(&path.join("sql/app.db"))?;
    db.execute_batch(
        "create table items(id integer primary key, value blob);
        insert into items select value, zeroblob(8192) from json_each('[1,2,3,4,5,6,7,8]')",
    )?;
    let mut parent = Node::open(&path)?;
    parent.publish("app", "sql/app.db")?;
    parent.capture("app")?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    pull(&parent, &mut copy)?;
    let before = Image::read(&mut db)?.pages.len();
    db.execute_batch(
        "delete from items where id>1; vacuum; create table extra(id integer primary key)",
    )?;
    parent.capture("app")?;
    pull(&parent, &mut copy)?;
    let image = Image::read(&mut db)?;
    assert!(image.pages.len() < before);
    assert_eq!(copy.position("app")?.unwrap().checksum, image.checksum());
    copy.read("app", |db| {
        db.query_row("select count(*) from extra", [], |row| row.get::<_, u32>(0))?;
        Ok(())
    })?;
    Ok(())
}

#[test]
fn sql_keys_and_jobs_share_one_replicated_transaction() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut source = primary(&dir.path().join("source"))?;
    let bucket = source.db.bucket::<String>("keys").open()?;
    let queue = source.db.queue::<String>("jobs").open()?;
    source.db.tx::<_, tinystore::Error>(|tx| {
        tx.exec("insert into items values(2, 'atomic')")?;
        tx.with(&bucket).set("key", &"value".to_owned())?;
        tx.with(&queue).add(&"job".to_owned())?;
        Ok(())
    })?;
    source.node.capture("app")?;
    let mut copy = Node::open(&dir.path().join("copy"))?;
    pull(&source.node, &mut copy)?;
    assert_eq!(count(&copy)?, 2);
    for table in ["_tinystore_kv_cells", "_tinystore_jobs"] {
        let rows = copy.read("app", |db| {
            Ok(
                db.query_row(&format!("select count(*) from {table}"), [], |row| {
                    row.get::<_, u32>(0)
                })?,
            )
        })?;
        assert_eq!(rows, 1);
    }
    Ok(())
}

#[test]
fn parser_rejects_truncation_bad_names_and_decompression_bombs() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source = primary(&dir.path().join("source"))?;
    let bytes = source.node.changes("app", 0, Limits::default())?.remove(0);
    for end in 0..bytes.len() {
        assert!(Packet::decode(&bytes[..end]).is_err());
    }
    let mut packet = Packet::decode(&bytes)?;
    for stream in ["../app", "", "a/b", "/tmp/app"] {
        packet.stream = stream.to_owned();
        assert!(packet.encode().is_err());
    }
    let bomb = zstd::stream::encode_all(vec![0; MAX_PACKET + 1].as_slice(), 1)?;
    assert!(Packet::decode(&bomb).is_err());
    Ok(())
}

#[test]
fn a_second_process_cannot_open_the_same_replication_directory() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let node = Node::open(dir.path())?;
    assert!(Node::open(dir.path()).is_err());
    drop(node);
    Node::open(dir.path())?;
    Ok(())
}

#[test]
fn process_death_recovers_a_durable_packet_before_and_after_page_commit() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let source = primary(&dir.path().join("source"))?;
    let bytes = source.node.changes("app", 0, Limits::default())?.remove(0);
    let packet_path = dir.path().join("packet.zst");
    std::fs::write(&packet_path, bytes)?;
    for phase in ["before-pages", "after-pages"] {
        let path = dir.path().join(phase);
        let status = std::process::Command::new(std::env::current_exe()?)
            .args(["--ignored", "--exact", "tests::apply_crash_worker"])
            .env("REPLICATION_CRASH_ROOT", &path)
            .env("REPLICATION_CRASH_PACKET", &packet_path)
            .env("REPLICATION_CRASH_PHASE", phase)
            .stdout(std::process::Stdio::null())
            .status()?;
        assert_eq!(status.code(), Some(86));
        let copy = Node::open(&path)?;
        assert_eq!(copy.position("app")?, source.node.position("app")?);
        assert_eq!(count(&copy)?, 1);
    }
    Ok(())
}

#[test]
#[ignore = "subprocess worker launched by process_death_recovers_a_durable_packet_before_and_after_page_commit"]
fn apply_crash_worker() -> Result<()> {
    let root = std::env::var_os("REPLICATION_CRASH_ROOT").ok_or("missing crash root")?;
    let packet = std::env::var_os("REPLICATION_CRASH_PACKET").ok_or("missing crash packet")?;
    let mut node = Node::open(Path::new(&root))?;
    node.interrupt_apply(
        &std::fs::read(packet)?,
        std::env::var("REPLICATION_CRASH_PHASE")? == "after-pages",
    )?;
    // exit deliberately skips Rust destructors and SQLite's connection close.
    std::process::exit(86);
}

#[test]
fn nondefault_page_sizes_replicate_and_survive_growth() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("source");
    std::fs::create_dir_all(path.join("sql"))?;
    let _startup = Node::open(&dir.path().join("startup"))?;
    let db = rusqlite::Connection::open(path.join("sql/app.db"))?;
    db.pragma_update(None, "page_size", 8192)?;
    db.execute_batch("create table items(id integer primary key, value text); insert into items values(1, 'first')")?;
    let mut parent = Node::open(&path)?;
    parent.publish("app", "sql/app.db")?;
    parent.capture("app")?;
    let mut child = Node::open(&dir.path().join("child"))?;
    pull(&parent, &mut child)?;
    db.execute_batch("create table extra(value blob); insert into extra values(zeroblob(65536))")?;
    parent.capture("app")?;
    pull(&parent, &mut child)?;
    assert_eq!(parent.position("app")?, child.position("app")?);
    child.read("app", |db| {
        assert_eq!(
            db.pragma_query_value(None, "page_size", |row| row.get::<_, u32>(0))?,
            8192
        );
        assert_eq!(
            db.query_row("select length(value) from extra", [], |row| row
                .get::<_, u32>(0))?,
            65536
        );
        Ok(())
    })?;
    Ok(())
}
