//! The MySQL probe: what a read-only read of MySQL 8.4 and MariaDB 11 can promise, through mysql_async.
//!
//! `probe <part> <mysql|mariadb>`; the parts are `attacks`, `types`, `params`, `cap`, `cancel`, `zones`,
//! `latency`, `trips`, and `hold`, which a test of a lost client runs as its own process.

use std::time::{Duration, Instant};

use mysql_async::consts::{ColumnFlags, ColumnType};
use mysql_async::prelude::*;
use mysql_async::{BinaryProtocol, Column, Conn, Opts, OptsBuilder, QueryResult, Row, TextProtocol, Value};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Server {
    MySql,
    MariaDb,
}

impl Server {
    fn port(self) -> u16 {
        match self {
            Server::MySql => 13306,
            Server::MariaDb => 13307,
        }
    }

    /// The server's own bound on a statement's time, which each spells its own way.
    fn timeout(self, ms: u64) -> String {
        match self {
            Server::MySql => format!("max_execution_time = {ms}"),
            Server::MariaDb => format!("max_statement_time = {}", ms as f64 / 1000.0),
        }
    }
}

const READER: (&str, &str) = ("reader", "probe-reader");
const WRITER: (&str, &str) = ("writer", "probe-writer");
const ROOT: (&str, &str) = ("root", "probe-root");

fn host() -> String {
    std::env::var("PROBE_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned())
}

fn port(server: Server) -> u16 {
    match (server, std::env::var("PROBE_HOST")) {
        (_, Ok(_)) => 3306,
        (server, Err(_)) => server.port(),
    }
}

fn server_host(server: Server) -> String {
    match std::env::var("PROBE_HOST") {
        Ok(_) => match server {
            Server::MySql => "mysql".to_owned(),
            Server::MariaDb => "mariadb".to_owned(),
        },
        Err(_) => host(),
    }
}

fn opts(server: Server, (user, pass): (&str, &str)) -> Opts {
    OptsBuilder::default()
        .ip_or_hostname(server_host(server))
        .tcp_port(port(server))
        .user(Some(user))
        .pass(Some(pass))
        .db_name(Some("probe"))
        .prefer_socket(false)
        .stmt_cache_size(0)
        .max_allowed_packet(std::env::var("PROBE_KNOWN_SETTINGS").ok().map(|_| 64 * 1024 * 1024))
        .wait_timeout(std::env::var("PROBE_KNOWN_SETTINGS").ok().map(|_| 28_800))
        .into()
}

async fn connect(server: Server, who: (&str, &str)) -> Conn {
    Conn::new(opts(server, who)).await.expect("connect")
}

/// The session a read would run in: read-only twice, the server's own timeout, UTC, and maybe a row limit.
async fn session(server: Server, who: (&str, &str), timeout_ms: u64, select_limit: Option<u64>) -> Conn {
    let mut conn = connect(server, who).await;
    let mut set = vec!["transaction_read_only = ON".to_owned(), "time_zone = '+00:00'".to_owned(), server.timeout(timeout_ms)];
    if let Some(limit) = select_limit {
        set.push(format!("sql_select_limit = {limit}"));
    }
    conn.query_drop(format!("SET SESSION {}", set.join(", "))).await.expect("session");
    conn.query_drop("START TRANSACTION READ ONLY").await.expect("read only");
    conn
}

fn said(error: &mysql_async::Error) -> String {
    match error {
        mysql_async::Error::Server(server) => format!("{} {}", server.code, server.message),
        other => other.to_string(),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server = match args.get(2).map(String::as_str) {
        Some("mariadb") => Server::MariaDb,
        _ => Server::MySql,
    };
    match args.get(1).map(String::as_str) {
        Some("attacks") => attacks(server).await,
        Some("types") => types(server).await,
        Some("params") => params(server).await,
        Some("cap") => cap(server).await,
        Some("cancel") => cancel(server).await,
        Some("zones") => zones(server).await,
        Some("latency") => latency(server).await,
        Some("trips") => trips(server).await,
        Some("hold") => hold(server).await,
        Some("memory") => memory(server).await,
        Some("followup") => followup(server).await,
        Some("columns") => {
            let mut conn = session(server, READER, 5_000, None).await;
            for sql in &args[3..] {
                match conn.prep(sql.as_str()).await {
                    Ok(statement) => println!("- `{sql}`: {} columns at prepare", statement.columns().len()),
                    Err(error) => println!("- `{sql}`: refused: {}", said(&error)),
                }
            }
        }
        Some("connect") => match Conn::new(opts(server, READER)).await {
            Ok(conn) => println!("mysql_async connected as reader, id {}", conn.id()),
            Err(error) => println!("mysql_async failed: {}", said(&error)),
        },
        _ => eprintln!("probe <attacks|types|params|cap|cancel|zones|latency|trips|memory> <mysql|mariadb>"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// attacks: what each layer refuses

const ATTACKS: &[(&str, &str)] = &[
    ("select (control)", "SELECT x FROM probe.victim"),
    ("create table", "CREATE TABLE probe.t_new (x INT)"),
    ("create table as select", "CREATE TABLE probe.t_new AS SELECT 1 AS x"),
    ("drop table", "DROP TABLE probe.victim"),
    ("truncate", "TRUNCATE TABLE probe.victim"),
    ("rename table", "RENAME TABLE probe.victim TO probe.victim2"),
    ("create procedure", "CREATE PROCEDURE probe.p2() SELECT 1"),
    ("insert", "INSERT INTO probe.victim VALUES (9)"),
    ("update", "UPDATE probe.victim SET x = 9"),
    ("delete returning", "DELETE FROM probe.victim RETURNING x"),
    ("insert returning", "INSERT INTO probe.victim VALUES (9) RETURNING x"),
    ("two statements", "SELECT 1; DROP TABLE probe.victim"),
    ("drop in /*! */", "SELECT 1 /*! ; DROP TABLE probe.victim */"),
    ("function that writes", "SELECT probe.writes()"),
    ("procedure: commit, read-write, insert", "CALL probe.escapes()"),
    ("session read-write", "SET SESSION transaction_read_only = OFF"),
    ("set transaction read write", "SET TRANSACTION READ WRITE"),
    ("start transaction read write", "START TRANSACTION READ WRITE"),
    ("commit", "COMMIT"),
    ("select for update", "SELECT x FROM probe.victim FOR UPDATE"),
    ("lock in share mode", "SELECT x FROM probe.victim LOCK IN SHARE MODE"),
    ("lock tables", "LOCK TABLES probe.victim WRITE"),
    ("named lock", "SELECT GET_LOCK('probe', 0)"),
    ("into outfile", "SELECT x FROM probe.victim INTO OUTFILE '/tmp/victim.txt'"),
    ("into variable", "SELECT x INTO @x FROM probe.victim"),
    ("user variable :=", "SELECT @x := 5"),
    ("temporary table", "CREATE TEMPORARY TABLE probe.scratch (x INT)"),
    ("analyze table", "ANALYZE TABLE probe.victim"),
    ("optimize table", "OPTIMIZE TABLE probe.victim"),
    ("repair table", "REPAIR TABLE probe.victim"),
    ("check table", "CHECK TABLE probe.victim"),
    ("checksum table", "CHECKSUM TABLE probe.victim"),
    ("cache index", "CACHE INDEX probe.victim IN default"),
    ("flush tables", "FLUSH TABLES"),
    ("kill", "KILL 999999"),
    ("set global", "SET GLOBAL max_connections = 99"),
    ("set password", "SET PASSWORD = 'changed-by-probe'"),
    ("alter own user", "ALTER USER CURRENT_USER() IDENTIFIED BY 'changed-by-probe'"),
    ("grant", "GRANT SELECT ON probe.* TO reader"),
    ("create user", "CREATE USER sneaky IDENTIFIED BY 'x'"),
    ("prepare inside", "PREPARE s FROM 'DROP TABLE probe.victim'"),
    ("do", "DO SLEEP(0)"),
    ("handler open", "HANDLER probe.victim OPEN"),
    ("xa start", "XA START 'probe'"),
    ("signal", "SIGNAL SQLSTATE '45000'"),
    ("load data local", "LOAD DATA LOCAL INFILE '/etc/hostname' INTO TABLE probe.victim"),
    ("load_file", "SELECT LOAD_FILE('/etc/hostname') IS NOT NULL"),
    ("sequence nextval", "SELECT NEXTVAL(probe.seq)"),
    ("set statement ... for select", "SET STATEMENT max_statement_time = 0 FOR SELECT 1"),
    (
        "set statement read-write for delete returning",
        "SET STATEMENT transaction_read_only = 0 FOR DELETE FROM probe.victim RETURNING x",
    ),
    ("hint set_var read-write", "SELECT /*+ SET_VAR(transaction_read_only = OFF) */ 1"),
    ("show", "SHOW TABLES FROM probe"),
    ("explain analyze", "EXPLAIN ANALYZE SELECT x FROM probe.victim"),
    ("analyze select", "ANALYZE SELECT x FROM probe.victim"),
    ("table", "TABLE probe.victim"),
    ("values", "VALUES ROW(1)"),
    ("with", "WITH v AS (SELECT x FROM probe.victim) SELECT x FROM v"),
    ("help", "HELP 'select'"),
    ("shutdown", "SHUTDOWN"),
];

/// What the database looks like to an attack: the victim's rows, whether a new table or user exists, when the
/// victim's statistics were last written and when its table was made.
async fn state(root: &mut Conn) -> String {
    let rows: Vec<Row> = root
        .query(
            "SELECT (SELECT GROUP_CONCAT(x ORDER BY x) FROM probe.victim), \
             (SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'probe' AND table_name IN ('t_new', 'victim2')), \
             (SELECT COUNT(*) FROM mysql.user WHERE user = 'sneaky'), \
             (SELECT last_update FROM mysql.innodb_table_stats WHERE database_name = 'probe' AND table_name = 'victim'), \
             (SELECT create_time FROM information_schema.tables WHERE table_schema = 'probe' AND table_name = 'victim'), \
             (SELECT COUNT(*) FROM information_schema.routines WHERE routine_schema = 'probe'), \
             @@global.max_connections",
        )
        .await
        .unwrap_or_default();
    rows.into_iter().next().map(|row| format!("{:?}", row.unwrap())).unwrap_or_else(|| "victim gone".to_owned())
}

async fn reset(root: &mut Conn, server: Server) {
    for sql in [
        "DROP TABLE IF EXISTS probe.t_new",
        "CREATE TABLE IF NOT EXISTS probe.victim (x INT)",
        "DELETE FROM probe.victim",
        "INSERT INTO probe.victim VALUES (1)",
        "DROP USER IF EXISTS sneaky",
        "DROP PROCEDURE IF EXISTS probe.p2",
        "ALTER USER reader IDENTIFIED BY 'probe-reader'",
        "ALTER USER writer IDENTIFIED BY 'probe-writer'",
        "SET GLOBAL max_connections = 151",
    ] {
        root.query_drop(sql).await.unwrap_or_else(|error| eprintln!("reset: {sql}: {}", said(&error)));
    }
    let _ = root.query_drop("RENAME TABLE probe.victim2 TO probe.victim").await;
    let _ = root.query_drop("DROP TABLE IF EXISTS probe.victim2").await;
    if server == Server::MariaDb {
        let _ = root.query_drop("CREATE SEQUENCE IF NOT EXISTS probe.seq").await;
    }
}

/// One statement as a read would run it: prepared in the read-only session, then executed whatever its columns.
async fn attempt(server: Server, who: (&str, &str), sql: &str) -> (String, String) {
    let mut conn = session(server, who, 5_000, None).await;
    let prepared = match conn.prep(sql).await {
        Ok(statement) => statement,
        Err(error) => return (format!("refused: {}", said(&error)), String::new()),
    };
    let columns = format!("{} columns", prepared.columns().len());
    let ran = match conn.exec_iter(&prepared, ()).await {
        Ok(mut result) => {
            let mut rows = 0;
            let mut last = None;
            loop {
                match result.next().await {
                    Ok(Some(row)) => {
                        rows += 1;
                        last = Some(row.unwrap());
                    }
                    Ok(None) => break,
                    Err(error) => {
                        last = None;
                        rows = -1;
                        let _ = format!("{}", said(&error));
                        return (columns, format!("failed while reading: {}", said(&error)));
                    }
                }
            }
            drop(result);
            format!("ran, {rows} rows{}", last.map(|row| format!(", last {row:?}")).unwrap_or_default())
        }
        Err(error) => format!("failed: {}", said(&error)),
    };
    let _ = conn.query_drop("ROLLBACK").await;
    let _ = conn.disconnect().await;
    (columns, ran)
}

/// What the text protocol does with two statements in one string, which every driver enables.
async fn two_statements_as_text(server: Server) -> String {
    let mut conn = session(server, WRITER, 5_000, None).await;
    let mut out = Vec::new();
    match conn.query_iter("SELECT 1 AS one; DROP TABLE probe.victim").await {
        Ok(mut result) => loop {
            match result.collect::<Row>().await {
                Ok(rows) => out.push(format!("set of {} rows", rows.len())),
                Err(error) => {
                    out.push(format!("failed: {}", said(&error)));
                    break;
                }
            }
            if result.is_empty() {
                break;
            }
        },
        Err(error) => out.push(format!("failed: {}", said(&error))),
    }
    let _ = conn.query_drop("ROLLBACK").await;
    out.join("; ")
}

async fn attacks(server: Server) {
    let mut root = connect(server, ROOT).await;
    reset(&mut root, server).await;
    let before = state(&mut root).await;
    println!("# attacks on {server:?}: state before {before}\n");
    println!("| attack | statement | prepared | writer: ran | changed | reader: ran |");
    println!("|---|---|---|---|---|---|");
    for (name, sql) in ATTACKS {
        let (columns, ran) = attempt(server, WRITER, sql).await;
        let after = state(&mut root).await;
        let changed = if after == before { "no".to_owned() } else { format!("**{after}**") };
        reset(&mut root, server).await;
        let (_, as_reader) = attempt(server, READER, sql).await;
        reset(&mut root, server).await;
        println!("| {name} | `{sql}` | {columns} | {ran} | {changed} | {as_reader} |");
    }
    let text = two_statements_as_text(server).await;
    let after = state(&mut root).await;
    println!("\ntext protocol, `SELECT 1 AS one; DROP TABLE probe.victim` as writer: {text}; state {after}");
    reset(&mut root, server).await;
}

// ---------------------------------------------------------------------------------------------------------------
// types: each column through the text protocol and the binary one

fn shown_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) if text.chars().all(|c| !c.is_control()) => format!("{text:?}"),
        _ => format!("0x{}", bytes.iter().map(|b| format!("{b:02X}")).collect::<String>()),
    }
}

fn shown_value(value: &Value) -> String {
    match value {
        Value::NULL => "NULL".to_owned(),
        Value::Bytes(bytes) => format!("Bytes {}", shown_bytes(bytes)),
        Value::Int(int) => format!("Int {int}"),
        Value::UInt(int) => format!("UInt {int}"),
        Value::Float(float) => format!("Float {float}"),
        Value::Double(double) => format!("Double {double}"),
        Value::Date(y, mo, d, h, mi, s, us) => format!("Date {y}-{mo}-{d} {h}:{mi}:{s}.{us}"),
        Value::Time(negative, days, h, mi, s, us) => format!("Time neg={negative} {days}d {h}:{mi}:{s}.{us}"),
    }
}

fn column_type(column: &Column) -> String {
    let unsigned = if column.flags().contains(ColumnFlags::UNSIGNED_FLAG) { " unsigned" } else { "" };
    let binary = if column.character_set() == 63 { " binary" } else { "" };
    format!("{:?}{unsigned}{binary} decimals={}", column.column_type(), column.decimals())
        .replace("MYSQL_TYPE_", "")
}

async fn types(server: Server) {
    let mut conn = session(server, READER, 5_000, None).await;
    let sql = "SELECT * FROM probe.types ORDER BY id";
    let text: Vec<Row> = conn.query(sql).await.expect("text");
    let statement = conn.prep(sql).await.expect("prep");
    let binary: Vec<Row> = conn.exec(&statement, ()).await.expect("binary");
    println!("# types on {server:?}\n");
    println!("| column | type (text) | type (binary) | row | text protocol | binary protocol |");
    println!("|---|---|---|---|---|---|");
    let text_columns = text[0].columns();
    let binary_columns = binary[0].columns();
    for (at, column) in text_columns.iter().enumerate() {
        for (row, (text_row, binary_row)) in text.iter().zip(&binary).enumerate().take(2) {
            println!(
                "| {} | {} | {} | {} | {} | {} |",
                column.name_str(),
                column_type(column),
                column_type(&binary_columns[at]),
                row + 1,
                shown_value(text_row.as_ref(at).unwrap()),
                shown_value(binary_row.as_ref(at).unwrap()),
            );
        }
    }
    let nulls = binary[2].clone().unwrap();
    println!("\nrow 3 through the binary protocol: {} values, all NULL but id: {}", nulls.len(), nulls[1..].iter().all(|v| *v == Value::NULL));
    // a computed instant and a parameter's type come back as what?
    let computed = conn
        .prep("SELECT created_at, created_at + INTERVAL 1 HOUR AS later, DATE(created_at) AS day, UNIX_TIMESTAMP(created_at) AS epoch, \
               CAST(? AS DATETIME(6)) AS cast_param, ? AS raw_param, SUM(amount) AS total, COUNT(*) AS n, AVG(amount) AS mean \
               FROM probe.big WHERE id = 1 GROUP BY created_at")
        .await
        .expect("prep computed");
    let rows: Vec<Row> = conn.exec(&computed, ("2026-11-01 00:00:00", "2026-11-01 00:00:00")).await.expect("computed");
    println!("\ncomputed columns through the binary protocol:\n");
    for (at, column) in rows[0].columns().iter().enumerate() {
        println!("- {}: {} → {}", column.name_str(), column_type(column), shown_value(rows[0].as_ref(at).unwrap()));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// params: binding (a), a user variable set apart, against (b), a placeholder in the statement

const D_EXACT: &str = "99999999999999999999999999999999999.999999999999999999999999999999";
const D_NEAR: &str = "99999999999999999999999999999999999.999999999999999999999999999998";
const U_EXACT: &str = "18446744073709551615";
const U_NEAR: &str = "18446744073709551614";

async fn first_text(conn: &mut Conn, sql: &str) -> String {
    match conn.query::<Row, _>(sql).await {
        Ok(rows) => rows.into_iter().next().map(|row| shown_value(row.as_ref(0).unwrap())).unwrap_or_default(),
        Err(error) => format!("failed: {}", said(&error)),
    }
}

async fn first_binary(conn: &mut Conn, sql: &str, params: Vec<Value>) -> String {
    let statement = match conn.prep(sql).await {
        Ok(statement) => statement,
        Err(error) => return format!("prepare failed: {}", said(&error)),
    };
    match conn.exec::<Row, _, _>(&statement, params).await {
        Ok(rows) => rows.into_iter().next().map(|row| shown_value(row.as_ref(0).unwrap())).unwrap_or_default(),
        Err(error) => format!("failed: {}", said(&error)),
    }
}

/// (a): `SET @p = <set>` prepared with the value apart, then `<query>` as text with `@p` in it.
async fn by_variable(conn: &mut Conn, set: &str, value: Value, query: &str) -> String {
    let statement = conn.prep(format!("SET @p = {set}")).await.expect("prep set");
    if let Err(error) = conn.exec_drop(&statement, vec![value]).await {
        return format!("set failed: {}", said(&error));
    }
    first_text(conn, query).await
}

async fn params(server: Server) {
    let mut conn = session(server, READER, 5_000, None).await;
    println!("# parameters on {server:?}\n");
    println!("exact must say 1, near must say 0; both 1 means the comparison went through DOUBLE.\n");
    println!("| value | binding | exact | near |");
    println!("|---|---|---|---|");
    let text = |s: &str| Value::Bytes(s.as_bytes().to_vec());
    for (what, column, exact, near, cast) in [
        ("DECIMAL(65,30)", "d65", D_EXACT, D_NEAR, "CAST(? AS DECIMAL(65,30))"),
        ("BIGINT UNSIGNED", "u64", U_EXACT, U_NEAR, "CAST(? AS UNSIGNED)"),
    ] {
        let query = format!("SELECT {column} = @p FROM probe.types WHERE id = 1");
        let a_text = (
            by_variable(&mut conn, "?", text(exact), &query).await,
            by_variable(&mut conn, "?", text(near), &query).await,
        );
        println!("| {what} | (a) `SET @p = ?`, text | {} | {} |", a_text.0, a_text.1);
        let a_cast = (
            by_variable(&mut conn, cast, text(exact), &query).await,
            by_variable(&mut conn, cast, text(near), &query).await,
        );
        println!("| {what} | (a) `SET @p = {cast}` | {} | {} |", a_cast.0, a_cast.1);
        let b = format!("SELECT {column} = ? FROM probe.types WHERE id = 1");
        let b_text = (first_binary(&mut conn, &b, vec![text(exact)]).await, first_binary(&mut conn, &b, vec![text(near)]).await);
        println!("| {what} | (b) `= ?`, text | {} | {} |", b_text.0, b_text.1);
        let b_cast = format!("SELECT {column} = {cast} FROM probe.types WHERE id = 1");
        let b_cast = (
            first_binary(&mut conn, &b_cast, vec![text(exact)]).await,
            first_binary(&mut conn, &b_cast, vec![text(near)]).await,
        );
        println!("| {what} | (b) `= {cast}`, text | {} | {} |", b_cast.0, b_cast.1);
        if column == "u64" {
            let typed = (
                first_binary(&mut conn, &b, vec![Value::UInt(u64::MAX)]).await,
                first_binary(&mut conn, &b, vec![Value::UInt(u64::MAX - 1)]).await,
            );
            println!("| {what} | (b) `= ?`, UInt | {} | {} |", typed.0, typed.1);
        }
    }
    println!("\nwhat the value itself comes back as:\n");
    println!("- (a) `SET @p = CAST(? AS DECIMAL(65,30))`, `SELECT @p`: {}", by_variable(&mut conn, "CAST(? AS DECIMAL(65,30))", text(D_EXACT), "SELECT @p").await);
    println!("- (a) `SET @p = CAST(? AS DATETIME(6))`, `SELECT @p`: {}", by_variable(&mut conn, "CAST(? AS DATETIME(6))", text("2026-10-02 00:00:00"), "SELECT @p").await);
    println!("- (a) the same, `SELECT @p + INTERVAL 1 DAY`: {}", by_variable(&mut conn, "CAST(? AS DATETIME(6))", text("2026-10-02 00:00:00"), "SELECT @p + INTERVAL 1 DAY").await);
    println!("- (b) `SELECT CAST(? AS DECIMAL(65,30))`: {}", first_binary(&mut conn, "SELECT CAST(? AS DECIMAL(65,30))", vec![text(D_EXACT)]).await);
    println!("- (b) `SELECT CAST(? AS DATETIME(6)) + INTERVAL 1 DAY`: {}", first_binary(&mut conn, "SELECT CAST(? AS DATETIME(6)) + INTERVAL 1 DAY", vec![text("2026-10-02 00:00:00")]).await);
    println!("- (b) `SELECT ?` with a string: {}", first_binary(&mut conn, "SELECT ?", vec![text("12.50")]).await);
    println!("- (b) `SELECT CAST(? AS DECIMAL(4,2))` with '12.345': {}", first_binary(&mut conn, "SELECT CAST(? AS DECIMAL(4,2))", vec![text("12.345")]).await);
    println!("- (b) `SELECT CAST(? AS DECIMAL(4,2))` with '123.45': {}", first_binary(&mut conn, "SELECT CAST(? AS DECIMAL(4,2))", vec![text("123.45")]).await);
    println!("- (b) `SELECT CAST(? AS DECIMAL(65,30))` with 'abc': {}", first_binary(&mut conn, "SELECT CAST(? AS DECIMAL(65,30))", vec![text("abc")]).await);
    println!("- (b) `SELECT CAST(? AS SIGNED)` with NULL: {}", first_binary(&mut conn, "SELECT CAST(? AS SIGNED)", vec![Value::NULL]).await);

    println!("\n## a day of `big`, 5760 rows, through each binding, and whether the index is used\n");
    println!("| binding | count | plan |");
    println!("|---|---|---|");
    let (from, to) = (text("2026-10-02 00:00:00"), text("2026-10-03 00:00:00"));
    for (name, set) in [("(a) `SET @f = ?`, text", "?"), ("(a) `SET @f = CAST(? AS DATETIME(6))`", "CAST(? AS DATETIME(6))")] {
        let statement = conn.prep(format!("SET @f = {set}, @t = {set}")).await.expect("prep set");
        conn.exec_drop(&statement, vec![from.clone(), to.clone()]).await.expect("set");
        let count = first_text(&mut conn, "SELECT COUNT(*) FROM probe.big WHERE created_at >= @f AND created_at < @t").await;
        let plan: Vec<Row> = conn.query("EXPLAIN SELECT COUNT(*) FROM probe.big WHERE created_at >= @f AND created_at < @t").await.expect("explain");
        println!("| {name} | {count} | {} |", plan_of(&plan));
    }
    for (name, place) in [("(b) `?`, text", "?"), ("(b) `CAST(? AS DATETIME(6))`", "CAST(? AS DATETIME(6))")] {
        let sql = format!("SELECT COUNT(*) FROM probe.big WHERE created_at >= {place} AND created_at < {place}");
        let count = first_binary(&mut conn, &sql, vec![from.clone(), to.clone()]).await;
        let statement = conn.prep(format!("EXPLAIN {sql}")).await.expect("prep explain");
        let plan: Vec<Row> = conn.exec(&statement, vec![from.clone(), to.clone()]).await.expect("explain");
        println!("| {name} | {count} | {} |", plan_of(&plan));
    }
    let sql = "SELECT COUNT(*) FROM probe.big WHERE created_at >= ? AND created_at < ?";
    let dates = vec![Value::Date(2026, 10, 2, 0, 0, 0, 0), Value::Date(2026, 10, 3, 0, 0, 0, 0)];
    let count = first_binary(&mut conn, sql, dates.clone()).await;
    println!("| (b) `?`, a DATETIME value | {count} | |");
}

fn plan_of(rows: &[Row]) -> String {
    rows.first()
        .map(|row| {
            let columns = row.columns();
            let pick = |name: &str| {
                columns
                    .iter()
                    .position(|column| column.name_str() == name)
                    .and_then(|at| row.as_ref(at))
                    .map(|value| match value {
                        Value::Bytes(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                        other => shown_value(other),
                    })
                    .unwrap_or_default()
            };
            format!("type={} key={} rows={}", pick("type"), pick("key"), pick("rows"))
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------------------
// cap: a read takes the cap and one row more, then stops the statement on the server

const CAP: usize = 10_000;

async fn kill_query(server: Server, who: (&str, &str), id: u32) -> (Duration, String) {
    let started = Instant::now();
    let mut killer = connect(server, who).await;
    let outcome = match killer.query_drop(format!("KILL QUERY {id}")).await {
        Ok(()) => "killed".to_owned(),
        Err(error) => said(&error),
    };
    let _ = killer.disconnect().await;
    (started.elapsed(), outcome)
}

/// Reads up to the cap and one row more; then, when asked, kills the statement and reads what is left.
async fn capped(server: Server, sql: &str, select_limit: Option<u64>, kill: bool) -> String {
    let started = Instant::now();
    let mut conn = session(server, READER, 60_000, select_limit).await;
    let id = conn.id();
    let statement = conn.prep(sql).await.expect("prep");
    let mut result: QueryResult<'_, '_, BinaryProtocol> = conn.exec_iter(&statement, ()).await.expect("exec");
    let mut rows = 0usize;
    let mut first = None;
    while rows <= CAP {
        match result.next().await {
            Ok(Some(_)) => {
                rows += 1;
                first.get_or_insert(started.elapsed());
            }
            Ok(None) => break,
            Err(error) => return format!("failed after {rows} rows: {}", said(&error)),
        }
    }
    let at_cap = started.elapsed();
    if rows <= CAP {
        return format!("{rows} rows, all of them, first after {:?}, done after {at_cap:?}", first.unwrap_or_default());
    }
    if !kill {
        drop(result);
        drop(conn);
        return format!("cut at {rows} after {at_cap:?}; the connection dropped without a kill");
    }
    let (kill_took, killed) = kill_query(server, READER, id).await;
    let mut drained = 0usize;
    let ending = loop {
        match result.next().await {
            Ok(Some(_)) => drained += 1,
            Ok(None) => break "ended".to_owned(),
            Err(error) => break said(&error),
        }
    };
    drop(result);
    let _ = conn.disconnect().await;
    format!(
        "cut at {rows} after {at_cap:?} (first row {:?}); kill {killed} in {kill_took:?}; {drained} rows drained, then {ending}; all after {:?}",
        first.unwrap_or_default(),
        started.elapsed()
    )
}

/// The statements of `user` still running on the server, as root sees them.
async fn running(root: &mut Conn, user: &str) -> Vec<String> {
    let rows: Vec<Row> = root
        .exec(
            "SELECT ID, TIME, STATE, LEFT(INFO, 60) FROM information_schema.PROCESSLIST WHERE USER = ? AND INFO IS NOT NULL AND INFO NOT LIKE '%PROCESSLIST%'",
            (user,),
        )
        .await
        .unwrap_or_default();
    rows.into_iter().map(|row| format!("{:?}", row.unwrap())).collect()
}

async fn watch(root: &mut Conn, user: &str, seconds: u64) -> String {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        let now = running(root, user).await;
        if now.is_empty() {
            return format!("nothing of {user}'s runs after {:?}", started.elapsed());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    format!("still running after {seconds} s: {:?}", running(root, user).await)
}

async fn cap(server: Server) {
    let mut root = connect(server, ROOT).await;
    println!("# the row cap on {server:?}: {CAP} rows and one more\n");
    let limit = Some(CAP as u64 + 1);
    let cases: &[(&str, &str, Option<u64>, bool)] = &[
        ("168 000 rows, sql_select_limit", "SELECT * FROM probe.big", limit, true),
        ("168 000 rows, no limit, kill", "SELECT * FROM probe.big", None, true),
        ("LIMIT 1000000 wins over sql_select_limit, kill", "SELECT * FROM probe.big LIMIT 1000000", limit, true),
        ("sorted, sql_select_limit", "SELECT * FROM probe.big ORDER BY note", limit, true),
        ("sorted, no limit, kill", "SELECT * FROM probe.big ORDER BY note", None, true),
        ("endless join, LIMIT of its own, kill", "SELECT a.id, b.id FROM probe.big a, probe.big b LIMIT 100000000000", limit, true),
    ];
    for (name, sql, limit, kill) in cases {
        let said = capped(server, sql, *limit, *kill).await;
        let after = watch(&mut root, "reader", 5).await;
        println!("- {name}: `{sql}`: {said}; {after}");
    }
    let said = capped(server, "SELECT a.id, b.id FROM probe.big a, probe.big b LIMIT 100000000000", limit, false).await;
    let after = watch(&mut root, "reader", 10).await;
    println!("- endless join, dropped without a kill: {said}; {after}");
    let _ = root.query_drop("KILL USER reader").await;
    let _ = root.query_drop(format!("SELECT 1")).await;
    // kill anything the background drain left
    let left: Vec<Row> = root.query("SELECT ID FROM information_schema.PROCESSLIST WHERE USER = 'reader'").await.unwrap_or_default();
    for row in left {
        let id: u64 = row.get(0).unwrap_or(0);
        let _ = root.query_drop(format!("KILL {id}")).await;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// cancel: a timeout, the user's own hint against it, a kill without rights, a lost client

const HEAVY: &str = "SELECT COUNT(*) FROM probe.big a, probe.big b WHERE a.amount + b.amount < 0";

async fn timed(conn: &mut Conn, sql: &str) -> String {
    let started = Instant::now();
    let statement = match conn.prep(sql).await {
        Ok(statement) => statement,
        Err(error) => return format!("prepare failed: {}", said(&error)),
    };
    let outcome = match conn.exec::<Row, _, _>(&statement, ()).await {
        Ok(rows) => format!("{} rows", rows.len()),
        Err(error) => said(&error),
    };
    format!("{outcome} after {:.2?}", started.elapsed())
}

async fn cancel(server: Server) {
    let mut root = connect(server, ROOT).await;
    println!("# stopping a statement on {server:?}\n");

    let mut conn = session(server, READER, 2_000, None).await;
    println!("- the server's own timeout, 2 s: `{HEAVY}`: {}", timed(&mut conn, HEAVY).await);
    let hinted = match server {
        Server::MySql => format!("SELECT /*+ MAX_EXECUTION_TIME(600000) */ COUNT(*) FROM probe.big a, probe.big b WHERE a.amount + b.amount < 0"),
        Server::MariaDb => format!("SET STATEMENT max_statement_time = 600 FOR {HEAVY}"),
    };
    let mut conn = session(server, READER, 2_000, None).await;
    let id = conn.id();
    let killer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(6)).await;
        kill_query(server, READER, id).await
    });
    println!("- the statement's own longer timeout, the server's 2 s, our kill at 6 s: `{hinted}`: {}", timed(&mut conn, &hinted).await);
    println!("  kill as reader, who has no PROCESS or CONNECTION_ADMIN: {:?}", killer.await.unwrap());

    // a kill of someone else's statement
    let mut other = session(server, WRITER, 60_000, None).await;
    let other_id = other.id();
    let running_other = tokio::spawn(async move { timed(&mut other, HEAVY).await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("- reader kills writer's statement: {:?}", kill_query(server, READER, other_id).await);
    println!("- writer kills its own: {:?}", kill_query(server, WRITER, other_id).await);
    println!("  writer's statement: {}", running_other.await.unwrap());

    // a lost client: the process that runs the statement is killed
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["hold", if server == Server::MySql { "mysql" } else { "mariadb" }])
        .spawn()
        .expect("child");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let before = running(&mut root, "reader").await;
    child.kill().expect("kill child");
    let _ = child.wait();
    println!("- a client killed while its statement runs (60 s timeout): running before {before:?}; {}", watch(&mut root, "reader", 15).await);

    // a dropped future: the connection object is dropped while its statement runs
    let mut conn = session(server, READER, 60_000, None).await;
    let dropped = tokio::spawn(async move { timed(&mut conn, HEAVY).await });
    tokio::time::sleep(Duration::from_secs(1)).await;
    dropped.abort();
    let _ = dropped.await;
    println!("- a future dropped while its statement runs (60 s timeout): {}", watch(&mut root, "reader", 15).await);
    let left: Vec<Row> = root.query("SELECT ID FROM information_schema.PROCESSLIST WHERE USER IN ('reader', 'writer')").await.unwrap_or_default();
    for row in left {
        let id: u64 = row.get(0).unwrap_or(0);
        let _ = root.query_drop(format!("KILL {id}")).await;
    }

    // SLEEP under the server's timeout
    let mut conn = session(server, READER, 1_000, None).await;
    println!("- `SELECT SLEEP(5)` under a 1 s timeout: {} → {}", timed(&mut conn, "SELECT SLEEP(5)").await, first_text(&mut conn, "SELECT 'session alive'").await);
    let mut conn = session(server, READER, 1_000, None).await;
    println!("- `SHOW TABLES` timeout does not matter; `SELECT BENCHMARK(100000000, MD5('x'))` under 1 s: {}", timed(&mut conn, "SELECT BENCHMARK(100000000, MD5('x'))").await);
}

async fn hold(server: Server) {
    let mut conn = session(server, READER, 60_000, None).await;
    let _ = timed(&mut conn, HEAVY).await;
}

// ---------------------------------------------------------------------------------------------------------------
// zones

async fn zones(server: Server) {
    let mut conn = session(server, READER, 5_000, None).await;
    println!("# zones on {server:?}\n");
    for sql in [
        "SELECT COUNT(*) FROM mysql.time_zone_name",
        "SELECT CONVERT_TZ('2026-10-25 00:30:00', '+00:00', 'Europe/Berlin')",
        "SELECT CONVERT_TZ('2026-10-25 01:30:00', '+00:00', 'Europe/Berlin')",
        "SELECT CONVERT_TZ('2026-11-01 00:00:00', '+00:00', 'Asia/Kolkata')",
        "SELECT CONVERT_TZ('2026-11-01 00:00:00', '+00:00', 'Nowhere/Zone')",
        "SELECT @@session.time_zone, @@system_time_zone",
    ] {
        println!("- `{sql}` as reader: {}", first_text(&mut conn, sql).await);
    }
    let mut root = connect(server, ROOT).await;
    println!("- `SELECT COUNT(*) FROM mysql.time_zone_name` as root: {}", first_text(&mut root, "SELECT COUNT(*) FROM mysql.time_zone_name").await);
    println!("\n## one TIMESTAMP and one DATETIME holding the same 15 seconds past 2026-10-01 00:00 UTC, row 1 of big\n");
    for zone in ["+00:00", "Europe/Moscow"] {
        conn.query_drop(format!("SET time_zone = '{zone}'")).await.expect("zone");
        let text: Vec<Row> = conn.query("SELECT created_at, stamped, UNIX_TIMESTAMP(created_at) FROM probe.big WHERE id = 1").await.expect("text");
        let statement = conn.prep("SELECT created_at, stamped FROM probe.big WHERE id = 1").await.expect("prep");
        let binary: Vec<Row> = conn.exec(&statement, ()).await.expect("binary");
        let count = first_binary(
            &mut conn,
            "SELECT COUNT(*) FROM probe.big WHERE created_at >= CAST(? AS DATETIME(6)) AND created_at < CAST(? AS DATETIME(6))",
            vec![Value::from("2026-10-02 00:00:00"), Value::from("2026-10-03 00:00:00")],
        )
        .await;
        let first_hour = first_binary(
            &mut conn,
            "SELECT MIN(id) FROM probe.big WHERE created_at >= CAST(? AS DATETIME(6))",
            vec![Value::from("2026-10-02 00:00:00")],
        )
        .await;
        println!(
            "- session `{zone}`: text {:?}; binary {:?}; a UTC day bound as text counts {count}, its first id {first_hour} (5761 is right)",
            text[0].clone().unwrap().iter().map(shown_value).collect::<Vec<_>>(),
            binary[0].clone().unwrap().iter().map(shown_value).collect::<Vec<_>>(),
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// latency and round trips

fn median(mut values: Vec<Duration>) -> (Duration, Duration) {
    values.sort();
    (values[values.len() / 2], values[values.len() * 9 / 10])
}

const DAY: &str = "SELECT COUNT(*) FROM probe.big WHERE created_at >= CAST(? AS DATETIME(6)) AND created_at < CAST(? AS DATETIME(6))";
const DAY_BY_VARIABLES: &str = "SELECT COUNT(*) FROM probe.big WHERE created_at >= @from AND created_at < @to";

fn day() -> Vec<Value> {
    vec![Value::from("2026-10-02 00:00:00"), Value::from("2026-10-03 00:00:00")]
}

/// A read whole: connect, the session, the statement, a rollback and goodbye. (b) prepares the statement; (a) sets
/// the variables with a prepared SET and sends the statement as text.
async fn read_once(server: Server, by_variables: bool) -> (Duration, u32) {
    let started = Instant::now();
    let mut conn = session(server, READER, 5_000, None).await;
    let id = conn.id();
    if by_variables {
        let set = conn.prep("SET @from = CAST(? AS DATETIME(6)), @to = CAST(? AS DATETIME(6))").await.expect("prep set");
        conn.exec_drop(&set, day()).await.expect("set");
        let _: Vec<Row> = conn.query(DAY_BY_VARIABLES).await.expect("query");
    } else {
        let statement = conn.prep(DAY).await.expect("prep");
        assert!(!statement.columns().is_empty());
        let _: Vec<Row> = conn.exec(&statement, day()).await.expect("exec");
    }
    conn.query_drop("ROLLBACK").await.expect("rollback");
    conn.disconnect().await.expect("bye");
    (started.elapsed(), id)
}

async fn latency(server: Server) {
    const RUNS: usize = 100;
    println!("# latency on {server:?}, {RUNS} runs each, median and 90th percentile\n");
    let mut connects = Vec::new();
    for _ in 0..RUNS {
        let started = Instant::now();
        let conn = connect(server, READER).await;
        connects.push(started.elapsed());
        conn.disconnect().await.expect("bye");
    }
    println!("- connect and authenticate: {:?}", median(connects));
    for (name, by_variables) in [("(b) prepared statement", false), ("(a) prepared SET, statement as text", true)] {
        let mut times = Vec::new();
        for _ in 0..RUNS {
            times.push(read_once(server, by_variables).await.0);
        }
        println!("- a read whole, {name}: {:?}", median(times));
    }
    let mut conn = session(server, READER, 5_000, None).await;
    for (name, which) in [("(b) prep + exec", 0), ("(a) set + text", 1), ("text, values written in", 2)] {
        let mut times = Vec::new();
        for _ in 0..RUNS {
            let started = Instant::now();
            match which {
                0 => {
                    let statement = conn.prep(DAY).await.expect("prep");
                    let _: Vec<Row> = conn.exec(&statement, day()).await.expect("exec");
                    conn.close(statement).await.expect("close");
                }
                1 => {
                    let set = conn.prep("SET @from = CAST(? AS DATETIME(6)), @to = CAST(? AS DATETIME(6))").await.expect("prep");
                    conn.exec_drop(&set, day()).await.expect("set");
                    conn.close(set).await.expect("close");
                    let _: Vec<Row> = conn.query(DAY_BY_VARIABLES).await.expect("query");
                }
                _ => {
                    let _: Vec<Row> = conn
                        .query("SELECT COUNT(*) FROM probe.big WHERE created_at >= '2026-10-02 00:00:00' AND created_at < '2026-10-03 00:00:00'")
                        .await
                        .expect("query");
                }
            }
            times.push(started.elapsed());
        }
        println!("- the statement alone on an open session, {name}: {:?}", median(times));
    }
}

async fn trips(server: Server) {
    let mut root = connect(server, ROOT).await;
    root.query_drop("SET GLOBAL log_output = 'TABLE'").await.expect("log output");
    root.query_drop("TRUNCATE mysql.general_log").await.expect("truncate");
    root.query_drop("SET GLOBAL general_log = 'ON'").await.expect("log on");
    let (_, b) = read_once(server, false).await;
    let (_, a) = read_once(server, true).await;
    root.query_drop("SET GLOBAL general_log = 'OFF'").await.expect("log off");
    println!("# what one read sends on {server:?}, from the server's general log\n");
    for (name, id) in [("(b)", b), ("(a)", a)] {
        let rows: Vec<Row> = root
            .exec("SELECT command_type, LEFT(CONVERT(argument USING utf8mb4), 110) FROM mysql.general_log WHERE thread_id = ? ORDER BY event_time", (id,))
            .await
            .expect("log");
        println!("{name}, {} commands:", rows.len());
        for row in rows {
            let values = row.unwrap();
            let text = |value: &Value| match value {
                Value::Bytes(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                other => shown_value(other),
            };
            println!("    {:<12} {}", text(&values[0]), text(&values[1]));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// memory: the peak of a process that reads the cap, on Linux

fn peak() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status.lines().find(|line| line.starts_with("VmHWM")).unwrap_or("VmHWM unknown").split_whitespace().skip(1).collect::<Vec<_>>().join(" ")
}

async fn memory(server: Server) {
    let mut conn = session(server, READER, 60_000, Some(CAP as u64 + 1)).await;
    println!("- connected, in its session: peak {}", peak());
    let statement = conn.prep("SELECT * FROM probe.big").await.expect("prep");
    let mut result: QueryResult<'_, '_, BinaryProtocol> = conn.exec_iter(&statement, ()).await.expect("exec");
    let mut streamed = 0;
    while let Some(row) = result.next().await.expect("row") {
        streamed += row.len();
    }
    drop(result);
    println!("- {CAP} rows and one more streamed, a row at a time: peak {} ({streamed} values)", peak());
    let rows: Vec<Row> = conn.exec(&statement, ()).await.expect("exec");
    println!("- the same rows held at once as mysql_async's rows: peak {} ({} rows)", peak(), rows.len());
    let _ = (TextProtocol, ColumnType::MYSQL_TYPE_NULL);
}

// ---------------------------------------------------------------------------------------------------------------
// followup: what a second review asked of the form, before slice 6 is written

async fn followup(server: Server) {
    println!("# the second review's questions on {server:?}\n");

    // how the server reads a quote and a backslash under each sql_mode
    let mut conn = connect(server, READER).await;
    for mode in ["", "NO_BACKSLASH_ESCAPES", "ANSI_QUOTES", "ANSI", "ORACLE", "MSSQL"] {
        if let Err(error) = conn.query_drop(format!("SET SESSION sql_mode = '{mode}'")).await {
            println!("- sql_mode '{mode}': refused, {}", said(&error));
            continue;
        }
        let shown = first_text(&mut conn, "SELECT @@sql_mode").await;
        let backslash = first_text(&mut conn, r#"SELECT 'x\' AS a -- '"#).await;
        let doubled = first_text(&mut conn, r#"SELECT "y" AS b"#).await;
        println!("- sql_mode '{mode}': reads as {shown}; `SELECT 'x\' AS a -- '` → {backslash}; `SELECT \"y\" AS b` → {doubled}");
    }
    drop(conn);

    // SET NAMES beside other assignments in one SET
    let mut conn = connect(server, READER).await;
    let combined = conn
        .query_drop("SET NAMES utf8mb4, SESSION transaction_read_only = ON, time_zone = '+00:00'")
        .await
        .map(|_| "accepted".to_owned())
        .unwrap_or_else(|error| said(&error));
    println!("\n- `SET NAMES utf8mb4, SESSION transaction_read_only = ON, time_zone = '+00:00'`: {combined}; then {}",
        first_text(&mut conn, "SELECT CONCAT(@@character_set_results, ' ', @@transaction_read_only, ' ', @@time_zone)").await);
    let _ = conn.disconnect().await;

    // a named lock against a rollback and a closed connection
    let mut holder = session(server, READER, 5_000, None).await;
    let mut other = connect(server, READER).await;
    let took = first_text(&mut holder, "SELECT GET_LOCK('probe-lock', 0)").await;
    holder.query_drop("ROLLBACK").await.expect("rollback");
    let after_rollback = first_text(&mut other, "SELECT IS_FREE_LOCK('probe-lock')").await;
    holder.disconnect().await.expect("bye");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after_close = first_text(&mut other, "SELECT IS_FREE_LOCK('probe-lock')").await;
    println!("\n- `GET_LOCK` in a read: took {took}; free after ROLLBACK: {after_rollback}; free after the connection closed: {after_close}");

    // a kill that arrives after its statement ended
    let mut late = Vec::new();
    for _ in 0..5 {
        let mut conn = connect(server, READER).await;
        let _ = first_text(&mut conn, "SELECT 1").await;
        let (_, killed) = kill_query(server, READER, conn.id()).await;
        let next = first_text(&mut conn, "SELECT 'next statement ran'").await;
        late.push(format!("kill {killed}, then {next}"));
        let _ = conn.disconnect().await;
    }
    println!("\n- `KILL QUERY` sent after its statement ended, five times: {late:?}");

    // the transaction after its statement is killed
    let mut conn = session(server, READER, 60_000, None).await;
    let id = conn.id();
    let killer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        kill_query(server, READER, id).await
    });
    let killed = timed(&mut conn, HEAVY).await;
    let _ = killer.await;
    let still = conn
        .query_drop("SET TRANSACTION READ WRITE")
        .await
        .map(|_| "no transaction open".to_owned())
        .unwrap_or_else(|error| said(&error));
    let read_only = first_text(&mut conn, "SELECT @@transaction_read_only").await;
    println!("\n- a statement killed inside START TRANSACTION READ ONLY: {killed}; then `SET TRANSACTION READ WRITE` says {still}; session read-only {read_only}");
    let _ = conn.disconnect().await;
}
