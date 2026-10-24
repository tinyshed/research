// what one QuickJS plugin instance costs, what a hundred cost, and what unloading them gives back
#[path = "common.rs"]
mod common;

use std::time::{Duration, Instant};

use rquickjs::{Context, Function, Runtime};

fn instance() -> (Runtime, Context) {
    let rt = Runtime::new().unwrap();
    rt.set_memory_limit(16 << 20);
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| ctx.eval::<(), _>(common::PLUGIN).unwrap());
    (rt, ctx)
}

fn call(ctx: &Context) -> f64 {
    ctx.with(|ctx| {
        let query: Function = ctx.globals().get("query").unwrap();
        query.call::<_, f64>((10_000,)).unwrap()
    })
}

fn main() {
    println!("allocator: {}", common::allocator());
    std::thread::sleep(Duration::from_secs(1));
    common::checkpoint("start, quickjs linked, no runtime");
    let mut all = vec![instance()];
    common::checkpoint("1 instance, plugin loaded");
    call(&all[0].1);
    all[0].0.run_gc();
    common::checkpoint("1 instance, after one call and gc");
    let started = Instant::now();
    for _ in 0..100 {
        call(&all[0].1);
    }
    println!("  speed: 100 calls on one instance took {:?}", started.elapsed());
    all[0].0.run_gc();
    for n in [10, 100] {
        while all.len() < n {
            let next = instance();
            call(&next.1);
            next.0.run_gc();
            all.push(next);
        }
        common::checkpoint(&format!("{n} instances, each called once, gc'd"));
    }
    drop(all);
    common::checkpoint("all 100 unloaded, at once");
    std::thread::sleep(Duration::from_secs(1));
    common::checkpoint("all 100 unloaded, 1 s later");
    std::thread::sleep(Duration::from_secs(11));
    common::checkpoint("all 100 unloaded, 12 s later");
    let how = common::trim();
    common::checkpoint(&format!("forced: {how}"));
}
