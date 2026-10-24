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
