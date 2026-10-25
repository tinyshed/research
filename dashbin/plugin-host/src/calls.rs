// chapter 6's plugins in this host: declarations read under limits at install, calls with their two arguments,
// and a call's end aborting its requests whether or not the plugin's SDK passed a signal
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rquickjs::{CatchResultExt, Function, Module, Object, Promise, async_with};
use tokio::io::AsyncReadExt;

use crate::{Instance, checkpoint, purge};

const INSTALL_MEMORY: usize = 24 << 20;
const INSTALL_STACK: usize = 512 << 10;
const INSTALL_TIME: Duration = Duration::from_secs(2);
const CALL_TIME: Duration = Duration::from_secs(30);
// how long a call's leftovers may still run JavaScript once it has ended
const GRACE: Duration = Duration::from_millis(500);

type Deadline = Arc<Mutex<Option<Instant>>>;

async fn guard(instance: &Instance) -> Deadline {
    let deadline: Deadline = Arc::default();
    let watched = deadline.clone();
    let handler = move || watched.lock().unwrap().is_some_and(|d| Instant::now() > d);
    instance.runtime.set_interrupt_handler(Some(Box::new(handler))).await;
    deadline
}

fn arm(deadline: &Deadline, after: Duration) {
    *deadline.lock().unwrap() = Some(Instant::now() + after);
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").chars().take(160).collect()
}

async fn load(instance: &Instance, bytes: Vec<u8>) -> Result<(), String> {
    async_with!(instance.context => |ctx| {
        let module = Module::declare(ctx.clone(), "plugin", bytes).catch(&ctx).map_err(|e| e.to_string())?;
        let (module, promise) = module.eval().catch(&ctx).map_err(|e| e.to_string())?;
        match tokio::time::timeout(INSTALL_TIME, promise.into_future::<()>()).await {
            Err(_) => return Err(format!("its top level did not finish within {INSTALL_TIME:?}")),
            Ok(done) => done.catch(&ctx).map_err(|e| e.to_string())?,
        }
        let exports = module.namespace().catch(&ctx).map_err(|e| e.to_string())?;
        ctx.globals().set("__dashbinExports", exports).map_err(|e| e.to_string())
    })
    .await
}

/// Reads what a bundle declares, in a runtime of its own: no host reachable, no call open, no settings, limits on.
async fn discover(bytes: Vec<u8>) -> Result<String, String> {
    let instance = Instance::with_net(Some((Some(vec![]), vec![]))).await.map_err(|e| e.to_string())?;
    instance.runtime.set_memory_limit(INSTALL_MEMORY).await;
    instance.runtime.set_max_stack_size(INSTALL_STACK).await;
    let deadline = guard(&instance).await;
    arm(&deadline, INSTALL_TIME);
    let mut found = load(&instance, bytes).await.map(|_| String::new());
    if found.is_ok() {
        found = async_with!(instance.context => |ctx| {
            let describe: Function = ctx.globals().get("__dashbinDescribe").map_err(|e| e.to_string())?;
            describe.call::<_, String>(()).catch(&ctx).map_err(|e| e.to_string())
        })
        .await;
    }
    instance.timers.sweep();
    instance.unload().await;
    found
}

pub async fn discover_all(paths: &[String]) {
    checkpoint("discover: before");
    for path in paths {
        let started = Instant::now();
        println!("  {path}");
        let outcome = discover(std::fs::read(path).unwrap()).await;
        let took = started.elapsed();
        match outcome {
            Ok(json) => println!("    declared in {took:.0?}: {json}"),
            Err(e) => println!("    refused in {took:.0?}: {}", first_line(&e)),
        }
        checkpoint(&format!("    after {}", path.rsplit('/').next().unwrap_or(path)));
    }
    purge();
    checkpoint("discover: after all, purged");
}

enum Stop {
    // the host ends the call: its requests are dropped, its signal is not touched
    End,
    // the host only aborts the call's signal, and ends the call later
    Signal,
}

enum Outcome {
    Done(Result<String, String>),
    Stopped,
}

struct Call<'a> {
    function: &'a str,
    params: String,
    settings: String,
    id: &'a str,
    rows: u32,
    stop_after: Option<(Duration, Stop)>,
}

/// Runs one call to its end or its stop; returns what it gave and when it ended.
async fn call(instance: &Instance, deadline: &Deadline, c: Call<'_>) -> (Outcome, Instant) {
    if let Some(net) = &instance.net {
        net.open_call();
    }
    arm(deadline, CALL_TIME);
    let signal_only = matches!(c.stop_after, Some((_, Stop::Signal)));
    let (function, params, settings, id, rows) = (c.function.to_string(), c.params, c.settings, c.id.to_string(), c.rows);
    let stop_after = c.stop_after;
    let outcome = async_with!(instance.context => |ctx| {
        let start: Result<Promise, String> = (|| {
            let call: Function = ctx.globals().get("__dashbinCall")?;
            call.call((function, params, settings, id, rows))
        })()
        .catch(&ctx)
        .map_err(|e| e.to_string());
        let promise = match start {
            Ok(promise) => promise,
            Err(e) => return Outcome::Done(Err(e)),
        };
        let wait = async {
            match &stop_after {
                Some((after, _)) => tokio::time::sleep(*after).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            result = promise.into_future::<String>() => Outcome::Done(result.catch(&ctx).map_err(|e| e.to_string())),
            _ = wait => {
                if let Some((_, Stop::Signal)) = stop_after {
                    let abort = ctx.globals().get::<_, Object>("__dashbinAbort").and_then(|c| c.get::<_, Function>("abort").map(|f| (c, f)));
                    if let Ok((controller, abort)) = abort {
                        let _ = abort.call::<_, ()>((rquickjs::function::This(controller),));
                    }
                }
                Outcome::Stopped
            }
        }
    })
    .await;
    let ended = Instant::now();
    if !signal_only {
        if let Some(net) = &instance.net {
            net.end_call();
        }
    }
    instance.timers.sweep();
    arm(deadline, GRACE);
    (outcome, ended)
}

/// Drives what a call left behind until the runtime is idle; says how long that took, or that it did not.
async fn settle(instance: &Instance) -> String {
    let started = Instant::now();
    match tokio::time::timeout(Duration::from_secs(3), instance.runtime.idle()).await {
        Ok(()) => format!("idle after {:.0?}", started.elapsed()),
        Err(_) => "not idle after 3 s".into(),
    }
}

fn describe(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Done(Ok(json)) => format!("returned {}", json.chars().take(200).collect::<String>()),
        Outcome::Done(Err(e)) => format!("failed: {}", first_line(e)),
        Outcome::Stopped => "stopped by the host".into(),
    }
}

// a hub drives every runtime, so a request the call's end woke is dropped then, not at the runtime's next use
async fn instance_for(bytes: Vec<u8>, allow: Vec<String>, typed: Vec<String>) -> (Instance, Deadline) {
    let instance = Instance::with_net(Some((Some(allow), typed))).await.unwrap();
    tokio::task::spawn_local(instance.runtime.drive());
    let deadline = guard(&instance).await;
    arm(&deadline, INSTALL_TIME);
    load(&instance, bytes).await.unwrap();
    (instance, deadline)
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', ':']).next().unwrap_or("").to_string()
}

// the range of the chapter's example: whole seconds, as precision 'second' makes the kernel hand it
const RANGE: &str = r#""fromUs":1790812800000000,"toUs":1790899200000000"#;

/// The chapter's Stripe plugin against stripe-mock: declarations, a read, an action, and an instance that may not reach the mock.
pub async fn stripe(bundle: &str, mock: &str) {
    tokio::task::LocalSet::new().run_until(stripe_here(bundle, mock)).await
}

async fn stripe_here(bundle: &str, mock: &str) {
    let bytes = std::fs::read(bundle).unwrap();
    println!("install");
    match discover(bytes.clone()).await {
        Ok(json) => println!("  declared: {json}"),
        Err(e) => return println!("  refused: {}", first_line(&e)),
    }
    let settings = format!(r#"{{"key":"sk_test_123","api":"{mock}"}}"#);
    let (instance, deadline) = instance_for(bytes.clone(), vec!["api.stripe.com".into()], vec![host_of(mock)]).await;
    for (function, params, id) in [
        ("charges", format!(r#"{{{RANGE},"status":"succeeded"}}"#), "call-1"),
        ("charges", format!(r#"{{{RANGE},"status":"failed"}}"#), "call-2"),
        ("refund", r#"{"charge":"ch_123"}"#.to_string(), "op-refund-1"),
    ] {
        let started = Instant::now();
        let (outcome, _) = call(&instance, &deadline, Call { function, params, settings: settings.clone(), id, rows: 10, stop_after: None }).await;
        println!("  {function} {id}: {} in {:.0?}", describe(&outcome), started.elapsed());
    }
    if let Some(net) = &instance.net {
        println!("  hosts it asked for: {:?}", net.contacted());
    }
    instance.unload().await;

    println!("an instance whose admin did not type the mock's address");
    let (stranger, deadline) = instance_for(bytes, vec!["api.stripe.com".into()], vec![]).await;
    let params = format!(r#"{{{RANGE}}}"#);
    let (outcome, _) = call(&stranger, &deadline, Call { function: "charges", params, settings, id: "call-3", rows: 10, stop_after: None }).await;
    println!("  charges call-3: {}", describe(&outcome));
    stranger.unload().await;
}

struct Slow {
    url: String,
    accepted: AtomicUsize,
    closed: Mutex<Vec<Instant>>,
}

/// A server that reads a request and never answers, and notes when each connection is closed by the other side.
async fn slow_server() -> Arc<Slow> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://127.0.0.1:{}/slow", listener.local_addr().unwrap().port());
    let slow = Arc::new(Slow { url, accepted: AtomicUsize::new(0), closed: Mutex::default() });
    let server = slow.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            server.accepted.fetch_add(1, Ordering::SeqCst);
            let server = server.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                while let Ok(n) = socket.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                }
                server.closed.lock().unwrap().push(Instant::now());
            });
        }
    });
    slow
}

impl Slow {
    fn connections(&self) -> (usize, usize) {
        (self.accepted.load(Ordering::SeqCst), self.closed.lock().unwrap().len())
    }

    // how long after `since` the server saw its newest connection closed, waiting up to two seconds
    async fn closed_after(&self, open_before: usize, since: Instant) -> String {
        for _ in 0..200 {
            if let Some(at) = self.closed.lock().unwrap().iter().skip(open_before).max() {
                return format!("server saw the connection closed {:.1} ms after", at.saturating_duration_since(since).as_secs_f64() * 1e3);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        "server still holds the connection after 2 s".into()
    }
}

/// Requests a call leaves in flight: ended by the host, aborted by the signal, left behind, and a retry loop.
pub async fn abort(bundle: &str, stripe_bundle: &str, mock: &str) {
    tokio::task::LocalSet::new().run_until(abort_here(bundle, stripe_bundle, mock)).await
}

async fn abort_here(bundle: &str, stripe_bundle: &str, mock: &str) {
    let slow = slow_server().await;
    let typed = vec!["127.0.0.1".to_string(), host_of(mock)];
    let settings = format!(r#"{{"slow":"{}"}}"#, slow.url);
    let (instance, deadline) = instance_for(std::fs::read(bundle).unwrap(), vec![], typed.clone()).await;
    let cases: [(&str, &str, Option<(Duration, Stop)>); 4] = [
        ("waits", "the host ends the call at 300 ms; the fetch had no signal", Some((Duration::from_millis(300), Stop::End))),
        ("waitsWithSignal", "the host aborts only the call's signal at 300 ms", Some((Duration::from_millis(300), Stop::Signal))),
        ("leaves", "the call returns at once and leaves a fetch running", None),
        ("retries", "the host ends the call at 300 ms; the plugin retries in a loop", Some((Duration::from_millis(300), Stop::End))),
    ];
    for (function, what, stop_after) in cases {
        println!("{function}: {what}");
        let (accepted, closed) = slow.connections();
        let signal_only = matches!(stop_after, Some((_, Stop::Signal)));
        let (outcome, ended) =
            call(&instance, &deadline, Call { function, params: "{}".into(), settings: settings.clone(), id: function, rows: 10, stop_after }).await;
        println!("  {}", describe(&outcome));
        println!("  {}", slow.closed_after(closed, ended).await);
        println!("  {}", settle(&instance).await);
        if signal_only {
            if let Some(net) = &instance.net {
                net.end_call();
            }
        }
        let (now_accepted, _) = slow.connections();
        println!("  connections the server accepted during the case: {}", now_accepted - accepted);
    }
    instance.unload().await;

    println!("stripe: the chapter's plugin pointed at the slow server, no signal given to the SDK; the host ends the call at 300 ms");
    let stripe = std::fs::read(stripe_bundle).unwrap();
    let (instance, deadline) = instance_for(stripe, vec!["api.stripe.com".into()], typed).await;
    let (accepted, closed) = slow.connections();
    let slow_settings = format!(r#"{{"key":"sk_test_123","api":"{}"}}"#, slow.url.trim_end_matches("/slow"));
    let params = format!(r#"{{{RANGE}}}"#);
    let stop_after = Some((Duration::from_millis(300), Stop::End));
    let (outcome, ended) =
        call(&instance, &deadline, Call { function: "charges", params: params.clone(), settings: slow_settings, id: "slow-1", rows: 10, stop_after })
            .await;
    println!("  {}", describe(&outcome));
    println!("  {}", slow.closed_after(closed, ended).await);
    println!("  {}", settle(&instance).await);
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("  connections the server accepted, one second later: {}", slow.connections().0 - accepted);

    println!("stripe: the same instance, next call, against stripe-mock");
    let settings = format!(r#"{{"key":"sk_test_123","api":"{mock}"}}"#);
    let (outcome, _) = call(&instance, &deadline, Call { function: "charges", params, settings, id: "after-1", rows: 10, stop_after: None }).await;
    println!("  {}", describe(&outcome));
    instance.unload().await;
}
