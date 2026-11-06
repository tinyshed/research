//! The hub's side: the agents' door at `/api/v1/agent`, a read at `/api/v1/run` routed to `agent.instance`, and
//! `/probe/agents` for the driver to see who is connected.
//!
//! Settings from the environment:
//!
//! | Variable          | Means                                                                       |
//! |-------------------|-----------------------------------------------------------------------------|
//! | `PROBE_LISTEN`    | the address, `0.0.0.0:7317` in a container                                  |
//! | `PROBE_AGENTS`    | the agents' names, `site1,site2,…`; each one's token is `token-<name>`     |
//! | `PROBE_LIVENESS`  | `bytes` (alive while bytes come) or `messages` (alive when a message ends) |
//! | `PROBE_MAX_BYTES` | the largest message either side reads, 16 MiB by default                   |

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::{SinkExt as _, StreamExt as _};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc, oneshot};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Role, WebSocketConfig};
use tokio_tungstenite::tungstenite::{Bytes, Message as Frame};

use crate::watched::{Liveness, Watched};
use crate::wire::{self, Message, VERSION, log};

const PING: Duration = Duration::from_secs(15);
const SILENCE: Duration = Duration::from_secs(45);

type Answer = Result<Value, (String, String)>;

struct Hub {
    tokens: HashMap<String, String>,
    agents: Mutex<HashMap<String, Link>>,
    next_id: AtomicU64,
    generation: AtomicU64,
    by_bytes: bool,
    max_bytes: usize,
}

/// One connected agent, as a read reaches it.
#[derive(Clone)]
struct Link {
    generation: u64,
    outgoing: mpsc::Sender<Message>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Answer>>>>,
    replaced: Arc<Notify>,
}

pub async fn run() {
    let names = std::env::var("PROBE_AGENTS").unwrap_or_default();
    let tokens = names.split(',').filter(|name| !name.is_empty()).map(|name| (format!("token-{name}"), name.to_owned()));
    let by_bytes = std::env::var("PROBE_LIVENESS").map_or(true, |mode| mode != "messages");
    let max_bytes = std::env::var("PROBE_MAX_BYTES").ok().and_then(|bytes| bytes.parse().ok()).unwrap_or(16 << 20);
    let hub = Arc::new(Hub {
        tokens: tokens.collect(),
        agents: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
        generation: AtomicU64::new(1),
        by_bytes,
        max_bytes,
    });
    let router = axum::Router::new()
        .route("/api/v1/agent", get(door))
        .route("/api/v1/run", post(read))
        .route("/probe/agents", get(connected))
        .with_state(hub);
    let address = std::env::var("PROBE_LISTEN").unwrap_or_else(|_| "127.0.0.1:7317".to_owned());
    let Ok(listener) = tokio::net::TcpListener::bind(&address).await else {
        log("hub.failed", json!({ "why": format!("cannot listen on {address}") }));
        return;
    };
    log("hub.listening", json!({ "address": address, "liveness": if by_bytes { "bytes" } else { "messages" } }));
    let _ = axum::serve(listener, router).await;
}

async fn connected(State(hub): State<Arc<Hub>>) -> Json<Value> {
    let mut names: Vec<String> = hub.agents.lock().unwrap_or_else(PoisonError::into_inner).keys().cloned().collect();
    names.sort();
    Json(json!({ "agents": names, "liveness": if hub.by_bytes { "bytes" } else { "messages" } }))
}

/// The agents' door: a token of an agent's opens it, and the upgrade is done by hand so that the socket under the
/// WebSocket can be watched for bytes.
async fn door(State(hub): State<Arc<Hub>>, mut request: Request) -> Response {
    let headers = request.headers();
    let token = headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok());
    let agent = token.and_then(|token| token.strip_prefix("Bearer ")).and_then(|token| hub.tokens.get(token)).cloned();
    let from = headers.get("x-forwarded-for").and_then(|value| value.to_str().ok()).unwrap_or("").to_owned();
    let Some(agent) = agent else {
        log("door.refused", json!({ "why": "no agent has that token", "from": from }));
        return (StatusCode::UNAUTHORIZED, "no agent has that token").into_response();
    };
    let Some(key) = headers.get(header::SEC_WEBSOCKET_KEY).map(|key| derive_accept_key(key.as_bytes())) else {
        return (StatusCode::BAD_REQUEST, "not a websocket").into_response();
    };
    let upgrade = hyper::upgrade::on(&mut request);
    tokio::spawn(async move {
        let Ok(upgraded) = upgrade.await else { return };
        let liveness = Liveness::new();
        let io = Watched::new(TokioIo::new(upgraded), liveness.clone());
        let config = WebSocketConfig::default().max_message_size(Some(hub.max_bytes)).max_frame_size(Some(hub.max_bytes));
        let socket = WebSocketStream::from_raw_socket(io, Role::Server, Some(config)).await;
        session(hub, agent, from, socket, liveness).await;
    });
    let response = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_ACCEPT, key)
        .body(Body::empty());
    response.unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn session<S>(hub: Arc<Hub>, agent: String, from: String, socket: WebSocketStream<S>, liveness: Liveness)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let opened = Instant::now();
    let (mut sink, mut stream) = socket.split();
    let hello = match tokio::time::timeout(Duration::from_secs(10), stream.next()).await {
        Ok(Some(Ok(Frame::Text(text)))) => wire::decode(text.as_str()),
        _ => None,
    };
    let Some(Message::Hello { agent: said, versions, instances }) = hello else {
        log("door.refused", json!({ "agent": agent, "why": "no greeting" }));
        return;
    };
    if said != agent {
        let why = format!("this token is {agent}'s, and the agent says it is {said}");
        log("door.refused", json!({ "agent": agent, "said": said, "why": why }));
        let close = CloseFrame { code: CloseCode::Policy, reason: why.into() };
        let _ = sink.send(Frame::Close(Some(close))).await;
        return;
    }
    if !versions.contains(&VERSION) {
        log("door.refused", json!({ "agent": agent, "why": "no protocol in common" }));
        return;
    }
    let approved: Vec<String> = instances.iter().map(|instance| instance.id.clone()).collect();
    let welcome = Message::Welcome { version: VERSION, approved };
    if sink.send(Frame::text(wire::encode(&welcome))).await.is_err() {
        return;
    }
    let (outgoing, mut queue) = mpsc::channel::<Message>(64);
    let link = Link {
        generation: hub.generation.fetch_add(1, Ordering::Relaxed),
        outgoing,
        pending: Arc::default(),
        replaced: Arc::default(),
    };
    let older = hub.agents.lock().unwrap_or_else(PoisonError::into_inner).insert(agent.clone(), link.clone());
    if let Some(older) = older {
        log("agent.twice", json!({ "agent": agent, "from": from }));
        older.replaced.notify_one();
    }
    log("agent.connected", json!({ "agent": agent, "from": from, "generation": link.generation }));
    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval(PING);
        loop {
            let sent = tokio::select! {
                message = queue.recv() => match message {
                    Some(message) => sink.send(Frame::text(wire::encode(&message))).await,
                    None => break,
                },
                _ = ping.tick() => sink.send(Frame::Ping(Bytes::new())).await,
            };
            if sent.is_err() {
                break;
            }
        }
    });
    let mut last_message = Instant::now();
    let why = loop {
        let silent = if hub.by_bytes { liveness.silent_for() } else { last_message.elapsed() };
        if silent > SILENCE {
            break "silent";
        }
        let next = tokio::select! {
            next = stream.next() => next,
            () = tokio::time::sleep(Duration::from_secs(1)) => continue,
            () = link.replaced.notified() => break "replaced",
        };
        match next {
            None => break "closed",
            Some(Err(_)) => break "broken",
            Some(Ok(Frame::Text(text))) => {
                last_message = Instant::now();
                answered(&link, wire::decode(text.as_str()));
            }
            Some(Ok(Frame::Close(_))) => break "closed",
            Some(Ok(_)) => last_message = Instant::now(),
        }
    };
    writer.abort();
    let mut agents = hub.agents.lock().unwrap_or_else(PoisonError::into_inner);
    if agents.get(&agent).is_some_and(|current| current.generation == link.generation) {
        agents.remove(&agent);
    }
    drop(agents);
    let waiting: Vec<oneshot::Sender<Answer>> =
        link.pending.lock().unwrap_or_else(PoisonError::into_inner).drain().map(|(_, waiting)| waiting).collect();
    let failed = waiting.len();
    for waiting in waiting {
        let _ = waiting.send(Err(("unavailable".to_owned(), "the agent went away".to_owned())));
    }
    let lasted = opened.elapsed().as_secs_f64();
    log("agent.disconnected", json!({ "agent": agent, "why": why, "lasted": lasted, "failed": failed }));
}

/// Hands an answer to the call it is for; one for a call nobody waits on is dropped.
fn answered(link: &Link, message: Option<Message>) {
    let (id, answer) = match message {
        Some(Message::Answer { id, frame }) => (id, Ok(frame)),
        Some(Message::Failed { id, kind, message }) => (id, Err((kind, message))),
        _ => return,
    };
    let waiting = link.pending.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
    if let Some(waiting) = waiting {
        let _ = waiting.send(answer);
    }
}

#[derive(Deserialize)]
struct Asked {
    on: String,
    sql: String,
    rows: Option<usize>,
    millis: Option<u64>,
    /// How long the hub waits for the answer, its time and ten seconds by default: a big answer over a slow link
    /// may take longer to cross than to read.
    wait: Option<u64>,
}

/// Sends `$cancel` for a call whose caller went away before its answer.
struct CancelOnDrop {
    id: u64,
    link: Option<Link>,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        let Some(link) = self.link.take() else { return };
        link.pending.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.id);
        let sent = link.outgoing.try_send(Message::Cancel { id: self.id }).is_ok();
        log("call.cancelled", json!({ "id": self.id, "sent": sent }));
    }
}

async fn read(State(hub): State<Arc<Hub>>, Json(asked): Json<Asked>) -> Response {
    let Some((agent, instance)) = asked.on.split_once('.') else {
        return failure(StatusCode::BAD_REQUEST, "invalid", "on: agent.instance");
    };
    let link = hub.agents.lock().unwrap_or_else(PoisonError::into_inner).get(agent).cloned();
    let Some(link) = link else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "the agent is away");
    };
    let id = hub.next_id.fetch_add(1, Ordering::Relaxed);
    let (answering, answer) = oneshot::channel();
    link.pending.lock().unwrap_or_else(PoisonError::into_inner).insert(id, answering);
    let mut guard = CancelOnDrop { id, link: Some(link.clone()) };
    let millis = asked.millis.unwrap_or(30_000);
    let args = json!({ "sql": asked.sql });
    let call = Message::Call { id, instance: instance.to_owned(), function: "sql".to_owned(), args, rows: asked.rows.unwrap_or(10_000), millis };
    if link.outgoing.send(call).await.is_err() {
        guard.link = None;
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "the agent went away");
    }
    let backstop = asked.wait.map_or(Duration::from_millis(millis) + Duration::from_secs(10), Duration::from_secs);
    match tokio::time::timeout(backstop, answer).await {
        Ok(Ok(Ok(frame))) => {
            guard.link = None;
            Json(frame).into_response()
        }
        Ok(Ok(Err((kind, message)))) => {
            guard.link = None;
            let status = match kind.as_str() {
                "invalid" => StatusCode::BAD_REQUEST,
                "forbidden" => StatusCode::FORBIDDEN,
                "timeout" => StatusCode::GATEWAY_TIMEOUT,
                "limit" => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            };
            failure(status, &kind, &message)
        }
        Ok(Err(_)) => {
            guard.link = None;
            failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "the agent went away")
        }
        // the guard sends the cancel as it drops
        Err(_) => failure(StatusCode::GATEWAY_TIMEOUT, "timeout", "the agent did not answer in time"),
    }
}

fn failure(status: StatusCode, kind: &str, message: &str) -> Response {
    (status, Json(json!({ "kind": kind, "message": message }))).into_response()
}
