# Chapter 6's plugins in the host, 25 October 2026

Chapter 6 left three things as tests before promises: its Stripe example
against `stripe-mock`, the host aborting a call's requests whatever the SDK
does, and installing a plugin under limits. All three ran in the plugin host
of [the earlier round](plugin-host-2026-10-25.md), extended with the
`dashbin` module, discovery and calls. Two of them needed a change to the
host before they held, and the round found a crash in rquickjs.

| Question | Answer | What follows |
|---|---|---|
| Does the chapter's example run? | Yes, as written, against `stripe-mock`: its declarations read back exactly, `charges` with and without a match, `refund` with the operation's id; Stripe's worker build uses `fetch` without being told | the example stands |
| Does an instance that may not reach a host fail clearly? | It fails, but Stripe's SDK replaces the host's reason with "An error occurred with our connection to Stripe" and retries twice | the host keeps its own refusals per call and puts them beside the plugin's error |
| Does the call's end drop its requests? | Yes, 0.1–0.2 ms after the end, in all seven cases, Stripe's included, **once the host drives the runtime after the end** | the end of a call is: stop requests, stop timers, drive to idle, judge |
| Without that drive? | The connection stays open: rquickjs keeps one waker per runtime, and `async_with` takes it from a runtime's driver | a driver is not enough; the host's own step after the call is |
| Does `signal` abort a `fetch`? | Yes, 0.1 ms | chapter 5's open row is closed |
| A retry the SDK schedules after the end? | Stripe's backoff timer kept the runtime busy 1 s; with timers closed between calls as requests are, 117 µs | timers are gated like requests |
| A plugin that retries in a tight loop? | Ended by the interrupt handler 500 ms after the call | a grace for leftovers, enforced, not hoped for |
| Do install limits hold? | An endless loop, deep recursion, a memory bomb, a top-level `fetch`, a top-level wait and a module declaring nothing: all refused, the process alive | the install contract stands |
| What did the memory bomb cost? | Its runtime was limited to 24 MiB and failed at it; the process's RSS rose to 53.9 MiB and fell back to 7.3 within two seconds | an install limit is not an RSS limit; installs need a lower limit and one at a time |
| Anything worse? | Unloading a runtime after a job was interrupted **aborted the process**: `Assertion failed: p->ref_count > 0` in QuickJS's GC | rquickjs-core 0.11.0's bug, fixed in 0.12.0; LLRT pins 0.11, so the host carries the fix |

## Environment and reproduction

- The plugin host as in `plugin-host/` at e375bb2 and 5afef12: rquickjs 0.11,
  `llrt_modules` 0.8.1-beta with our timers and the vendored `llrt_fetch`,
  static musl, the tuned jemalloc (`MALLOC_CONF` as in the memory round), built
  in the `rssprobe-musl` image; `rquickjs-core` 0.11.0 vendored with one fix
  from the third run on.
- `stripe/stripe-mock@sha256:aeb96b83…`, stripe 23.0.1 as locked in
  `plugin-host/plugins/bun.lock`, bundled by `plugins/build.sh` with
  `bun build --target=browser --conditions=worker --external dashbin`, which
  is what `dashbin plugin build` would do.
- Docker Desktop on WSL2, 16 vCPU. The slow server is inside the host's
  process: it accepts, reads, never answers, and notes when the other side
  closes. Timings are one pass each; the four runs repeated each other within
  0.1 ms wherever the harness was the same.
- The harness was committed before each run: 5588441, 5f1b362, e375bb2,
  5afef12. Between runs it changed for what the run before had found, and the
  raw output of every run is kept.

```sh
cd <research>/dashbin/plugin-host && bash calls.sh
```

Raw output: [data/plugin-calls-2026-10-25/](data/plugin-calls-2026-10-25/),
one file a run.

## The example on stripe-mock

The plugin is the chapter's with one change, an `api` setting that points the
SDK at the mock the way an admin would type a URL. Installing it read back:

```json
{"settings":{"key":{"type":"secret","title":"Secret key"},"api":{"type":"url","optional":true}},
 "hosts":["api.stripe.com"],
 "functions":{"charges":{"kind":"read","params":{"status":{"type":"text","optional":true}},
   "columns":{"amount":{"unit":"money","currency":"currency"}},"precision":"second"},
   "refund":{"kind":"action","params":{"charge":{"type":"text"}}}}}
```

| Call | Result |
|---|---|
| `charges`, whole seconds, `status=succeeded` | one row, 3 ms: time, amount 100, `usd`, `succeeded` |
| `charges`, `status=failed` | no rows, 1 ms: filtered in the plugin, as the chapter says the list cannot |
| `refund`, id `op-refund-1` | a refund id, `succeeded`, 1 ms |
| `charges` on an instance whose admin did not type the mock's address | "An error occurred with our connection to Stripe. Request was retried 2 times." |

The last row is right and unhelpful. The policy refused with
"dashbin: plughost-stripe is not a host this plugin may reach", and the SDK
caught it, retried twice and threw its own text. An SDK will do that to any
error the host raises, so **the host must keep the refusals it made during a
call and report them beside whatever the plugin throws**, or an admin reads
"connection error" where the answer was one missing host.

## The end of a call

Seven plugins left a request open against a server that never answers, and
the host stopped each call at 300 ms:

| Case | Server saw the connection close | Runtime idle | Connections |
|---|---|---|---|
| `fetch` without a signal, the call ended | 0.1 ms after the end | 44 µs | 1 |
| `fetch` with the call's `signal`, only the signal aborted | 0.1 ms | 33 µs | 1 |
| the call returned and left a `fetch` behind | 0.1 ms | 22 µs | 1 |
| a tight loop retrying `fetch`, the call ended | 0.1 ms | 501 ms, interrupted | 1 |
| Stripe's shape: own controller, no timer | 0.1 ms | 44 µs | 1 |
| Stripe's shape: own controller, an 80 s timeout timer | 0.1 ms | 56 µs | 1 |
| the chapter's Stripe plugin, the SDK given no signal | 0.2 ms | 117 µs | 1, no retry reached the server |

After the Stripe case, the same instance's next call against `stripe-mock`
returned its row: ending a call does not spoil the runtime.

What made it hold, in the order the host does it:

1. **Requests start only inside a call** and are dropped when it ends: a gate
   in the vendored `llrt_fetch`, around sending and around reading a body.
2. **Timers do the same**: the ones the call left are stopped, and one its
   leftovers start after the end never fires. Without this, Stripe's retry
   backoff kept the runtime busy for a second (second and third runs).
3. **The host drives the runtime until it is idle.** A dropped request is
   only dropped when its future is polled. The first run judged before
   driving, and every connection was still open two seconds later. A driver
   task spawned beside the runtime did not help; as rquickjs's code reads, a
   runtime keeps one waker for its spawned futures, and every `async_with`
   replaces the driver's with its own.
4. **What still runs after a grace is interrupted**: the interrupt handler,
   armed for 500 ms at the call's end, stopped the retry loop.

## Installing under limits

Each module ran in a runtime of its own: no host reachable, no call open, no
settings, 24 MiB, a 512 KiB stack, an interrupt at 2 s.

| Module | Refused with | In |
|---|---|---|
| `while (true) {}` at the top level | `interrupted` | 2 s |
| endless recursion | `Maximum call stack size exceeded` | 3 ms |
| arrays pushed until memory runs out | `out of memory` | 41–63 ms |
| `await fetch(…)` at the top level | `dashbin: no call is running in this instance` | 2–3 ms |
| `await new Promise(() => {})` | its top level did not finish within 2 s | 2 s |
| a module that declares no function | `dashbin: the module declares no function` | 2 ms |

The real plugins declared in 24–26 ms (Stripe) and 2 ms. The process lived
through all of it. The memory bomb is the one to read twice: its runtime hit
its 24 MiB and failed, but the process's RSS went from 7.1 to 53.9 MiB
before it did, and was back at 7.3 two seconds later as jemalloc returned the
pages. A QuickJS limit bounds what JavaScript holds, not what the allocator
keeps while it reallocates, so under a 64 MiB promise an install runs with a
smaller limit than an instance and never two at once.

## The crash, and whose it is

In the first two runs, unloading the runtime whose retry loop had been
interrupted aborted the process:

```
Assertion failed: p->ref_count > 0 (quickjs.c: gc_decref_child: 6183)
```

`JS_ExecutePendingJob` returns the context of a job that threw as a borrowed
pointer. rquickjs-core 0.11.0's `AsyncRuntime::idle` and
`execute_pending_job` wrap it in `AsyncContext::from_raw`, which frees a
reference when dropped, so the context loses one reference it never had, and
the garbage collector meets it later. rquickjs-core 0.12.0 (27 May 2026) adds
`JS_DupContext` before the wrap, and 0.14.0 still has it. `llrt_modules` and
`llrt_fetch` 0.8.1-beta require `rquickjs ^0.11`, so a host on LLRT gets the
bug. With the two lines of 0.12.0 applied to a vendored 0.11.0
(`plugin-host/vendor/rquickjs-core`, its `UPSTREAM` says what changed), the
same case unloads cleanly in the third and fourth runs. A job ends in an
exception only when it cannot become a rejection — an interrupt, memory
running out — which are exactly the ways a host stops a plugin, so a host
that interrupts must carry the fix until LLRT moves.

## What follows

- **For chapter 6**: the end of a call is four steps — gate requests, gate
  timers, drive to idle, interrupt after a grace — and that is what "the host
  aborts every request it started" means in code. The example stands as
  written.
- **The host reports its own refusals** next to the plugin's error, since SDKs
  rewrap them.
- **An install runs alone, with a smaller memory limit** than an instance;
  the number belongs to the load run.
- **The vendored rquickjs-core** joins `llrt_fetch` in the thin fork, until
  LLRT requires 0.12 or later. Whether to tell LLRT, or rquickjs, is Dmitry's
  call; rquickjs fixed it already.
