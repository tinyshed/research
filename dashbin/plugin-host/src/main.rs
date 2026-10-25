// a plugin host as dashbin would have it: one quickjs runtime per plugin instance, web apis from llrt
mod calls;
mod timers;

use std::time::{Duration, Instant};

use rquickjs::module::WriteOptions;
use rquickjs::{AsyncContext, AsyncRuntime, CatchResultExt, Module, Object, async_with};

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn rss() -> (f64, f64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let pick = |key: &str| {
        status
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l[key.len()..].trim().trim_end_matches(" kB").trim().parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1024.0
    };
    (pick("VmRSS:"), pick("VmHWM:"))
}

fn checkpoint(what: &str) {
    let (now, peak) = rss();
    eprintln!("{what:<46} VmRSS {now:>5.1} MiB  peak {peak:>5.1}");
}

fn purge() {
    #[cfg(feature = "jemalloc")]
    unsafe {
        tikv_jemalloc_sys::mallctl(c"arena.4096.purge".as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0);
    }
}

// llrt's default set, or the same set without llrt's timers, whose init registers every runtime forever
#[cfg(feature = "web-apis")]
fn modules() -> llrt_modules::module_builder::ModuleBuilder {
    use llrt_modules::module_builder::ModuleBuilder;
    if std::env::var("NO_LLRT_TIMERS").is_err() {
        return ModuleBuilder::default();
    }
    ModuleBuilder::new()
        .with_global(llrt_modules::module::init)
        .with_module(llrt_modules::module::ModuleModule)
        .with_global(llrt_modules::abort::init)
        .with_global(llrt_modules::buffer::init)
        .with_module(llrt_modules::buffer::BufferModule)
        .with_global(llrt_modules::console::init)
        .with_module(llrt_modules::console::ConsoleModule)
        .with_global(llrt_modules::crypto::init)
        .with_module(llrt_modules::crypto::CryptoModule)
        .with_global(llrt_modules::events::init)
        .with_module(llrt_modules::events::EventsModule)
        .with_global(llrt_modules::exceptions::init)
        .with_global(llrt_modules::fetch::init)
        .with_global(llrt_modules::navigator::init)
        .with_module(llrt_modules::path::PathModule)
        .with_global(llrt_modules::stream_web::init)
        .with_module(llrt_modules::stream_web::StreamWebModule)
        .with_module(llrt_modules::string_decoder::StringDecoderModule)
        .with_global(llrt_modules::url::init)
        .with_module(llrt_modules::url::UrlModule)
        .with_global(llrt_modules::util::init)
        .with_module(llrt_modules::util::UtilModule)
}

fn ours() -> bool {
    std::env::var("OUR_TIMERS").is_ok()
}

// what an instance may reach: NET_ALLOW is a list of hosts (absent: any public host), NET_PRIVATE the hosts an operator typed
type Net = Option<(Option<Vec<String>>, Vec<String>)>;

fn net_from_env() -> Net {
    let list = |name: &str| std::env::var(name).ok().map(|v| v.split(',').filter(|h| !h.is_empty()).map(String::from).collect::<Vec<_>>());
    std::env::var("NET_POLICY").ok()?;
    Some((list("NET_ALLOW"), list("NET_PRIVATE").unwrap_or_default()))
}

struct Instance {
    runtime: AsyncRuntime,
    context: AsyncContext,
    timers: timers::Timers,
    #[cfg(feature = "web-apis")]
    net: Option<std::sync::Arc<llrt_modules::fetch::policy::Policy>>,
}

impl Instance {
    async fn new() -> rquickjs::Result<Self> {
        Self::with_net(net_from_env()).await
    }

    async fn with_net(net: Net) -> rquickjs::Result<Self> {
        let runtime = AsyncRuntime::new()?;
        runtime.set_memory_limit(32 << 20).await;
        #[cfg(feature = "web-apis")]
        let (resolver, loader, globals) = modules().build();
        let dashbin = (
            rquickjs::loader::BuiltinResolver::default().with_module("dashbin"),
            rquickjs::loader::BuiltinLoader::default().with_module("dashbin", include_str!("dashbin.js")),
        );
        #[cfg(feature = "web-apis")]
        runtime.set_loader((resolver, dashbin.0), (loader, dashbin.1)).await;
        #[cfg(not(feature = "web-apis"))]
        runtime.set_loader((dashbin.0,), (dashbin.1,)).await;
        let context = AsyncContext::full(&runtime).await?;
        let timers = timers::Timers::default();
        let ours = ours();
        let installing = timers.clone();
        #[allow(unused_variables)]
        let policy = async_with!(context => |ctx| {
            #[cfg(feature = "web-apis")]
            globals.attach(&ctx)?;
            #[cfg(feature = "web-apis")]
            let policy = match net {
                Some((allow, private)) => Some(
                    llrt_modules::fetch::policy::set(&ctx, allow, private)
                        .map_err(|e| rquickjs::Error::new_from_js_message("policy", "network", e.to_string()))?,
                ),
                None => None,
            };
            #[cfg(not(feature = "web-apis"))]
            let policy = { let _ = net; () };
            if ours {
                installing.install(&ctx)?;
            }
            // the instance's own settings, never the host's environment
            let process = Object::new(ctx.clone())?;
            let env = Object::new(ctx.clone())?;
            env.set("STRIPE_KEY", "sk_test_123")?;
            process.set("env", env)?;
            ctx.globals().set("process", process)?;
            Ok::<_, rquickjs::Error>(policy)
        })
        .await?;
        Ok(Instance {
            runtime,
            context,
            timers,
            #[cfg(feature = "web-apis")]
            net: policy,
        })
    }

    // a call is over once its promise settles; what it left running is swept, like a closed browser tab's
    async fn run(&self, bytes: &[u8], from_source: bool) -> Option<String> {
        let bytes = bytes.to_vec();
        let failed = async_with!(self.context => |ctx| {
            let module = if from_source {
                Module::declare(ctx.clone(), "plugin", bytes).catch(&ctx)
            } else {
                unsafe { Module::load(ctx.clone(), &bytes) }.catch(&ctx)
            };
            let outcome = match module {
                Ok(module) => match module.eval().catch(&ctx) {
                    Ok((_, promise)) => promise.into_future::<()>().await.catch(&ctx).err(),
                    Err(e) => Some(e),
                },
                Err(e) => Some(e),
            };
            outcome.map(|e| e.to_string())
        })
        .await;
        if ours() {
            self.timers.sweep();
        }
        if std::env::var("WAIT_IDLE").is_ok() {
            self.runtime.idle().await;
        }
        failed
    }

    async fn unload(self) {
        if ours() {
            self.timers.sweep();
        }
        drop(self.context);
        self.runtime.run_gc().await;
        drop(self.runtime);
    }
}

async fn compile(input: &str, output: &str) {
    let source = std::fs::read(input).unwrap();
    let instance = Instance::new().await.unwrap();
    let bytes = async_with!(instance.context => |ctx| {
        let module = Module::declare(ctx.clone(), "plugin", source).catch(&ctx).unwrap();
        module.write(WriteOptions::default()).unwrap()
    })
    .await;
    instance.unload().await;
    std::fs::write(output, &bytes).unwrap();
    eprintln!("{input} -> {output}: {} KiB", bytes.len() / 1024);
}

async fn soak(cycles: usize, paths: &[String]) {
    let plugins: Vec<(Vec<u8>, bool)> = paths.iter().map(|p| (std::fs::read(p).unwrap(), p.ends_with(".mjs"))).collect();
    let started = Instant::now();
    let mut failures = 0usize;
    checkpoint(&format!("soak {cycles} cycles of {}, our timers={}", paths.join(" "), ours()));
    for cycle in 1..=cycles {
        for (bytes, from_source) in &plugins {
            let instance = Instance::new().await.unwrap();
            if let Some(e) = instance.run(bytes, *from_source).await {
                failures += 1;
                if failures <= 3 {
                    eprintln!("  failure: {}", e.lines().next().unwrap_or(""));
                }
            }
            instance.unload().await;
        }
        if [1, 100, 1000, 2500, 5000, 7500, 10000].contains(&cycle) || cycle == cycles {
            checkpoint(&format!("  cycle {cycle:>5}, {:>6.1} s, failures {failures}", started.elapsed().as_secs_f64()));
        }
    }
    purge();
    checkpoint("  after purge");
}

// two instances in one process run the same plugin at once: only the one an operator let into the private network gets in
async fn pair(path: &str) {
    let bytes = std::fs::read(path).unwrap();
    let trusted = Instance::with_net(Some((None, vec!["rssprobe-mock".into()]))).await.unwrap();
    let stranger = Instance::with_net(Some((None, vec![]))).await.unwrap();
    let (a, b) = tokio::join!(trusted.run(&bytes, true), stranger.run(&bytes, true));
    eprintln!("  instance allowed rssprobe-mock: {}", a.map(|e| format!("FAILED {}", e.lines().next().unwrap_or(""))).unwrap_or("ok".into()));
    eprintln!("  instance not allowed it:        {}", b.map(|e| format!("FAILED {}", e.lines().next().unwrap_or(""))).unwrap_or("ok".into()));
    trusted.unload().await;
    stranger.unload().await;
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args[1].as_str() {
        "compile" => return compile(&args[2], &args[3]).await,
        "soak" => return soak(args[2].parse().unwrap(), &args[3..]).await,
        "pair" => return pair(&args[2]).await,
        "discover" => return calls::discover_all(&args[2..]).await,
        "stripe" => return calls::stripe(&args[2], &args[3]).await,
        "abort" => return calls::abort(&args[2], &args[3], &args[4], &args[5]).await,
        _ => {}
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    checkpoint(&format!("start, web-apis={}, our timers={}", cfg!(feature = "web-apis"), ours()));
    let mut alive = Vec::new();
    for path in &args[1..] {
        eprintln!("{path}");
        let instance = Instance::new().await.unwrap();
        let started = Instant::now();
        if let Some(e) = instance.run(&std::fs::read(path).unwrap(), path.ends_with(".mjs")).await {
            eprintln!("  FAILED: {}", e.lines().take(3).collect::<Vec<_>>().join(" | "));
        }
        eprintln!("  took {:?}", started.elapsed());
        #[cfg(feature = "web-apis")]
        if let Some(net) = &instance.net {
            eprintln!("  hosts it asked for: {:?}", net.contacted());
        }
        alive.push(instance);
        checkpoint("  after it ran, instance kept");
    }
    for instance in alive {
        instance.unload().await;
    }
    checkpoint("all unloaded");
    purge();
    checkpoint("all unloaded, purged");
}
