// an agent without the web: a socket to the hub, postgres, mysql and one quickjs plugin
#[path = "common.rs"]
mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mysql_async::prelude::Queryable;
use rquickjs::{Context, Function, Runtime};
use tokio_tungstenite::tungstenite::Message;

const PG_ROWS: &str = "select g, now() - g * interval '1 second', random() from generate_series(1, 10000) g";
const MY_ROWS: &str = "with recursive c(x) as (select 1 union all select x + 1 from c where x < 10000) select x, rand() from c";

#[tokio::main]
async fn main() {
    println!("allocator: {}", common::allocator());
    let hub = std::env::var("HUB").unwrap_or_else(|_| "ws://hub:8080/agent".into());
    let (socket, _) = tokio_tungstenite::connect_async(hub).await.unwrap();
    let (mut to_hub, mut from_hub) = socket.split();
    tokio::spawn(async move { while let Some(Ok(_)) = from_hub.next().await {} });
    tokio::time::sleep(Duration::from_secs(3)).await;
    common::checkpoint("connected to the hub, idle");

    let (pg, connection) = tokio_postgres::connect(&std::env::var("PG").unwrap(), tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(connection);
    let mysql = mysql_async::Pool::new(std::env::var("MY").unwrap().as_str());
    let mut my = mysql.get_conn().await.unwrap();
    my.query_drop("set session cte_max_recursion_depth = 20000").await.unwrap();
    let rt = Runtime::new().unwrap();
    rt.set_memory_limit(16 << 20);
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| ctx.eval::<(), _>(common::PLUGIN).unwrap());
    tokio::time::sleep(Duration::from_secs(1)).await;
    common::checkpoint("plus a pg and a mysql connection, a plugin");

    #[cfg(feature = "buffer")]
    let (buffer, stats) = {
        let dir = std::path::PathBuf::from("/tmp/agent-buffer");
        let _ = std::fs::remove_dir_all(&dir);
        let store = tinystore::aio::Store::open(&dir, tinystore::Options::default()).unwrap();
        let stats = store.metrics().unwrap();
        common::checkpoint("plus a tinystore with metrics open");
        (store, stats)
    };

    let started = std::time::Instant::now();
    for round in 0..100 {
        let rows = pg.query(PG_ROWS, &[]).await.unwrap();
        let my_rows: Vec<(i64, f64)> = my.query(MY_ROWS).await.unwrap();
        let sum = ctx.with(|ctx| {
            let query: Function = ctx.globals().get("query").unwrap();
            query.call::<_, f64>((10_000,)).unwrap()
        });
        #[cfg(feature = "buffer")]
        {
            let now = std::time::SystemTime::now();
            let series: Vec<tinystore::metrics::Series> = (0..100)
                .map(|i| tinystore::metrics::Series::gauge("scraped").label("i", i).sample(now, round as f64))
                .collect();
            stats.ingest(&series).await.unwrap();
        }
        to_hub.send(Message::text(format!("{round} {} {} {sum}", rows.len(), my_rows.len()))).await.unwrap();
        if round == 0 {
            common::checkpoint("after one round of pg, mysql, plugin");
        }
    }
    println!("  speed: 100 rounds took {:?}", started.elapsed());
    common::checkpoint("after 100 rounds");
    tokio::time::sleep(Duration::from_secs(10)).await;
    common::checkpoint("10 s later");
    let how = common::trim();
    common::checkpoint(&format!("forced: {how}"));
    #[cfg(feature = "buffer")]
    buffer.close().await.unwrap();
}
