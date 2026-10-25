// one process as dashbin would run it: axum and tinystore::aio on the main runtime,
// the plugin instances on a thread of their own, since a quickjs runtime never leaves the thread it was made on
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Router, routing::get};
use tinystore::Options;
use tinystore::aio::Store;
use tinystore::metrics::{Op, Series};
use tokio::sync::{mpsc, oneshot};
use tracing_subscriber::prelude::*;

use crate::Instance;
use crate::calls::{Call, Deadline, Outcome, call, discover_limited, first_line, host_of, instance_limited, settle, RANGE};

const INSTANCE_MEMORY: usize = 32 << 20;

enum Job {
    Call { instance: String, reply: oneshot::Sender<Result<String, String>> },
    Install { bundle: String, limit: usize, reply: oneshot::Sender<String> },
    Held { reply: oneshot::Sender<String> },
}

struct Loaded {
    instance: Instance,
    deadline: Deadline,
    function: &'static str,
    settings: String,
    // a runtime runs one call at a time (chapter 5)
    turn: tokio::sync::Mutex<()>,
}

// what each kind of plugin is asked, and where its mock answers
fn kind_of(bundle: &str) -> (&'static str, String) {
    let stripe = std::env::var("STRIPE_MOCK").unwrap_or_default();
    let mock = std::env::var("SDK_MOCK").unwrap_or_default();
    match bundle.rsplit('/').next().unwrap_or(bundle) {
        "stripe.js" => ("charges", format!(r#"{{"key":"sk_test_123","api":"{stripe}"}}"#)),
        "openai.js" => ("reply", format!(r#"{{"key":"sk-test","api":"{mock}"}}"#)),
        "notion.js" => ("pages", format!(r#"{{"key":"secret_test","api":"{mock}"}}"#)),
        "octokit.js" => ("repo", format!(r#"{{"api":"{mock}"}}"#)),
        other => panic!("no kind for {other}"),
    }
}

fn plugins(specs: Vec<(String, String)>, mut jobs: mpsc::Receiver<Job>) {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async move {
        let mut loaded: HashMap<String, Rc<Loaded>> = HashMap::new();
        for (name, bundle) in specs {
            let (function, settings) = kind_of(&bundle);
            let typed = vec![host_of(settings.split("\"api\":\"").nth(1).unwrap_or("").split('"').next().unwrap_or(""))];
            let bytes = std::fs::read(&bundle).unwrap();
            match instance_limited(bytes, vec![], typed, INSTANCE_MEMORY).await {
                Ok((instance, deadline)) => {
                    let one = Loaded { instance, deadline, function, settings, turn: tokio::sync::Mutex::new(()) };
                    loaded.insert(name, Rc::new(one));
                }
                Err(e) => eprintln!("{name}: {}", first_line(&e)),
            }
        }
        println!("plugins: {} instances loaded", loaded.len());
        let installing = Rc::new(tokio::sync::Mutex::new(()));
        while let Some(job) = jobs.recv().await {
            match job {
                Job::Call { instance, reply } => {
                    let Some(one) = loaded.get(&instance).cloned() else {
                        let _ = reply.send(Err(format!("no instance {instance}")));
                        continue;
                    };
                    tokio::task::spawn_local(async move {
                        let _turn = one.turn.lock().await;
                        let params = format!("{{{RANGE}}}");
                        let c = Call { function: one.function, params, settings: one.settings.clone(), id: "hub", rows: 10, stop_after: None };
                        let (outcome, _) = call(&one.instance, &one.deadline, c).await;
                        settle(&one.instance).await;
                        let _ = reply.send(match outcome {
                            Outcome::Done(result) => result,
                            Outcome::Stopped => Err("stopped".into()),
                        });
                    });
                }
                Job::Install { bundle, limit, reply } => {
                    let installing = installing.clone();
                    tokio::task::spawn_local(async move {
                        // one install at a time: what it may cost is counted once
                        let _one = installing.lock().await;
                        let (found, _) = discover_limited(std::fs::read(&bundle).unwrap_or_default(), limit).await;
                        let _ = reply.send(match found {
                            Ok(_) => "declared".into(),
                            Err(e) => format!("refused: {}", first_line(&e)),
                        });
                    });
                }
                Job::Held { reply } => {
                    let mut total = 0;
                    for one in loaded.values() {
                        total += one.instance.runtime.memory_usage().await.malloc_size;
                    }
                    let _ = reply.send(format!("{} instances hold {:.1} MiB of JavaScript", loaded.len(), total as f64 / 1048576.0));
                }
            }
        }
    });
}

struct App {
    db: tinystore::aio::sql::Database,
    sessions: tinystore::aio::kv::Bucket<String>,
    events: tinystore::aio::records::Events,
    stats: tinystore::aio::metrics::Metrics,
    jobs: mpsc::Sender<Job>,
}

fn status() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let pick = |key: &str| {
        let kb = status.lines().find(|l| l.starts_with(key)).map(|l| l[key.len()..].trim().trim_end_matches(" kB").trim().parse::<f64>().unwrap_or(0.0));
        kb.unwrap_or(0.0) / 1024.0
    };
    format!(
        "VmRSS {:.1} MiB  RssAnon {:.1}  RssFile {:.1}  VmHWM {:.1}  threads {}",
        pick("VmRSS:"),
        pick("RssAnon:"),
        pick("RssFile:"),
        pick("VmHWM:"),
        pick("Threads:") * 1024.0
    )
}

pub async fn serve() {
    let dir = std::path::PathBuf::from("/tmp/hub-data");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default()).unwrap();
    let log = store.logger("hub").layer().unwrap();
    tracing_subscriber::registry().with(log).init();
    let db = store.database("meta").open().unwrap();
    db.exec("create table if not exists dashboards(id integer primary key, name text not null, spec text not null)").await.unwrap();
    for i in 0..20 {
        db.exec(tinystore::sql!("insert into dashboards(id, name, spec) values (?, ?, ?)", i, format!("dashboard {i}"), "{}")).await.unwrap();
    }
    let sessions = db.bucket::<String>("sessions").idle(Duration::from_hours(24 * 30)).open().unwrap();
    sessions.set("s-42", &"owner".to_string()).await.unwrap();
    let events = store.events("audit").keep(Duration::from_hours(24 * 365)).open().unwrap();
    let stats = store.metrics().unwrap();
    let _collect = store.queue::<String>("collect").open().unwrap();

    let specs: Vec<(String, String)> = std::env::var("INSTANCES")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.split_once('='))
        .map(|(name, bundle)| (name.to_string(), bundle.to_string()))
        .collect();
    let (jobs, inbox) = mpsc::channel(256);
    std::thread::Builder::new().name("plugins".into()).spawn(move || plugins(specs, inbox)).unwrap();

    let app = Arc::new(App { db, sessions, events, stats, jobs });
    let router = Router::new()
        .route("/rss", get(|| async { status() }))
        .route("/js", get(held))
        .route("/seed", get(seed))
        .route("/work", get(work))
        .route("/panel", get(panel))
        .route("/plugin/{instance}", get(plugin))
        .route("/install/{name}", get(install))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening");
    axum::serve(listener, router).await.unwrap();
}

async fn held(State(app): State<Arc<App>>) -> String {
    let (reply, answer) = oneshot::channel();
    let _ = app.jobs.send(Job::Held { reply }).await;
    answer.await.unwrap_or_default()
}

async fn plugin(State(app): State<Arc<App>>, Path(instance): Path<String>) -> (StatusCode, String) {
    let (reply, answer) = oneshot::channel();
    let _ = app.jobs.send(Job::Call { instance, reply }).await;
    match answer.await {
        Ok(Ok(rows)) => (StatusCode::OK, rows),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, first_line(&e)),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "the plugin thread is gone".into()),
    }
}

async fn install(State(app): State<Arc<App>>, Path(name): Path<String>) -> String {
    let limit = std::env::var("INSTALL_LIMIT_MIB").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(24) << 20;
    let (reply, answer) = oneshot::channel();
    // a test module from install/, or a real plugin's bundle by its name
    let test = format!("/p/install/{name}.js");
    let bundle = if std::path::Path::new(&test).exists() { test } else { format!("/p/{name}.js") };
    let _ = app.jobs.send(Job::Install { bundle, limit, reply }).await;
    format!("{}  then {}", answer.await.unwrap_or_default(), status())
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
    app.events.append("dashboard.viewed", &serde_json::json!({ "dashboard": 7, "user": session })).await.unwrap();
    tracing::info!(rows = rows.len(), "work");
    format!("{}", rows.len())
}

// one panel: average load by host, by the minute, over the last hour
async fn panel(State(app): State<Arc<App>>) -> String {
    let answer =
        app.stats.query().name("node_load").since(Duration::from_hours(1)).every(Duration::from_mins(1)).by(&["host"]).aggregate(Op::Avg).await.unwrap();
    format!("{}", answer.len())
}
