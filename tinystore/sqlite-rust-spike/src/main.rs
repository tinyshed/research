use rusqlite::config::DbConfig;
use rusqlite::limits::Limit;
use rusqlite::types::Value;
use rusqlite::{Connection, ErrorCode, OpenFlags, StatementStatus, TransactionBehavior, params};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LENGTH: i32 = 1 << 20;
const STATEMENT_CACHE: usize = 32;
const GO_SQLITE_VERSION: &str = "3.53.4";
type SmokeResult<T> = Result<T, Box<dyn Error>>;

// The guard is declared before connections so their handles close first.
struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn create() -> SmokeResult<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tinystore-sqlite-rust-smoke-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("remove smoke scratch directory: {error}");
        }
    }
}

fn configure(connection: &Connection, reader: bool) -> SmokeResult<()> {
    for config in [
        DbConfig::SQLITE_DBCONFIG_DQS_DDL,
        DbConfig::SQLITE_DBCONFIG_DQS_DML,
        DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA,
    ] {
        connection.set_db_config(config, false)?;
        assert!(!connection.db_config(config)?);
    }
    connection.execute_batch(
        "PRAGMA foreign_keys=1;
         PRAGMA busy_timeout=5000;
         PRAGMA synchronous=FULL;
         PRAGMA fullfsync=1;
         PRAGMA checkpoint_fullfsync=1;
         PRAGMA cache_size=-1024;",
    )?;
    if reader {
        connection.execute_batch("PRAGMA query_only=1;")?;
    }
    connection.set_prepared_statement_cache_capacity(STATEMENT_CACHE);
    connection.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_LENGTH)?;
    assert_eq!(connection.limit(Limit::SQLITE_LIMIT_LENGTH)?, MAX_LENGTH);
    Ok(())
}

fn print_build(connection: &Connection) -> SmokeResult<()> {
    let (version, source_id): (String, String) =
        connection.query_row("SELECT sqlite_version(), sqlite_source_id()", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    println!("sqlite_version={version}");
    println!("sqlite_source_id={source_id}");
    println!("tinystore_go_sqlite_version={GO_SQLITE_VERSION}");
    if version != GO_SQLITE_VERSION {
        println!("version_match=false; this smoke does not establish version parity");
    }
    let mut statement = connection.prepare("PRAGMA compile_options")?;
    let mut options = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    options.sort();
    assert!(options.iter().any(|option| option == "THREADSAFE=1"));
    println!("compile_options:");
    for option in options {
        println!("  {option}");
    }
    Ok(())
}

fn check_pragmas(
    connection: &Connection,
    name: &str,
    reader: bool,
    page_size: i64,
) -> SmokeResult<()> {
    println!("{name} pragma readbacks:");
    for (pragma, expected) in [
        ("foreign_keys", 1),
        ("busy_timeout", 5000),
        ("synchronous", 2),
        ("fullfsync", 1),
        ("checkpoint_fullfsync", 1),
        ("cache_size", -1024),
        ("page_size", page_size),
        ("query_only", i64::from(reader)),
        ("trusted_schema", 0),
    ] {
        let value: i64 = connection.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))?;
        assert_eq!(value, expected, "{name} {pragma}");
        println!("  {pragma}={value}");
    }
    let journal: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    assert_eq!(journal, "wal");
    println!("  journal_mode={journal}");
    println!("  statement_cache_capacity={STATEMENT_CACHE} (configured)");
    println!(
        "  sqlite3_limit_length={}",
        connection.limit(Limit::SQLITE_LIMIT_LENGTH)?
    );
    for (config, name) in [
        (DbConfig::SQLITE_DBCONFIG_DQS_DDL, "dqs_ddl"),
        (DbConfig::SQLITE_DBCONFIG_DQS_DML, "dqs_dml"),
    ] {
        assert!(!connection.db_config(config)?);
        println!("  {name}=false (sqlite3_db_config readback)");
    }
    assert!(
        connection
            .prepare("SELECT \"unrecognized_string_literal\"")
            .is_err()
    );
    Ok(())
}

fn check_snapshot(writer: &mut Connection, reader: &mut Connection) -> SmokeResult<()> {
    writer.execute_batch(
        "CREATE TABLE smoke(id INTEGER PRIMARY KEY, n INTEGER NOT NULL);
         INSERT INTO smoke VALUES(1,10);",
    )?;
    let snapshot = reader.transaction_with_behavior(TransactionBehavior::Deferred)?;
    let read = || {
        snapshot.query_row("SELECT n FROM smoke WHERE id=1", [], |row| {
            row.get::<_, i64>(0)
        })
    };
    assert_eq!(read()?, 10);
    let update = writer.transaction_with_behavior(TransactionBehavior::Immediate)?;
    assert!(!update.is_autocommit());
    update.execute("UPDATE smoke SET n=100 WHERE id=1", [])?;
    assert_eq!(read()?, 10);
    update.commit()?;
    assert_eq!(read()?, 10);
    snapshot.commit()?;
    let fresh: i64 = reader.query_row("SELECT n FROM smoke WHERE id=1", [], |row| row.get(0))?;
    assert_eq!(fresh, 100);
    println!(
        "PASS BEGIN IMMEDIATE writer; deferred reader sees 10 during/after WAL commit, then 100 in a new snapshot"
    );
    Ok(())
}

fn check_query_only(reader: &Connection) -> SmokeResult<()> {
    let error = reader
        .execute("UPDATE smoke SET n=999 WHERE id=1", [])
        .unwrap_err();
    assert_eq!(error.sqlite_error_code(), Some(ErrorCode::ReadOnly));
    println!("PASS OPEN_READ_WRITE reader query_only rejects writes: {error}");
    Ok(())
}

fn check_savepoints(writer: &mut Connection, reader: &Connection) -> SmokeResult<()> {
    let mut transaction = writer.transaction_with_behavior(TransactionBehavior::Immediate)?;
    {
        let kept = transaction.savepoint_with_name("kept_write")?;
        kept.execute("INSERT INTO smoke VALUES(2,20)", [])?;
        kept.commit()?;
    }
    {
        let mut failed = transaction.savepoint_with_name("reverted_write")?;
        failed.execute("UPDATE smoke SET n=999 WHERE id=1", [])?;
        failed.execute("INSERT INTO smoke VALUES(3,30)", [])?;
        failed.rollback()?;
        failed.commit()?;
    }
    transaction.commit()?;
    let mut statement = reader.prepare("SELECT id,n FROM smoke ORDER BY id")?;
    let values = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(values, [(1, 100), (2, 20)]);
    println!(
        "PASS two savepoint scopes: kept insert survives; reverted update/insert are isolated"
    );
    Ok(())
}

fn check_types(connection: &Connection) -> SmokeResult<()> {
    let integer = 9_007_199_254_740_999_i64;
    let text = "2026-10-07T00:00:00Z";
    let blob = [0_u8, 0xff, 0, 0x80];
    let mut statement = connection.prepare_cached("SELECT ?1,?2,?3,?4,?5")?;
    let values: Vec<Value> = statement.query_row(
        params![integer, 1.25_f64, text, &blob[..], Option::<i64>::None],
        |row| (0..5).map(|i| row.get(i)).collect(),
    )?;
    assert_eq!(
        values,
        [
            Value::Integer(integer),
            Value::Real(1.25),
            Value::Text(text.to_owned()),
            Value::Blob(blob.to_vec()),
            Value::Null,
        ]
    );
    let next: Vec<u8> = statement.query_row(
        params![integer, 1.25_f64, text, &[7_u8][..], Option::<i64>::None],
        |row| row.get(3),
    )?;
    assert_eq!(next, [7]);
    assert_eq!(values[3], Value::Blob(blob.to_vec()));
    println!(
        "PASS native INTEGER/FLOAT/TEXT/BLOB/NULL variants; timestamp text stays text; owned blob survives statement reuse"
    );
    Ok(())
}

fn check_limits(connection: &Connection, name: &str) -> SmokeResult<()> {
    let error = connection
        .query_row("SELECT zeroblob(?)", [MAX_LENGTH + 1], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .unwrap_err();
    assert_eq!(error.sqlite_error_code(), Some(ErrorCode::TooBig));
    println!("PASS {name} sqlite3_limit LENGTH rejects a blob over 1 MiB: {error}");
    Ok(())
}

fn check_statement_reuse(reader: &Connection) -> SmokeResult<()> {
    const QUERY: &str = "SELECT id FROM smoke ORDER BY id LIMIT CAST(? AS INTEGER)";
    {
        let mut statement = reader.prepare_cached(QUERY)?;
        for limit in [1_i64, 2, 1, 2] {
            let rows = statement
                .query_map([limit], |row| row.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert_eq!(rows.len(), limit as usize);
        }
        assert_eq!(statement.get_status(StatementStatus::Run), 4);
        assert_eq!(statement.get_status(StatementStatus::RePrepare), 0);
    }
    let statement = reader.prepare_cached(QUERY)?;
    assert_eq!(statement.get_status(StatementStatus::Run), 4);
    assert_eq!(statement.get_status(StatementStatus::RePrepare), 0);
    println!(
        "PASS LIMIT CAST(? AS INTEGER) rebound four times: run=4, reprepare=0; same program returned from connection cache"
    );
    Ok(())
}

fn main() -> SmokeResult<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let page_size = match arguments.as_slice() {
        [] => 4096,
        [flag, value] if flag == "--page-size" && value == "1024" => 1024,
        [flag, value] if flag == "--page-size" && value == "4096" => 4096,
        _ => return Err("usage: tinystore-sqlite-rust-smoke [--page-size 1024|4096]".into()),
    };
    let scratch = ScratchDirectory::create()?;
    let database = scratch.0.join("smoke.db");
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut writer = Connection::open_with_flags(&database, flags | OpenFlags::SQLITE_OPEN_CREATE)?;
    configure(&writer, false)?;
    // WAL is a write, so page size must precede it on this newly created file.
    writer.execute_batch(&format!("PRAGMA page_size={page_size};"))?;
    let journal: String = writer.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    assert_eq!(journal, "wal");
    let mut reader = Connection::open_with_flags(&database, flags)?;
    configure(&reader, true)?;

    print_build(&writer)?;
    check_pragmas(&writer, "writer", false, page_size)?;
    check_pragmas(&reader, "reader", true, page_size)?;
    check_snapshot(&mut writer, &mut reader)?;
    check_query_only(&reader)?;
    check_savepoints(&mut writer, &reader)?;
    check_types(&writer)?;
    check_limits(&writer, "writer")?;
    check_limits(&reader, "reader")?;
    check_statement_reuse(&reader)?;
    reader.close().map_err(|(_, error)| error)?;
    writer.close().map_err(|(_, error)| error)?;
    println!(
        "PASS SQLite configuration and linking smoke; no engine migration or performance claim"
    );
    Ok(())
}
