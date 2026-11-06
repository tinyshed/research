//! The scenarios, run from the host against compose.yaml's hub, proxy, ten agents and three databases: what a
//! person's read does through the chain, and what each stop and each failure leaves running where it read.
//!
//! `agent-probe drive [scenario]`, every scenario in order without one. A line of each result goes to stdout.

use std::time::{Duration, Instant};

use dashbin::Frame;
use dashbin::frame::Data;
use dashbin::plugins::{Limits, Params, mysql, postgres};
use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::{Message as Frame_, http};

use crate::wire::{self, Announced, Message, VERSION};

const HUB: &str = "127.0.0.1:27317";
const PROJECT: &str = "agent-probe";
const CRLF: &str = "\r\n";
const LIMITS: Limits = Limits { rows: 10_000, time: Duration::from_secs(30) };

fn container(service: &str) -> String {
    format!("{PROJECT}-{service}-1")
}

pub async fn run(only: Option<String>) {
    let scenarios: [(&str, fn() -> futures_util::future::BoxFuture<'static, ()>); 9] = [
        ("connected", || Box::pin(connected())),
        ("round_trip", || Box::pin(round_trip())),
        ("wrong_token", || Box::pin(wrong_token())),
        ("cancel", || Box::pin(cancel())),
        ("lost_hub", || Box::pin(lost_hub())),
        ("overload", || Box::pin(overload())),
        ("memory", || Box::pin(memory())),
        ("hub_restart", || Box::pin(hub_restart())),
        ("proxy_reload", || Box::pin(proxy_reload())),
    ];
    for (name, scenario) in scenarios {
        if only.as_deref().is_none_or(|only| only == name) {
            println!("\n## {name}\n");
            scenario().await;
        }
    }
}

// ---- the hub over HTTP, as a command line reaches it

/// One HTTP/1.1 request to the hub and its answer, the connection closed after it.
async fn request(method: &str, path: &str, body: &str) -> (u16, String) {
    let Ok(mut stream) = TcpStream::connect(HUB).await else { return (0, "no hub".to_owned()) };
    let _ = stream.write_all(head(method, path, body).as_bytes()).await;
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer).await;
    let answer = String::from_utf8_lossy(&answer).into_owned();
    let status = answer.split(' ').nth(1).and_then(|status| status.parse().ok()).unwrap_or(0);
    let body = answer.split_once(&format!("{CRLF}{CRLF}")).map(|(_, body)| body.to_owned()).unwrap_or_default();
    (status, body)
}

fn head(method: &str, path: &str, body: &str) -> String {
    let lines = [
        format!("{method} {path} HTTP/1.1"),
        "Host: hub".to_owned(),
        "Content-Type: application/json".to_owned(),
        format!("Content-Length: {}", body.len()),
        "Connection: close".to_owned(),
    ];
    format!("{}{CRLF}{CRLF}{body}", lines.join(CRLF))
}

/// A read through the hub: its status, how long it took, and the frame's rows or the error's words.
async fn read_through(on: &str, sql: &str, wait: Option<u64>) -> (u16, Duration, String) {
    let started = Instant::now();
    let mut asked = json!({ "on": on, "sql": sql });
    if let Some(wait) = wait {
        asked["wait"] = json!(wait);
    }
    let (status, body) = request("POST", "/api/v1/run", &asked.to_string()).await;
    let said = serde_json::from_str::<Value>(&body).ok();
    let said = match said {
        Some(frame) if status == 200 => {
            let rows = frame["columns"][0]["values"].as_array().map_or(0, Vec::len);
            format!("{rows} rows")
        }
        Some(failure) => format!("{}: {}", failure["kind"], failure["message"]),
        None => body.chars().take(120).collect(),
    };
    (status, started.elapsed(), said)
}

async fn agents() -> (Vec<String>, String) {
    let (_, body) = request("GET", "/probe/agents", "").await;
    let said: Value = serde_json::from_str(&body).unwrap_or_default();
    let names = said["agents"].as_array().map(|names| names.iter().filter_map(|name| name.as_str().map(str::to_owned)).collect());
    (names.unwrap_or_default(), said["liveness"].as_str().unwrap_or("").to_owned())
}

async fn wait_for_agents(count: usize, within: Duration) -> Option<Duration> {
    let started = Instant::now();
    while started.elapsed() < within {
        if agents().await.0.len() == count {
            return Some(started.elapsed());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

// ---- the databases, read from the host as their admin

fn pg() -> postgres::Settings {
    postgres::Settings {
        host: "127.0.0.1".to_owned(),
        port: 25432,
        database: "postgres".to_owned(),
        user: "postgres".to_owned(),
        password: None,
    }
}

fn my(port: u16) -> mysql::Settings {
    mysql::Settings {
        host: "127.0.0.1".to_owned(),
        port,
        database: "mysql".to_owned(),
        user: "root".to_owned(),
        password: Some("probe".to_owned()),
    }
}

fn number(frame: &Frame) -> i64 {
    match frame.columns.first().map(|column| &column.data) {
        Some(Data::Int(values)) => values.first().copied().unwrap_or(-1),
        _ => -1,
    }
}

#[derive(Clone, Copy)]
enum Database {
    Postgres,
    MySql,
    MariaDb,
}

impl Database {
    fn name(self) -> &'static str {
        match self {
            Database::Postgres => "postgres",
            Database::MySql => "mysql",
            Database::MariaDb => "mariadb",
        }
    }

    /// The statements running there whose text holds `marker`, the asking one aside.
    async fn running(self, marker: &str) -> i64 {
        let frame = match self {
            Database::Postgres => {
                let sql = format!(
                    "select count(*)::int8 as n from pg_stat_activity where query like '%{marker}%' and pid <> pg_backend_pid()"
                );
                postgres::read(&pg(), &sql, &Params::new(), LIMITS, std::future::pending()).await
            }
            Database::MySql | Database::MariaDb => {
                let port = if matches!(self, Database::MySql) { 23306 } else { 23307 };
                let sql = format!(
                    "SELECT COUNT(*) AS n FROM information_schema.PROCESSLIST WHERE INFO LIKE '%{marker}%' AND ID <> CONNECTION_ID()"
                );
                mysql::read(&my(port), &sql, &Params::new(), LIMITS, std::future::pending()).await
            }
        };
        frame.map_or(-1, |frame| number(&frame))
    }

    /// Waits until nothing holding `marker` runs, and says how long that took; none past `within`.
    async fn ended(self, marker: &str, within: Duration) -> Option<Duration> {
        let started = Instant::now();
        while started.elapsed() < within {
            if self.running(marker).await == 0 {
                return Some(started.elapsed());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        None
    }

    fn sleep(self, seconds: u32, marker: &str) -> String {
        match self {
            Database::Postgres => format!("select pg_sleep({seconds}) as {marker}"),
            Database::MySql | Database::MariaDb => format!("SELECT SLEEP({seconds}) AS {marker}"),
        }
    }
}

// ---- docker

async fn docker(args: &[&str]) -> String {
    let output = tokio::process::Command::new("docker").args(args).output().await;
    output.map_or_else(
        |error| error.to_string(),
        |output| format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)),
    )
}

/// The JSON lines a service logged since `since`, seconds since 1970, of the event named.
async fn logged(service: &str, since: f64, event: &str) -> Vec<Value> {
    let since = format!("{since:.3}");
    let lines = docker(&["logs", "--since", &since, &container(service)]).await;
    lines
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| line["event"] == event)
        .collect()
}

fn ms(duration: Duration) -> String {
    format!("{} ms", duration.as_millis())
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort();
    values.get(values.len() / 2).copied().unwrap_or_default()
}

// ---- the scenarios

async fn connected() {
    match wait_for_agents(10, Duration::from_secs(120)).await {
        Some(after) => println!("- ten agents connected through the proxy ({} after the first look)", ms(after)),
        None => println!("- only {:?} connected after 120 s", agents().await.0),
    }
}

async fn round_trip() {
    let mut all = Vec::new();
    for site in 1..=10 {
        let mut times = Vec::new();
        let mut said = String::new();
        for _ in 0..30 {
            let (status, took, words) = read_through(&format!("site{site}.orders"), "select 1 as one", None).await;
            said = format!("{status} {words}");
            times.push(took);
        }
        all.extend(times.iter().copied());
        println!("- site{site}: median {} of 30, the last {said}", ms(median(times)));
    }
    println!("- every site: median {} of 300", ms(median(all)));
    let mut direct = Vec::new();
    for _ in 0..30 {
        let started = Instant::now();
        let _ = postgres::read(&pg(), "select 1 as one", &Params::new(), LIMITS, std::future::pending()).await;
        direct.push(started.elapsed());
    }
    println!("- the same read from the host straight to postgres: median {} of 30", ms(median(direct)));
}

async fn wrong_token() {
    for (token, says) in [("token-site1", "site2"), ("token-nobody", "site1")] {
        let Ok(mut request) = format!("ws://{HUB}/api/v1/agent").into_client_request() else { return };
        if let Ok(bearer) = http::HeaderValue::from_str(&format!("Bearer {token}")) {
            request.headers_mut().insert(http::header::AUTHORIZATION, bearer);
        }
        let said = match tokio_tungstenite::connect_async(request).await {
            Err(error) => format!("refused before the socket: {error}"),
            Ok((mut socket, _)) => {
                let instances = vec![Announced { id: "orders".to_owned(), plugin: "postgres".to_owned() }];
                let hello = Message::Hello { agent: says.to_owned(), versions: vec![VERSION], instances };
                let _ = socket.send(Frame_::text(wire::encode(&hello))).await;
                match socket.next().await {
                    Some(Ok(Frame_::Close(Some(close)))) => format!("closed: {}", close.reason.as_str()),
                    other => format!("answered {other:?}"),
                }
            }
        };
        println!("- {token} saying it is {says}: {said}");
    }
    let (connected, _) = agents().await;
    println!("- still connected: {} agents, site1 and site2 among them: {}", connected.len(), connected.contains(&"site1".to_owned()) && connected.contains(&"site2".to_owned()));
}

async fn cancel() {
    for (site, database) in [("site1", Database::Postgres), ("site6", Database::MySql), ("site9", Database::MariaDb)] {
        let marker = format!("probe_cancel_{site}");
        let body = json!({ "on": format!("{site}.orders"), "sql": database.sleep(30, &marker) }).to_string();
        let Ok(mut stream) = TcpStream::connect(HUB).await else { return };
        let _ = stream.write_all(head("POST", "/api/v1/run", &body).as_bytes()).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let before = database.running(&marker).await;
        drop(stream);
        let ended = database.ended(&marker, Duration::from_secs(10)).await;
        let ended = ended.map_or("still running after 10 s".to_owned(), |after| format!("ended {} after the client left", ms(after)));
        println!("- {site}, {}: {before} running while asked; {ended}", database.name());
    }
}

async fn lost_hub() {
    let marker = "probe_lost";
    let database = Database::Postgres;
    let sql = database.sleep(28, marker);
    let asked = tokio::spawn(async move { read_through("site2.orders", &sql, Some(120)).await });
    tokio::time::sleep(Duration::from_millis(800)).await;
    let cut = Instant::now();
    let since = wire::now();
    println!("- running before the cut: {}", database.running(marker).await);
    println!("- cut: {}", docker(&["network", "disconnect", &format!("{PROJECT}_wan"), &container("agent2")]).await.trim());
    let ended = database.ended(marker, Duration::from_secs(60)).await;
    println!("- the statement ended {} after the cut", ended.map_or("not within 60 s".to_owned(), ms));
    if let Ok((status, took, said)) = asked.await {
        println!("- the read answered {status} after {} in all: {said}", ms(took));
    }
    for line in logged("hub", since, "agent.disconnected").await.iter().filter(|line| line["agent"] == "site2") {
        println!("- the hub let site2 go as {} at {:.1} s after the cut", line["why"], line["at"].as_f64().unwrap_or(0.0) - since);
    }
    for line in logged("agent2", since, "agent.calls_dropped").await {
        println!("- the agent dropped {} calls at {:.1} s after the cut", line["calls"], line["at"].as_f64().unwrap_or(0.0) - since);
    }
    println!("- mended: {}", docker(&["network", "connect", &format!("{PROJECT}_wan"), &container("agent2")]).await.trim());
    let mended = Instant::now();
    let back = wait_for_agents(10, Duration::from_secs(90)).await;
    println!("- site2 back {} after the mend, {} after the cut", back.map_or("never".to_owned(), ms), ms(cut.elapsed()));
    let _ = mended;
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("- running after the reconnect, a call sent again would show: {}", database.running(marker).await);
}

/// The agent's interface on the network `wan`, from its address there.
async fn wan_interface(service: &str) -> Option<String> {
    let format = format!("{{{{(index .NetworkSettings.Networks \"{PROJECT}_wan\").IPAddress}}}}");
    let address = docker(&["inspect", "-f", &format, &container(service)]).await.trim().to_owned();
    let interfaces = docker(&["exec", &container(service), "ip", "-o", "-4", "addr", "show"]).await;
    let line = interfaces.lines().find(|line| line.contains(&format!("inet {address}/")))?;
    line.split_whitespace().nth(1).map(str::to_owned)
}

async fn overload() {
    let (_, liveness) = agents().await;
    println!("- the hub's liveness: {liveness}");
    let Some(interface) = wan_interface("agent5").await else {
        println!("- no interface of agent5 on wan");
        return;
    };
    let agent5 = container("agent5");
    println!("- agent5's link to the hub: {} at 1 Mbit/s", interface);
    let _ = docker(&["exec", &agent5, "tc", "qdisc", "add", "dev", &interface, "root", "netem", "rate", "1mbit"]).await;
    let since = wire::now();
    let big = "select n, repeat('x', 1000) as filler from generate_series(1, 10000) n";
    let asked = tokio::spawn(async move { read_through("site5.orders", big, Some(200)).await });
    let started = Instant::now();
    while !asked.is_finished() && started.elapsed() < Duration::from_secs(200) {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let (_, other, _) = read_through("site1.orders", "select 1 as one", None).await;
        let (status, same, said) = read_through("site5.orders", "select 1 as one", Some(5)).await;
        println!(
            "- at {:.0} s: site1 answers in {}; site5's small read {} in {}: {said}",
            started.elapsed().as_secs_f64(),
            ms(other),
            status,
            ms(same)
        );
    }
    if let Ok((status, took, said)) = asked.await {
        println!("- the big read answered {status} after {}: {said}", ms(took));
    }
    for line in logged("hub", since, "agent.disconnected").await {
        println!("- the hub let {} go as {} at {:.1} s", line["agent"], line["why"], line["at"].as_f64().unwrap_or(0.0) - since);
    }
    let _ = docker(&["exec", &agent5, "tc", "qdisc", "del", "dev", &interface, "root"]).await;
    let back = wait_for_agents(10, Duration::from_secs(60)).await;
    println!("- ten agents again {}", back.map_or("never".to_owned(), ms));
}

async fn status_kib(service: &str) -> (u64, u64) {
    let status = docker(&["exec", &container(service), "cat", "/proc/1/status"]).await;
    let field = |name: &str| {
        let line = status.lines().find(|line| line.starts_with(name));
        line.and_then(|line| line.split_whitespace().nth(1)).and_then(|kib| kib.parse().ok()).unwrap_or(0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

async fn memory() {
    for (service, site, sql) in [
        ("agent1", "site1", "select n, md5(n::text) as hash from generate_series(1, 10000) n".to_owned()),
        (
            "agent6",
            "site6",
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 100) \
             SELECT a.i * 100 + b.i AS n, MD5(a.i * 100 + b.i) AS hash FROM n a, n b"
                .to_owned(),
        ),
    ] {
        let (rss, peak) = status_kib(service).await;
        println!("- {service} before: {rss} KiB resident, {peak} KiB at its peak so far");
        let reads: Vec<_> = (0..4)
            .map(|_| {
                let sql = sql.clone();
                let on = format!("{site}.orders");
                tokio::spawn(async move { read_through(&on, &sql, None).await })
            })
            .collect();
        for read in reads {
            if let Ok((status, took, said)) = read.await {
                println!("  - {status} in {}: {said}", ms(took));
            }
        }
        let (rss, peak) = status_kib(service).await;
        println!("- {service} after four reads of 10 000 rows at once: {rss} KiB resident, {peak} KiB at its peak");
    }
}

async fn hub_restart() {
    let since = wire::now();
    println!("- restart: {}", docker(&["restart", "-t", "0", &container("hub")]).await.trim());
    let back = wait_for_agents(10, Duration::from_secs(120)).await;
    println!("- ten agents again {} after the restart began", back.map_or("never".to_owned(), ms));
    let mut offsets: Vec<f64> =
        logged("hub", since, "agent.connected").await.iter().filter_map(|line| line["at"].as_f64()).map(|at| at - since).collect();
    offsets.sort_by(f64::total_cmp);
    let offsets: Vec<String> = offsets.iter().map(|offset| format!("{offset:.2}")).collect();
    println!("- each agent's return, seconds after the restart began: {}", offsets.join(", "));
}

async fn proxy_reload() {
    // stream_close_delay keeps a config's upgraded streams open for a while after it is replaced
    let steps = [
        ("the same config, forced", "/etc/caddy/Caddyfile"),
        ("a changed config", "/etc/caddy/Caddyfile.changed"),
        ("a config that delays closing streams", "/etc/caddy/Caddyfile.delay"),
        ("that config changed", "/etc/caddy/Caddyfile.delay.changed"),
        ("back", "/etc/caddy/Caddyfile"),
    ];
    for (what, config) in steps {
        let since = wire::now();
        let said = docker(&["exec", &container("caddy"), "caddy", "reload", "--config", config, "--force"]).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        let gone = logged("hub", since, "agent.disconnected").await.len();
        let back = wait_for_agents(10, Duration::from_secs(90)).await;
        let said = said.lines().last().unwrap_or("").chars().take(100).collect::<String>();
        println!("- reload with {what}: {gone} agents disconnected within 5 s; ten again {}; caddy said {said:?}", back.map_or("never".to_owned(), ms));
    }
}
