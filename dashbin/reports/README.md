# Reports

The Rust rewrite of Dashbin, before its repository exists: what TinyStore
gives it, what its memory costs, and the plugin host it will carry.

- [TinyStore 0.1.0 as Dashbin would use it](tinystore-0.1.0-from-dashbin-2026-10-23.md):
  five inconveniences run against the published crate, filed as tinyshed/tinystore#1–#5.
- [TinyStore 0.1.1 against the five issues](tinystore-0.1.1-recheck-2026-10-24.md):
  all five rerun and closed; `tinystore::aio` stops a read by dropping its future.
- [Memory of a Rust Dashbin, and its allocator](memory-and-allocators-2026-10-24.md):
  the hub, QuickJS, Bun and Node plugins, the agent; glibc, musl, mimalloc and jemalloc
  on one, two and eight cores and under container limits; static musl with tuned jemalloc chosen.
- [Real SDKs inside QuickJS](sdks-in-quickjs-2026-10-25.md):
  Stripe, Octokit, OpenAI, Notion and libraries run in LLRT; Slack and `pg` do not; what a plugin costs.
- [A plugin host on rquickjs and llrt_modules](plugin-host-2026-10-25.md):
  the SDKs in our own host, an abort on unload and a leak per instance traced to `llrt_timers`,
  timers of our own, a network policy per instance; 60 000 instances without a failure.
- [A frame on the wire](wire-format-2026-10-25.md):
  JSON, Arrow IPC and a layout of our own on 50 000 rows in V8, Chromium and JavaScriptCore;
  JSON by the rule in round 1; labels as a dictionary in round 2, and our layout taken over Arrow
  as an exception to the margins, on what Arrow weighs in the binary and the page.
- [Chapter 6's plugins in the host](plugin-calls-2026-10-25.md):
  the Stripe example on stripe-mock, install limits, and a call's end dropping its requests in 0.1–0.2 ms
  once timers are gated and the runtime driven; a crash in rquickjs-core 0.11.0, fixed in 0.12.0, carried as a patch.
- [Memory with plugins](plugin-memory-2026-10-25.md):
  install limits against the process's peak, what four real SDKs need, a spike inside a call, and the hub
  with its instances in a 64 MiB container: eight real instances did not fit kept loaded, sixteen did with
  QuickJS in its own arena, GC after a call, idle runtimes unloaded and four slots.
- [TinyStore 0.1.1 against what flows need](tinystore-0.1.1-flows-2026-10-25.md):
  an event appended twice is two, an identical metrics batch is refused once packed, missed repeats collapse into one;
  seven asks besides, #25–#31.
