//! A measurement harness, never committed: a MySQL read as a plugin would make one, to weigh its driver.

use sqlx_core::connection::{ConnectOptions, Connection};
use sqlx_core::executor::Executor;
use sqlx_core::row::Row;
use sqlx_core::sql_str::{AssertSqlSafe, SqlSafeStr};
use sqlx_core::statement::Statement;
use sqlx_mysql::{MySqlConnectOptions, MySqlSslMode};

pub async fn mysql(host: &str) -> Result<usize, sqlx_core::Error> {
    let options = MySqlConnectOptions::new()
        .host(host)
        .username("reader")
        .password(host)
        .database("probe")
        .ssl_mode(MySqlSslMode::Disabled)
        .pipes_as_concat(false)
        .timezone(None)
        .set_names(false)
        .no_engine_substitution(false);
    let mut conn = options.connect().await?;
    sqlx_core::query::query("SET SESSION transaction_read_only = ON").execute(&mut conn).await?;
    let statement = (&mut conn).prepare(AssertSqlSafe(host.to_owned()).into_sql_str()).await?;
    let mut seen = statement.columns().len();
    let rows = sqlx_core::query::query(AssertSqlSafe(host.to_owned())).bind(host.to_owned()).fetch_all(&mut conn).await?;
    for row in rows {
        for at in 0..row.len() {
            seen += row.try_get_unchecked::<&[u8], _>(at)?.len();
        }
    }
    let mut killer = MySqlConnectOptions::new().host(host).connect().await?;
    sqlx_core::query::query(AssertSqlSafe(format!("KILL QUERY {seen}"))).execute(&mut killer).await?;
    conn.close().await?;
    Ok(seen)
}
