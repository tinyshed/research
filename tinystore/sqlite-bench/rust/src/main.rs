use rusqlite::config::DbConfig;
use rusqlite::limits::Limit;
use rusqlite::{Connection, OpenFlags, params};
use std::error::Error;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

type BenchResult<T> = Result<T, Box<dyn Error>>;
const FNV_OFFSET: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;
const POINT: &str = "SELECT body FROM blocks WHERE series=?1 AND at=?2";
const RANGE: &str = "SELECT at,body FROM blocks WHERE series=?1 AND at>=?2 AND at<?3 ORDER BY at LIMIT CAST(?4 AS INTEGER)";
const AGGREGATE: &str = "SELECT count(*),sum(at) FROM blocks WHERE series=?1 AND at>=?2 AND at<?3";
const UPDATE: &str = "UPDATE blocks SET body=?1 WHERE series=?2 AND at=?3";
const SAVEPOINT: &str = "SAVEPOINT grouped";
const RELEASE: &str = "RELEASE grouped";

struct Arguments {
    database: PathBuf,
    case: String,
    mode: String,
    seconds: f64,
    iterations: u64,
}

impl Arguments {
    fn parse() -> BenchResult<Self> {
        let mut args = std::env::args().skip(1);
        let mut database = None;
        let (mut case, mut mode) = ("point".to_owned(), "bench".to_owned());
        let (mut seconds, mut iterations) = (0.15, 16);
        while let Some(argument) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--db" => database = Some(PathBuf::from(value)),
                "--case" => case = value,
                "--mode" => mode = value,
                "--seconds" => seconds = value.parse::<f64>()?,
                "--iterations" => iterations = value.parse::<u64>()?,
                _ => return Err(format!("unknown argument {argument}").into()),
            }
        }
        if ![
            "point",
            "point_txn",
            "range240",
            "aggregate240",
            "update1",
            "update64",
        ]
        .contains(&case.as_str())
        {
            return Err(format!("unknown case {case}").into());
        }
        if !["bench", "verify", "inspect"].contains(&mode.as_str())
            || !seconds.is_finite()
            || seconds <= 0.0
            || iterations == 0
        {
            return Err("mode, seconds or iterations is invalid".into());
        }
        Ok(Self {
            database: database.ok_or("--db is required")?,
            case,
            mode,
            seconds,
            iterations,
        })
    }
}

struct Database {
    writer: Connection,
    reader: Connection,
}

fn configure(connection: &Connection, reader: bool) -> BenchResult<()> {
    connection.execute_batch(
        "PRAGMA foreign_keys=1; PRAGMA busy_timeout=5000;
         PRAGMA synchronous=FULL; PRAGMA fullfsync=1;
         PRAGMA checkpoint_fullfsync=1; PRAGMA cache_size=-1024;",
    )?;
    if reader {
        connection.execute_batch("PRAGMA query_only=1")?;
    }
    for config in [
        DbConfig::SQLITE_DBCONFIG_DQS_DDL,
        DbConfig::SQLITE_DBCONFIG_DQS_DML,
        DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA,
    ] {
        connection.set_db_config(config, false)?;
    }
    connection.set_prepared_statement_cache_capacity(32);
    connection.set_limit(Limit::SQLITE_LIMIT_LENGTH, 1 << 20)?;
    Ok(())
}

impl Database {
    fn open(path: &PathBuf) -> BenchResult<Self> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let writer = Connection::open_with_flags(path, flags)?;
        configure(&writer, false)?;
        let journal: String = writer.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        if journal != "wal" {
            return Err("database did not enter WAL".into());
        }
        let reader = Connection::open_with_flags(path, flags)?;
        configure(&reader, true)?;
        let version: String = writer.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
        if version != "3.53.4" {
            return Err(format!("expected SQLite 3.53.4, linked {version}").into());
        }
        // Preparing programs is setup; verification performs no mutating warmup.
        drop(reader.prepare_cached(POINT)?);
        drop(reader.prepare_cached(RANGE)?);
        drop(reader.prepare_cached(AGGREGATE)?);
        drop(writer.prepare_cached(UPDATE)?);
        drop(writer.prepare_cached(SAVEPOINT)?);
        drop(writer.prepare_cached(RELEASE)?);
        Ok(Self { writer, reader })
    }

    fn operation(&self, case: &str, index: u64) -> BenchResult<Output> {
        let (series, at) = key(index);
        match case {
            "point" => self.point(series, at),
            "point_txn" => transaction(&self.reader, "BEGIN DEFERRED", || self.point(series, at)),
            "range240" => {
                let start = at % 273;
                let mut statement = self.reader.prepare_cached(RANGE)?;
                let mut rows = statement.query(params![series, start, start + 240, 240_i64])?;
                let mut output = Vec::new();
                while let Some(row) = rows.next()? {
                    let at: i64 = row.get(0)?;
                    let body: Vec<u8> = row.get(1)?;
                    output.push(Row { at, body });
                }
                Ok(Output {
                    count: output.len() as i64,
                    rows: output,
                    ..Output::default()
                })
            }
            "aggregate240" => {
                let start = at % 273;
                let mut statement = self.reader.prepare_cached(AGGREGATE)?;
                let (count, sum): (i64, i64) = statement
                    .query_row(params![series, start, start + 240], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })?;
                Ok(Output {
                    count,
                    sum,
                    ..Output::default()
                })
            }
            "update1" => transaction(&self.writer, "BEGIN IMMEDIATE", || {
                let body = update_body(index);
                let changed =
                    self.writer
                        .prepare_cached(UPDATE)?
                        .execute(params![&body[..], series, at])? as i64;
                Ok(Output {
                    affected: changed,
                    ..Output::default()
                })
            }),
            "update64" => transaction(&self.writer, "BEGIN IMMEDIATE", || {
                let mut changed = 0;
                for j in 0..64 {
                    let q = index * 64 + j;
                    let (series, at) = key(q);
                    let body = update_body(q);
                    self.writer.prepare_cached(SAVEPOINT)?.execute([])?;
                    changed += self.writer.prepare_cached(UPDATE)?.execute(params![
                        &body[..],
                        series,
                        at
                    ])? as i64;
                    self.writer.prepare_cached(RELEASE)?.execute([])?;
                }
                Ok(Output {
                    affected: changed,
                    ..Output::default()
                })
            }),
            _ => unreachable!(),
        }
    }

    fn point(&self, series: i64, at: i64) -> BenchResult<Output> {
        let blob = self
            .reader
            .prepare_cached(POINT)?
            .query_row(params![series, at], |row| row.get(0))?;
        Ok(Output {
            blob,
            count: 1,
            ..Output::default()
        })
    }

    fn digest(&self) -> BenchResult<u64> {
        let mut statement = self
            .reader
            .prepare("SELECT series,at,body FROM blocks ORDER BY series,at")?;
        let mut rows = statement.query([])?;
        let mut hash = FNV_OFFSET;
        while let Some(row) = rows.next()? {
            let series: i64 = row.get(0)?;
            let at: i64 = row.get(1)?;
            let body: Vec<u8> = row.get(2)?;
            hash_bytes(&mut hash, &series.to_le_bytes());
            hash_bytes(&mut hash, &at.to_le_bytes());
            hash_bytes(&mut hash, &body);
        }
        Ok(hash)
    }
}

struct Row {
    at: i64,
    body: Vec<u8>,
}

#[derive(Default)]
struct Output {
    blob: Vec<u8>,
    rows: Vec<Row>,
    count: i64,
    sum: i64,
    affected: i64,
}

fn transaction<F>(connection: &Connection, begin: &str, work: F) -> BenchResult<Output>
where
    F: FnOnce() -> BenchResult<Output>,
{
    connection.execute_batch(begin)?;
    match work() {
        Ok(output) => {
            connection.execute_batch("COMMIT")?;
            Ok(output)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn key(index: u64) -> (i64, i64) {
    let row = (index % 32768) * 7919 % 32768;
    ((row / 512) as i64, (row % 512) as i64)
}

fn update_body(index: u64) -> Vec<u8> {
    let mut body = vec![0; 128];
    for (k, byte) in body.iter_mut().enumerate() {
        *byte = ((index % 251 + k as u64) % 251) as u8;
    }
    body
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash = (*hash ^ byte as u64).wrapping_mul(FNV_PRIME);
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn verify(database: &Database, arguments: &Arguments) -> BenchResult<()> {
    let (mut hash, mut rows, mut affected) = (FNV_OFFSET, 0, 0);
    for i in 0..arguments.iterations {
        let output = database.operation(&arguments.case, i)?;
        match arguments.case.as_str() {
            "point" | "point_txn" => hash_bytes(&mut hash, &output.blob),
            "range240" => {
                for row in &output.rows {
                    hash_bytes(&mut hash, &row.at.to_le_bytes());
                    hash_bytes(&mut hash, &row.body);
                }
            }
            "aggregate240" => {
                hash_bytes(&mut hash, &output.count.to_le_bytes());
                hash_bytes(&mut hash, &output.sum.to_le_bytes());
            }
            "update1" | "update64" => hash_bytes(&mut hash, &output.affected.to_le_bytes()),
            _ => unreachable!(),
        }
        rows += output.count;
        affected += output.affected;
    }
    println!(
        "{{\"case\":{},\"implementation\":\"rust_native\",\"iterations\":{},\"checksum\":\"{hash:016x}\",\"rows\":{rows},\"affected\":{affected},\"final_digest\":\"{:016x}\"}}",
        json_string(&arguments.case),
        arguments.iterations,
        database.digest()?
    );
    Ok(())
}

fn benchmark(database: &Database, arguments: &Arguments) -> BenchResult<()> {
    for i in 0..32 {
        black_box(database.operation(black_box(&arguments.case), black_box(i))?);
    }
    let (mut sequence, mut iterations) = (32, 1_u64);
    let minimum = Duration::from_secs_f64(arguments.seconds);
    loop {
        let mut sink = 0_u64;
        let start = Instant::now();
        for _ in 0..iterations {
            let output = database.operation(black_box(&arguments.case), black_box(sequence))?;
            sink = sink.wrapping_add(
                output.blob.len() as u64
                    ^ output.rows.len() as u64
                    ^ output.count as u64
                    ^ output.affected as u64,
            );
            black_box(output);
            sequence += 1;
        }
        let elapsed = start.elapsed();
        if elapsed >= minimum {
            println!(
                "{{\"case\":{},\"implementation\":\"rust_native\",\"iterations\":{iterations},\"ns_per_op\":{},\"checksum_sink\":{sink}}}",
                json_string(&arguments.case),
                elapsed.as_secs_f64() * 1e9 / iterations as f64
            );
            return Ok(());
        }
        iterations = iterations
            .checked_mul(2)
            .ok_or("calibration iterations overflow")?;
    }
}

fn pragma_json(connection: &Connection) -> BenchResult<String> {
    let mut fields = Vec::new();
    for pragma in [
        "foreign_keys",
        "busy_timeout",
        "synchronous",
        "fullfsync",
        "checkpoint_fullfsync",
        "cache_size",
        "page_size",
        "query_only",
        "trusted_schema",
        "wal_autocheckpoint",
        "mmap_size",
    ] {
        let value: i64 = connection.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))?;
        fields.push(format!("{}:{value}", json_string(pragma)));
    }
    let journal: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    fields.push(format!("\"journal_mode\":{}", json_string(&journal)));
    let (version, source_id): (String, String) =
        connection.query_row("SELECT sqlite_version(),sqlite_source_id()", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    fields.push(format!("\"version\":{}", json_string(&version)));
    fields.push(format!("\"source_id\":{}", json_string(&source_id)));
    fields.push("\"statement_cache_capacity\":32".to_owned());
    fields.push(format!(
        "\"limit_length\":{}",
        connection.limit(Limit::SQLITE_LIMIT_LENGTH)?
    ));
    fields.push(format!(
        "\"dqs_ddl\":{}",
        connection.db_config(DbConfig::SQLITE_DBCONFIG_DQS_DDL)?
    ));
    fields.push(format!(
        "\"dqs_dml\":{}",
        connection.db_config(DbConfig::SQLITE_DBCONFIG_DQS_DML)?
    ));
    Ok(format!("{{{}}}", fields.join(",")))
}

fn cache_used(connection: &Connection) -> BenchResult<i32> {
    let (mut current, mut high) = (0, 0);
    // The connection owns the live handle throughout this synchronous call.
    let result = unsafe {
        rusqlite::ffi::sqlite3_db_status(
            connection.handle(),
            rusqlite::ffi::SQLITE_DBSTATUS_CACHE_USED,
            &mut current,
            &mut high,
            0,
        )
    };
    if result != rusqlite::ffi::SQLITE_OK {
        return Err(format!("sqlite3_db_status returned {result}").into());
    }
    Ok(current)
}

fn inspect(database: &Database) -> BenchResult<()> {
    for i in 0..1000 {
        black_box(database.operation("point", i)?);
    }
    let (version, source_id): (String, String) =
        database
            .writer
            .query_row("SELECT sqlite_version(),sqlite_source_id()", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
    let mut statement = database.writer.prepare("PRAGMA compile_options")?;
    let mut options = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    options.sort();
    let options = options
        .iter()
        .map(|option| json_string(option))
        .collect::<Vec<_>>()
        .join(",");
    let status = std::fs::read_to_string("/proc/self/status")?;
    let rss = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|line| line.split_whitespace().next())
        .ok_or("VmRSS absent")?
        .parse::<u64>()?;
    let sqlite_memory = unsafe { rusqlite::ffi::sqlite3_memory_used() };
    println!(
        "{{\"implementation\":\"rust_native\",\"sqlite_version\":{},\"sqlite_source_id\":{},\"compile_options\":[{options}],\"writer\":{},\"reader\":{},\"rss_kib\":{rss},\"sqlite_memory_used\":{sqlite_memory},\"writer_cache_used\":{},\"reader_cache_used\":{}}}",
        json_string(&version),
        json_string(&source_id),
        pragma_json(&database.writer)?,
        pragma_json(&database.reader)?,
        cache_used(&database.writer)?,
        cache_used(&database.reader)?
    );
    Ok(())
}

fn main() -> BenchResult<()> {
    let arguments = Arguments::parse()?;
    let database = Database::open(&arguments.database)?;
    match arguments.mode.as_str() {
        "verify" => verify(&database, &arguments),
        "bench" => benchmark(&database, &arguments),
        "inspect" => inspect(&database),
        _ => unreachable!(),
    }
}
