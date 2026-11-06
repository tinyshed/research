//! A measurement harness, never committed: a MySQL read as a plugin would make one, to weigh its driver.

use mysql_async::prelude::*;
use mysql_async::{Conn, OptsBuilder, Value};

pub async fn mysql(host: &str) -> Result<usize, mysql_async::Error> {
    let opts = OptsBuilder::default()
        .ip_or_hostname(host)
        .tcp_port(3306)
        .user(Some("reader"))
        .pass(Some(host))
        .db_name(Some("probe"))
        .prefer_socket(false)
        .stmt_cache_size(0);
    #[cfg(feature = "tls")]
    let opts = opts.ssl_opts(host.starts_with("tls").then(mysql_async::SslOpts::default));
    let mut conn = Conn::new(opts).await?;
    conn.query_drop("SET SESSION transaction_read_only = ON").await?;
    let statement = conn.prep(host).await?;
    let mut seen = statement.columns().len();
    let mut result = conn.exec_iter(&statement, vec![Value::from(host)]).await?;
    while let Some(row) = result.next().await? {
        for value in row.unwrap() {
            seen += match value {
                Value::Date(year, ..) => usize::from(year),
                Value::Time(_, days, ..) => days as usize,
                Value::Bytes(bytes) => bytes.len(),
                Value::Int(int) => int as usize,
                Value::UInt(int) => int as usize,
                Value::Double(double) => double as usize,
                Value::Float(float) => float as usize,
                Value::NULL => 0,
            };
        }
    }
    drop(result);
    let id = conn.id();
    let mut killer = Conn::new(OptsBuilder::default().ip_or_hostname(host)).await?;
    killer.query_drop(format!("KILL QUERY {id}")).await?;
    conn.disconnect().await?;
    Ok(seen)
}
