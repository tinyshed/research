// timers that live inside their runtime: a dropped runtime takes its timers with it, and sweep stops them early
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use rquickjs::prelude::{Opt, Rest};
use rquickjs::{Ctx, Function, Result, Value};
use tokio::sync::Notify;

#[derive(Clone, Default)]
pub struct Timers {
    running: Rc<RefCell<HashMap<u32, Rc<Notify>>>>,
    last: Rc<Cell<u32>>,
}

impl Timers {
    pub fn install<'js>(&self, ctx: &Ctx<'js>) -> Result<()> {
        let globals = ctx.globals();
        for (name, repeat) in [("setTimeout", false), ("setInterval", true)] {
            let timers = self.clone();
            let start = move |ctx: Ctx<'js>, callback: Function<'js>, delay: Opt<f64>, args: Rest<Value<'js>>| {
                timers.start(&ctx, callback, delay.0.unwrap_or(0.0), args.0, repeat)
            };
            globals.set(name, Function::new(ctx.clone(), start)?)?;
        }
        for name in ["clearTimeout", "clearInterval"] {
            let timers = self.clone();
            let clear = move |id: Opt<Value<'js>>| timers.clear(id.0.and_then(|v| v.as_number()));
            globals.set(name, Function::new(ctx.clone(), clear)?)?;
        }
        Ok(())
    }

    fn start<'js>(&self, ctx: &Ctx<'js>, callback: Function<'js>, delay: f64, args: Vec<Value<'js>>, repeat: bool) -> u32 {
        let id = self.last.get().wrapping_add(1);
        self.last.set(id);
        let stop = Rc::new(Notify::new());
        self.running.borrow_mut().insert(id, stop.clone());
        let delay = Duration::from_millis(if delay.is_finite() && delay > 0.0 { delay as u64 } else { 0 });
        let timers = self.clone();
        ctx.spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = stop.notified() => return,
                }
                if callback.call::<_, ()>((Rest(args.clone()),)).is_err() {
                    let _ = callback.ctx().catch();
                }
                if !repeat {
                    break;
                }
            }
            timers.running.borrow_mut().remove(&id);
        });
        id
    }

    fn clear(&self, id: Option<f64>) {
        if let Some(stop) = id.and_then(|id| self.running.borrow_mut().remove(&(id as u32))) {
            stop.notify_one();
        }
    }

    // stops every timer a call left behind; says how many there were
    pub fn sweep(&self) -> usize {
        let left: Vec<_> = self.running.borrow_mut().drain().collect();
        for (_, stop) in &left {
            stop.notify_one();
        }
        left.len()
    }
}
