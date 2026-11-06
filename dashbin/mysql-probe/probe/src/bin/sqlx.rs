//! The same read through sqlx-mysql, for what differs from mysql_async: its own session settings, how it
//! authenticates without TLS, how long a connection takes, and what a row gives back.
//!
//! `sqlx <mysql|mariadb>`

use std::time::{Duration, Instant};

use sqlx_core::connection::{ConnectOptions, Connection};
use sqlx_core::executor::Executor;
use sqlx_core::row::Row;
use sqlx_core::sql_str::AssertSqlSafe;
use sqlx_mysql::{MySqlConnectOptions, MySqlConnection, MySqlSslMode};

fn options(port: u16, user: &str, password: &str, plain: bool) -> MySqlConnectOptions {
    // in a container beside the servers, PROBE_HOST names the compose network and each server listens on 3306
    let (host, port) = match std::env::var("PROBE_HOST") {
        Ok(_) => (if port == 13307 { "mariadb" } else { "mysql" }, 3306),
        Err(_) => ("127.0.0.1", port),
    };
    let options = MySqlConnectOptions::new()
        .host(host)
        .port(port)
        .username(user)
        .password(password)
        .database("probe")
        .ssl_mode(MySqlSslMode::Disabled);
    if plain {
        options.pipes_as_concat(false).no_engine_substitution(false).timezone(None).set_names(false)
    } else {
        options
    }
}

async fn connect(port: u16, plain: bool) -> Result<MySqlConnection, sqlx_core::Error> {
    options(port, "reader", "probe-reader", plain).connect().await
}

fn median(mut values: Vec<Duration>) -> (Duration, Duration) {
    values.sort();
    (values[values.len() / 2], values[values.len() * 9 / 10])
}

const DAY: &str = "SELECT COUNT(*) FROM probe.big WHERE created_at >= CAST(? AS DATETIME(6)) AND created_at < CAST(? AS DATETIME(6))";

async fn read_once(port: u16) -> Duration {
    let started = Instant::now();
    let mut conn = connect(port, true).await.expect("connect");
    conn.execute("SET SESSION transaction_read_only = ON, time_zone = '+00:00'").await.expect("session");
    conn.execute("START TRANSACTION READ ONLY").await.expect("read only");
    let rows = sqlx_core::query::query(DAY)
        .bind("2026-10-02 00:00:00")
        .bind("2026-10-03 00:00:00")
        .fetch_all(&mut conn)
        .await
        .expect("exec");
    assert_eq!(rows.len(), 1);
    conn.execute("ROLLBACK").await.expect("rollback");
    conn.close().await.expect("bye");
    started.elapsed()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let server = std::env::args().nth(1).unwrap_or_else(|| "mysql".to_owned());
    let port = if server == "mariadb" { 13307 } else { 13306 };
    if std::env::args().nth(2).as_deref() == Some("cap") {
        return cap(port).await;
    }
    if std::env::args().nth(2).as_deref() == Some("charset") {
        let mut conn = connect(port, true).await.expect("connect");
        let row = sqlx_core::query::query("SELECT CAST(CONCAT(@@character_set_client, ' ', @@character_set_results, ' ', @@collation_connection) AS CHAR), word, latin FROM probe.types WHERE id = 1")
            .fetch_one(&mut conn).await.expect("charset");
        println!("- sqlx without its SET NAMES: {:?}; word {:?}; latin {:?}", row.try_get::<String, _>(0).ok(), row.try_get::<String, _>(1).ok(), row.try_get::<String, _>(2).ok());
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("memory") {
        return memory(port).await;
    }
    if std::env::args().nth(2).as_deref() == Some("trips") {
        return trips(port).await;
    }
    if std::env::args().nth(2).as_deref() == Some("connect") {
        match connect(port, true).await {
            Ok(_) => println!("sqlx connected as reader"),
            Err(error) => println!("sqlx failed: {error}"),
        }
        return;
    }
    println!("# sqlx-mysql 0.9 on {server}\n");

    let mut connects = Vec::new();
    for _ in 0..100 {
        let started = Instant::now();
        let conn = connect(port, true).await.expect("connect");
        connects.push(started.elapsed());
        conn.close().await.expect("bye");
    }
    println!("- connect and authenticate, its own settings off: {:?}", median(connects));
    let mut defaults = Vec::new();
    for _ in 0..100 {
        let started = Instant::now();
        let conn = connect(port, false).await.expect("connect");
        defaults.push(started.elapsed());
        conn.close().await.expect("bye");
    }
    println!("- connect and authenticate, its own settings on: {:?}", median(defaults));
    let mut conn = connect(port, false).await.expect("connect");
    let row = sqlx_core::query::query("SELECT @@session.sql_mode, @@session.time_zone, 'a' || 'b'")
        .fetch_one(&mut conn)
        .await
        .expect("settings");
    println!(
        "- what its own settings make of a session: sql_mode {:?}, time_zone {:?}, `'a' || 'b'` = {:?}",
        row.try_get::<String, _>(0).ok(),
        row.try_get::<String, _>(1).ok(),
        row.try_get_unchecked::<&[u8], _>(2).map(|bytes| String::from_utf8_lossy(bytes).into_owned()).ok(),
    );

    let mut reads = Vec::new();
    for _ in 0..100 {
        reads.push(read_once(port).await);
    }
    println!("- a read whole, prepared: {:?}", median(reads));

    // a row of edges: what a column gives without a decoder of its type
    let mut conn = connect(port, true).await.expect("connect");
    let rows = sqlx_core::query::query("SELECT d65, u64, wall6, instant6, doc FROM probe.types WHERE id = 1")
        .fetch_all(&mut conn)
        .await
        .expect("types");
    for (at, name) in ["d65", "u64", "wall6", "instant6", "doc"].iter().enumerate() {
        let checked = rows[0].try_get::<&str, _>(at).map(str::to_owned).map_err(|error| error.to_string());
        let raw = rows[0].try_get_unchecked::<&[u8], _>(at).map(|bytes| format!("{bytes:02X?}")).map_err(|error| error.to_string());
        println!("- {name} as &str: {checked:?}; its bytes unchecked: {raw:?}");
    }
}

async fn root(port: u16) -> MySqlConnection {
    options(port, "root", "probe-root", true).connect().await.expect("root")
}

async fn id_of(conn: &mut MySqlConnection) -> u64 {
    let row = sqlx_core::query::query("SELECT CONNECTION_ID()").fetch_one(&mut *conn).await.expect("id");
    row.try_get::<u64, _>(0).expect("id")
}

async fn running(root: &mut MySqlConnection) -> usize {
    let rows = sqlx_core::query::query(
        "SELECT ID FROM information_schema.PROCESSLIST WHERE USER = 'reader' AND INFO IS NOT NULL AND INFO NOT LIKE '%PROCESSLIST%'",
    )
    .fetch_all(&mut *root)
    .await
    .expect("processlist");
    rows.len()
}

async fn watch(root: &mut MySqlConnection, seconds: u64) -> String {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        if running(root).await == 0 {
            return format!("nothing of reader's runs after {:?}", started.elapsed());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    format!("still running after {seconds} s")
}

/// The cap and one row more through sqlx's stream, then a kill from a second connection, or a drop without one.
async fn cap(port: u16) {
    use futures_util::TryStreamExt;
    const CAP: usize = 10_000;
    let mut root = root(port).await;
    println!("# the row cap through sqlx on {port}\n");
    for (name, sql, kill) in [
        ("168 000 rows, kill", "SELECT * FROM probe.big", true),
        ("endless join, kill", "SELECT a.id, b.id FROM probe.big a, probe.big b LIMIT 100000000000", true),
        ("endless join, dropped without a kill", "SELECT a.id, b.id FROM probe.big a, probe.big b LIMIT 100000000000", false),
    ] {
        let started = Instant::now();
        let mut conn = connect(port, true).await.expect("connect");
        conn.execute("SET SESSION transaction_read_only = ON, time_zone = '+00:00'").await.expect("session");
        let id = id_of(&mut conn).await;
        let mut rows = 0usize;
        let mut drained = 0usize;
        let mut ending = String::new();
        {
            let mut stream = sqlx_core::query::query(AssertSqlSafe(sql.to_owned())).fetch(&mut conn);
            while rows <= CAP {
                match stream.try_next().await {
                    Ok(Some(_)) => rows += 1,
                    Ok(None) => break,
                    Err(error) => {
                        ending = error.to_string();
                        break;
                    }
                }
            }
            if kill {
                let mut killer = connect(port, true).await.expect("killer");
                sqlx_core::query::query(AssertSqlSafe(format!("KILL QUERY {id}"))).execute(&mut killer).await.expect("kill");
                loop {
                    match stream.try_next().await {
                        Ok(Some(_)) => drained += 1,
                        Ok(None) => break ending = "ended".to_owned(),
                        Err(error) => break ending = error.to_string(),
                    }
                }
            }
        }
        let at_cap = started.elapsed();
        if kill {
            let closed = Instant::now();
            let closing = conn.close().await.map(|_| "closed".to_owned()).unwrap_or_else(|error| error.to_string());
            println!(
                "- {name}: cut at {rows}, {drained} rows drained, then {ending}; {at_cap:?}; close: {closing} in {:?}; {}",
                closed.elapsed(),
                watch(&mut root, 5).await
            );
        } else {
            drop(conn);
            println!("- {name}: cut at {rows} after {at_cap:?}, connection dropped; {}", watch(&mut root, 10).await);
            let _ = sqlx_core::query::query(AssertSqlSafe(format!("KILL {id}"))).execute(&mut root).await;
        }
    }
}

async fn trips(port: u16) {
    let mut root = root(port).await;
    for sql in ["SET GLOBAL log_output = 'TABLE'", "TRUNCATE mysql.general_log", "SET GLOBAL general_log = 'ON'"] {
        sqlx_core::query::query(sql).execute(&mut root).await.expect(sql);
    }
    let mut conn = connect(port, true).await.expect("connect");
    let id = id_of(&mut conn).await;
    conn.execute("SET SESSION transaction_read_only = ON, time_zone = '+00:00'").await.expect("session");
    conn.execute("START TRANSACTION READ ONLY").await.expect("read only");
    let rows = sqlx_core::query::query(DAY).bind("2026-10-02 00:00:00").bind("2026-10-03 00:00:00").fetch_all(&mut conn).await.expect("exec");
    assert_eq!(rows.len(), 1);
    conn.execute("ROLLBACK").await.expect("rollback");
    conn.close().await.expect("bye");
    sqlx_core::query::query("SET GLOBAL general_log = 'OFF'").execute(&mut root).await.expect("log off");
    let log = sqlx_core::query::query(
        "SELECT CAST(command_type AS CHAR), CAST(LEFT(CONVERT(argument USING utf8mb4), 100) AS CHAR) FROM mysql.general_log WHERE thread_id = ? ORDER BY event_time",
    )
    .bind(id)
    .fetch_all(&mut root)
    .await
    .expect("log");
    println!("# what one read sends through sqlx on {port}, {} commands (one asks its id)\n", log.len());
    for row in log {
        println!("    {:<12} {}", row.try_get::<String, _>(0).unwrap_or_default(), row.try_get::<String, _>(1).unwrap_or_default());
    }
}

fn peak() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status.lines().find(|line| line.starts_with("VmHWM")).unwrap_or("VmHWM unknown").split_whitespace().skip(1).collect::<Vec<_>>().join(" ")
}

async fn memory(port: u16) {
    use futures_util::TryStreamExt;
    let mut conn = connect(port, true).await.expect("connect");
    conn.execute("SET SESSION transaction_read_only = ON, time_zone = '+00:00', sql_select_limit = 10001").await.expect("session");
    println!("- connected, in its session: peak {}", peak());
    let mut values = 0;
    {
        let mut stream = sqlx_core::query::query("SELECT * FROM probe.big WHERE id > ?").bind(0).fetch(&mut conn);
        while let Some(row) = stream.try_next().await.expect("row") {
            values += row.len();
        }
    }
    println!("- 10000 rows and one more streamed, a row at a time: peak {} ({values} values)", peak());
    let rows = sqlx_core::query::query("SELECT * FROM probe.big WHERE id > ?").bind(0).fetch_all(&mut conn).await.expect("all");
    println!("- the same rows held at once as sqlx's rows: peak {} ({} rows)", peak(), rows.len());
}
