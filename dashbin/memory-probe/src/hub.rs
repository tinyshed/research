// axum with tinystore::aio embedded, the engines a hub would open, and a socket for agents
#[path = "common.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::extract::State;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::{Router, routing::get};
use serde::{Deserialize, Serialize};
use tinystore::Options;
use tinystore::aio::Store;
use tinystore::metrics::{Op, Series};
use tracing_subscriber::prelude::*;

#[derive(Serialize, Deserialize, Clone)]
struct Session {
    user: i64,
    role: String,
}

struct App {
    db: tinystore::aio::sql::Database,
    sessions: tinystore::aio::kv::Bucket<Session>,
    events: tinystore::aio::records::Events,
    stats: tinystore::aio::metrics::Metrics,
}

#[tokio::main]
async fn main() -> tinystore::Result<()> {
    let dir = std::path::PathBuf::from("/tmp/hub-data");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let log = store.logger("hub").layer()?;
    tracing_subscriber::registry().with(log).init();

    let db = store.database("meta").open()?;
    db.exec("create table if not exists dashboards(id integer primary key, name text not null, spec text not null)").await?;
    for i in 0..20 {
        db.exec(tinystore::sql!("insert into dashboards(id, name, spec) values (?, ?, ?)", i, format!("dashboard {i}"), "{}"))
            .await?;
    }
    let sessions = db.bucket::<Session>("sessions").idle(Duration::from_hours(24 * 30)).open()?;
    sessions.set("s-42", &Session { user: 42, role: "owner".into() }).await?;
    let events = store.events("audit").keep(Duration::from_hours(24 * 365)).open()?;
    let stats = store.metrics()?;
    let _collect = store.queue::<String>("collect").open()?;

    let app = Arc::new(App { db, sessions, events, stats });
    let router = Router::new()
        .route("/", get(|| async { "ok" }))
        .route("/rss", get(|| async { common::rss() }))
        .route("/trim", get(|| async { format!("{}  ({})", common::rss_after_trim(), common::allocator()) }))
        .route("/seed", get(seed))
        .route("/work", get(work))
        .route("/panel", get(panel))
        .route("/agent", get(agent))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening");
    axum::serve(listener, router).await.unwrap();
    store.close().await
}

// an hour of 15-second samples for 50 hosts, what a small monitoring setup holds
async fn seed(State(app): State<Arc<App>>) -> String {
    let now = SystemTime::now();
    for half in 0..2 {
        let series: Vec<Series> = (half * 25..half * 25 + 25)
            .map(|host| {
                let mut s = Series::gauge("node_load").label("host", format!("site-{host}"));
                for i in 0..240u64 {
                    s = s.sample(now - Duration::from_secs(15 * (240 - i)), (i % 17) as f64 * 0.1 + host as f64);
                }
                s
            })
            .collect();
        app.stats.ingest(&series).await.unwrap();
    }
    "seeded".into()
}

// what a page load costs: the session, a listing, an audit event, a log line
async fn work(State(app): State<Arc<App>>) -> String {
    let session = app.sessions.get("s-42").await.unwrap();
    let rows: Vec<(i64, String)> = app.db.all("select id, name from dashboards order by id limit 20").await.unwrap();
    app.events.append("dashboard.viewed", &serde_json::json!({ "dashboard": 7, "user": session.map(|s| s.user) })).await.unwrap();
    tracing::info!(rows = rows.len(), "work");
    format!("{}", rows.len())
}

// one panel: average load by host, by the minute, over the last hour
async fn panel(State(app): State<Arc<App>>) -> String {
    let answer = app
        .stats
        .query()
        .name("node_load")
        .since(Duration::from_hours(1))
        .every(Duration::from_mins(1))
        .by(&["host"])
        .aggregate(Op::Avg)
        .await
        .unwrap();
    format!("{}", answer.len())
}

async fn agent(upgrade: WebSocketUpgrade) -> impl IntoResponse {
    upgrade.on_upgrade(|mut socket: WebSocket| async move { while let Some(Ok(_)) = socket.recv().await {} })
}
