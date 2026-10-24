# A plugin host on rquickjs and llrt_modules, 25 October 2026

The host Dashbin would build: one QuickJS runtime per plugin instance, web
APIs from `llrt_modules`, static musl with the tuned jemalloc of
[memory-and-allocators-2026-10-24](memory-and-allocators-2026-10-24.md). It
was built, the SDKs of [sdks-in-quickjs-2026-10-25](sdks-in-quickjs-2026-10-25.md)
were run in it, and its lifecycle and network isolation were pushed until
they held.

| Question | Answer | What follows |
|---|---|---|
| Do the SDKs run in our host? | Stripe, Octokit, OpenAI, Notion, libraries: yes, 2–10 ms a call | — |
| What does the host add? | 2.15 MiB bare (rquickjs, tokio, jemalloc), 9.19 MiB with the web APIs and the policy (9 633 144 B; 9.04 before the policy) | less inside the hub, which already has tokio and hyper; not measured |
| What does an instance cost? | the web APIs 3.4 MiB once, an empty instance 0.2, Stripe +2.8, OpenAI +4.8, Notion (browser bundle) +7.0, Octokit +1.4, libraries +1.3; five alive, 20.5 MiB for the process | the 64 MiB budget holds about eight active plugins beside a loaded hub |
| Does bytecode save memory? | No, the same after the call; it saves the cold start, 30 → 4 ms for OpenAI, 40 → 4 for Notion | the host compiles at install and keeps a cache; it never accepts an author's bytecode |
| Can an instance be unloaded? | Not on LLRT's timers: a live `setTimeout` or `setInterval` aborts the process in `JS_FreeRuntime` | the host owns its timers |
| Why? | `llrt_timers` keeps every runtime's timers in one `static RT_TIMER_STATE` with `Persistent` callbacks and a raw `*mut JSRuntime`, and has no call to drop a runtime's | turning the assertion off would turn the abort into a use-after-free |
| Does it leak? | Yes, 100 B an instance, whatever the plugin: `llrt_timers::init` registers every runtime and never removes it | the host builds LLRT's modules without its timers |
| With our timers and without LLRT's? | 40 000 instances that leave a timer, an interval, a request in flight or an endless promise: 6.3 → 6.2 MiB, no failure; 20 000 of Stripe and OpenAI: 12.6 → 13.8, no failure; Stripe 5 ms instead of 80 s | — |
| Can one instance reach what another may not? | LLRT's lists are one per process, match a suffix without a dot, and a redirect reached an internal service | a policy per instance, below |
| With the policy? | metadata, loopback, private and look-alike hosts refused with their reason, every redirect hop checked; two instances of one plugin in one process, at once: HTTP 200 for the one allowed, refusal for the other | the network part of the host is settled |

## Environment and reproduction

- `rquickjs` 0.11, `llrt_modules` 0.8.1-beta with `abort buffer console crypto
  crypto-rust events exceptions fetch tls-ring timers url util stream-web
  string-decoder navigator path`; the build needs LLRT's own `Cargo.lock`
  (copied into `plugin-host/Cargo.lock`), since its crypto rests on pre-release
  `rsa 0.10.0-rc.15` and `pkcs1 0.8.0-rc.4` that newer pre-releases break.
- `x86_64-unknown-linux-musl`, built in the `rssprobe-musl` image of
  `memory-probe/docker` with `CC_x86_64_unknown_linux_musl=musl-gcc` and
  `CXX_x86_64_unknown_linux_musl=g++` (zlib-ng's cmake asks for a C++ compiler
  it does not use). Run in `rust:1.99-bookworm`, Docker Desktop, kernel
  6.18.33.2-microsoft-standard-WSL2, 16 vCPU.
- `MALLOC_CONF=background_thread:true,narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0`
  except where a line says otherwise. Bundles are `sdk-probe/dist/min/*.mjs`,
  compiled to bytecode by the host itself (`plughost-web compile`).
- Switches in `src/main.rs`: `OUR_TIMERS` installs `src/timers.rs` over LLRT's,
  `NO_LLRT_TIMERS` builds LLRT's module set without its timers, `NET_POLICY`,
  `NET_ALLOW` and `NET_PRIVATE` give an instance its policy, `CAREFUL_DROP`,
  `WAIT_IDLE` and `PARK_LEAKS` are the teardown experiments.
- The harness was not committed before it ran; it is committed afterwards, in
  its final state. Runs 02–04 used an earlier `src/main.rs` (one instance
  type, no switches for timers or policy).

```sh
MSYS_NO_PATHCONV=1 docker run --rm -v <research>/dashbin/plugin-host:/src -v rssprobe-cargo:/usr/local/cargo/registry \
  -v plughost-target:/target -e CARGO_TARGET_DIR=/target -e CC_x86_64_unknown_linux_musl=musl-gcc \
  -e CXX_x86_64_unknown_linux_musl=g++ -w /src rssprobe-musl bash -c 'T=x86_64-unknown-linux-musl; \
  cargo build --release --target $T && mkdir -p /target/out && cp /target/$T/release/plughost /target/out/plughost-web'
# stripe-mock and sdk-probe/mock.mjs on a network `rssprobe`, then in rust:1.99-bookworm on it:
plughost-web compile /p/dist/min/stripe.mjs /tmp/stripe.qbc
OUR_TIMERS=1 NO_LLRT_TIMERS=1 plughost-web soak 10000 /tmp/stripe.qbc /tmp/openai.qbc
OUR_TIMERS=1 NO_LLRT_TIMERS=1 NET_POLICY=1 NET_PRIVATE=rssprobe-mock plughost-web /p/net/allowed.mjs
OUR_TIMERS=1 NO_LLRT_TIMERS=1 plughost-web pair /p/net/allowed.mjs
```

Raw output: [data/plugin-host-2026-10-25/](data/plugin-host-2026-10-25/), one
file a step, numbered as below.

## Unloading aborts the process (02, 03, 04)

With LLRT's default modules, freeing the runtimes after five SDKs ended in
`Assertion failed: list_empty(&rt->gc_obj_list)` in `JS_FreeRuntime` and an
abort. Only Stripe's instance did it; waiting for the runtime to go idle first
took 80 s and then freed cleanly. Plugins written to leave one thing behind
separated the cause: a cleared timer, a request in flight and a promise that
never settles freed cleanly; a live `setTimeout` or `setInterval` aborted.
QuickJS's object count cannot serve as a detector: 572 objects remain even in
an instance that frees cleanly.

## Timers the host owns (08)

`src/timers.rs`, about a hundred lines: a timer is a task spawned inside its
own runtime with a `Notify` to stop it; the registry is the instance's; after
every call and before unloading, `sweep()` stops what is left. A runtime
dropped with timers takes them with it, as it already did with a request in
flight. 100 ms slept 101; an interval stopped at its third tick; a cleared
timer did not fire; arguments reached the callback. Every leftover unloaded
cleanly where LLRT's timer aborted.

## A leak per instance (05, 06, 07)

10 000 cycles of load, call and unload grew the process by about 1.5 MiB
whatever the plugin, an empty module included, and with LLRT's timers untouched
as well. In the code, `llrt_timers::init` pushes a `RuntimeTimerState` for every
runtime into the global vector and never removes it. 30 000 cycles: 6.2 → 9.4
MiB with LLRT's module set, 6.2 → 6.2 with the same set built by
`ModuleBuilder::new()` without its timers.

## The final host under load (09, 10)

| | instances | memory over the run | failures | time |
|---|---|---|---|---|
| timer, interval, request in flight, endless promise | 40 000 | 6.3 → 6.2 MiB | 0 | 14.2 s |
| Stripe + OpenAI | 20 000 | 12.6, 14.1 at cycle 100, 13.8 at 10 000 | 0 | 86.7 s |

## A network policy per instance (11, 12)

`vendor/llrt_fetch` is LLRT's crate with one new file and three small
changes, kept with its `LICENSE`, `NOTICE` and an `UPSTREAM` that names what
changed. An instance with a policy sends through its own hyper client, whose
resolver drops private, loopback, shared and link-local addresses unless an
operator typed the host, and link-local always; an address written as digits
is judged before connecting; every redirect hop is checked; `*.stripe.com`
matches on a dot; an `agent` option is refused; the resolver's reason reaches
the plugin in words; the host can list every host an instance asked for.

| | LLRT as is | policy, nothing private | + operator typed the mock | only `*.stripe.com` |
|---|---|---|---|---|
| internal host | — | refused | HTTP 200 | refused |
| `169.254.169.254` | — | refused | — | — |
| `127.0.0.1` | tried to connect | refused | — | — |
| `localhost` | — | refused by the resolver | — | — |
| redirect to metadata | — | refused | refused at the hop | — |
| redirect to another internal host | HTTP 401 from it | refused | refused | — |
| `evil-stripe.com` | connect error | connect error | — | refused by the rule |
| `example.com` | — | HTTP 200 | — | refused |

Two instances of `net/allowed.mjs` in one process at once, one allowed the
mock and one not: HTTP 200 and a refusal. Under a policy the five SDKs ran
(Stripe asked for its host twice, a list and a refund); 40 000 leaking
instances stayed at 6.1 MiB; 20 000 of Stripe and OpenAI ended at 13.7 MiB with
no failure, 13 % slower than without a policy, since every new instance opens
its own connections.

## What follows

Built into Dashbin when its repository is started: the instance as measured,
timers of its own, LLRT's modules without its timers, `vendor/llrt_fetch`
with the policy. Four faults of LLRT were found on the way; reporting them
upstream is Dmitry's call.
