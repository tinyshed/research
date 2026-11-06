//! The agent's side: it dials the hub, says who it is, and runs each call through dashbin's own plugin; a call the
//! hub cancels, or every call when the hub is lost, is dropped, and the plugin stops its statement on its server.
//!
//! Settings from the environment:
//!
//! | Variable         | Means                                                                     |
//! |------------------|---------------------------------------------------------------------------|
//! | `PROBE_HUB`      | `wss://hub.probe/api/v1/agent`, or `ws://` to a hub on loopback          |
//! | `PROBE_AGENT`    | the name it says it is                                                    |
//! | `PROBE_TOKEN`    | its token                                                                 |
//! | `PROBE_CA`       | the PEM file of the CA that signed the proxy's certificate               |
//! | `PROBE_PLUGIN`   | `postgres` or `mysql`, the plugin of its one instance, `orders`          |
//! | `PROBE_SETTINGS` | that instance's settings as JSON                                          |
//! | `PROBE_CALLS`    | its calls at once, 8 by default                                           |

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher as _;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use dashbin::frame_json::FrameJson;
use dashbin::plugins::{self, Call, Limits, Params, Plugins, Values, Whose};
use futures_util::{SinkExt as _, StreamExt as _};
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use serde_json::{Value, json};
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{Message as Frame, http};

use crate::wire::{self, Announced, Message, VERSION, log};

const SILENCE: Duration = Duration::from_secs(45);
/// The agent's own ceilings: the hub may ask for less.
const ROWS: usize = 10_000;
const TIME: Duration = Duration::from_secs(30);
const BYTES: usize = 16 << 20;
/// How long past a call's time the agent drops it itself, for a plugin whose bound did not end it.
const GRACE: Duration = Duration::from_secs(1);

struct Config {
    hub: String,
    agent: String,
    token: String,
    ca: Option<String>,
    plugin: String,
    settings: Values,
    calls: usize,
}

impl Config {
    fn from_env() -> Config {
        let var = |name: &str| std::env::var(name).unwrap_or_default();
        let settings = serde_json::from_str::<Value>(&var("PROBE_SETTINGS")).ok().and_then(|value| value.as_object().cloned());
        Config {
            hub: var("PROBE_HUB"),
            agent: var("PROBE_AGENT"),
            token: var("PROBE_TOKEN"),
            ca: std::env::var("PROBE_CA").ok(),
            plugin: var("PROBE_PLUGIN"),
            settings: settings.unwrap_or_default(),
            calls: var("PROBE_CALLS").parse().unwrap_or(8),
        }
    }
}

pub async fn run() {
    let config = Arc::new(Config::from_env());
    let plugins = Arc::new(Plugins::built_in());
    let mut wait = Duration::from_secs(1);
    loop {
        let started = Instant::now();
        match dial(&config).await {
            Ok(socket) => {
                log("agent.connected", json!({ "agent": config.agent }));
                let why = session(&config, &plugins, socket).await;
                log("agent.disconnected", json!({ "agent": config.agent, "why": why, "lasted": started.elapsed().as_secs_f64() }));
            }
            Err(why) => log("agent.dial_failed", json!({ "agent": config.agent, "why": why })),
        }
        if started.elapsed() >= Duration::from_secs(60) {
            wait = Duration::from_secs(1);
        }
        // a fifth of the wait as jitter, so that agents that saw one outage do not dial in one volley
        let jitter = RandomState::new().hash_one(Instant::now()) % (wait.as_millis() as u64 / 5 + 1);
        tokio::time::sleep(wait + Duration::from_millis(jitter)).await;
        wait = (wait * 2).min(Duration::from_secs(30));
    }
}

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn dial(config: &Config) -> Result<Socket, String> {
    let mut request = config.hub.as_str().into_client_request().map_err(|error| error.to_string())?;
    let bearer = http::HeaderValue::from_str(&format!("Bearer {}", config.token)).map_err(|error| error.to_string())?;
    request.headers_mut().insert(http::header::AUTHORIZATION, bearer);
    let connector = match &config.ca {
        Some(ca) if config.hub.starts_with("wss://") => Some(connector(ca)?),
        _ => Some(tokio_tungstenite::Connector::Plain),
    };
    let limits = WebSocketConfig::default().max_message_size(Some(BYTES + 4096)).max_frame_size(Some(BYTES + 4096));
    let dialled = tokio_tungstenite::connect_async_tls_with_config(request, Some(limits), false, connector);
    let (socket, _) = tokio::time::timeout(Duration::from_secs(10), dialled)
        .await
        .map_err(|_| "the hub did not answer in 10 s".to_owned())?
        .map_err(|error| error.to_string())?;
    Ok(socket)
}

/// rustls on ring, trusting the CA in the PEM file `ca` alone.
fn connector(ca: &str) -> Result<tokio_tungstenite::Connector, String> {
    let mut roots = rustls::RootCertStore::empty();
    let certificates = CertificateDer::pem_file_iter(ca).map_err(|error| format!("{ca}: {error}"))?;
    for certificate in certificates {
        roots.add(certificate.map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    }
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|error| error.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(tokio_tungstenite::Connector::Rustls(Arc::new(config)))
}

type Running = Arc<Mutex<HashMap<u64, oneshot::Sender<()>>>>;

async fn session(config: &Arc<Config>, plugins: &Arc<Plugins>, socket: Socket) -> &'static str {
    let (mut sink, mut stream) = socket.split();
    let instances = vec![Announced { id: "orders".to_owned(), plugin: config.plugin.clone() }];
    let hello = Message::Hello { agent: config.agent.clone(), versions: vec![VERSION], instances };
    if sink.send(Frame::text(wire::encode(&hello))).await.is_err() {
        return "broken";
    }
    match tokio::time::timeout(Duration::from_secs(10), stream.next()).await {
        Ok(Some(Ok(Frame::Text(text)))) if matches!(wire::decode(text.as_str()), Some(Message::Welcome { .. })) => {}
        Ok(Some(Ok(Frame::Close(Some(close))))) => {
            log("agent.refused", json!({ "agent": config.agent, "why": close.reason.as_str() }));
            return "refused";
        }
        _ => return "no welcome",
    }
    let (outgoing, mut queue) = mpsc::channel::<Message>(config.calls * 2);
    let writer = tokio::spawn(async move {
        while let Some(message) = queue.recv().await {
            if sink.send(Frame::text(wire::encode(&message))).await.is_err() {
                break;
            }
        }
    });
    let running: Running = Arc::default();
    let calls = Arc::new(Semaphore::new(config.calls));
    let why = loop {
        let next = match tokio::time::timeout(SILENCE, stream.next()).await {
            Err(_) => break "silent",
            Ok(next) => next,
        };
        match next {
            None => break "closed",
            Some(Err(_)) => break "broken",
            Some(Ok(Frame::Close(_))) => break "closed",
            Some(Ok(Frame::Text(text))) => match wire::decode(text.as_str()) {
                Some(Message::Call { id, function, args, rows, millis, .. }) => {
                    let (stop, stopped) = oneshot::channel();
                    running.lock().unwrap_or_else(PoisonError::into_inner).insert(id, stop);
                    let read = Read { id, function, args, rows, millis };
                    let (config, plugins, running) = (config.clone(), plugins.clone(), running.clone());
                    let (outgoing, calls) = (outgoing.clone(), calls.clone());
                    tokio::spawn(async move { call(read, config, plugins, running, outgoing, calls, stopped).await });
                }
                Some(Message::Cancel { id }) => {
                    let stop = running.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
                    log("call.cancel", json!({ "id": id, "running": stop.is_some() }));
                    if let Some(stop) = stop {
                        let _ = stop.send(());
                    }
                }
                _ => {}
            },
            // a ping is answered by the library itself
            Some(Ok(_)) => {}
        }
    };
    // every call still running stops where it runs: its stop's sender goes
    let dropped = running.lock().unwrap_or_else(PoisonError::into_inner).drain().count();
    log("agent.calls_dropped", json!({ "agent": config.agent, "calls": dropped }));
    writer.abort();
    why
}

struct Read {
    id: u64,
    function: String,
    args: Value,
    rows: usize,
    millis: u64,
}

async fn call(
    read: Read,
    config: Arc<Config>,
    plugins: Arc<Plugins>,
    running: Running,
    outgoing: mpsc::Sender<Message>,
    calls: Arc<Semaphore>,
    stopped: oneshot::Receiver<()>,
) {
    let started = Instant::now();
    let Ok(_permit) = calls.acquire_owned().await else { return };
    let limits = Limits { rows: read.rows.min(ROWS), time: Duration::from_millis(read.millis).min(TIME) };
    let outcome = answer(&read, &config, &plugins, limits, stopped).await;
    running.lock().unwrap_or_else(PoisonError::into_inner).remove(&read.id);
    let (message, said) = match outcome {
        Ok(frame) => (Message::Answer { id: read.id, frame }, "answered".to_owned()),
        Err((kind, message)) => (Message::Failed { id: read.id, kind: kind.clone(), message }, kind),
    };
    log("call.done", json!({ "id": read.id, "outcome": said, "ms": started.elapsed().as_millis() as u64 }));
    let _ = outgoing.send(message).await;
}

async fn answer(
    read: &Read,
    config: &Config,
    plugins: &Plugins,
    limits: Limits,
    stopped: oneshot::Receiver<()>,
) -> Result<Value, (String, String)> {
    let failed = |error: dashbin::Error| (format!("{:?}", error.kind()).to_lowercase(), error.to_string());
    let plugin = plugins.named(&config.plugin).map_err(failed)?;
    let settings = plugins::check(Whose::Settings(&config.plugin), plugin.settings(), &config.settings).map_err(failed)?;
    let args = read.args.as_object().cloned().unwrap_or_default();
    let params = Params::new();
    let stop = Box::pin(async move {
        let _ = stopped.await;
    });
    let call = Call { read: &read.function, args, params: &params, settings: &settings, limits, stop };
    let frame = match tokio::time::timeout(limits.time + GRACE, plugins::read(plugin, call)).await {
        Ok(frame) => frame.map_err(failed)?,
        Err(_) => return Err(("timeout".to_owned(), "the agent stopped the call at its time".to_owned())),
    };
    let frame = serde_json::to_value(FrameJson::from(&frame)).map_err(|error| ("unavailable".to_owned(), error.to_string()))?;
    let bytes = serde_json::to_string(&frame).map_or(0, |text| text.len());
    if bytes > BYTES {
        return Err(("limit".to_owned(), format!("the answer is {bytes} bytes, past the agent's {BYTES}")));
    }
    Ok(frame)
}
