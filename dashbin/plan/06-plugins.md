# 6. Plugins

**Status: decided, 25 October 2026**, except what *Not measured yet* lists.
The first draft's seven forks went through one review; what it changed is
listed under *Forks*.

Dmitry's three demands: a plugin is ordinary code that any programmer would
write without reading a manual first; anyone may write one, in JavaScript or
another language, visualizations included; and plugins must not swell the
process. October's earlier designs — a `ctx` that became an operating system,
handles named after services, a manifest that repeated the code — failed the
first demand, and this chapter starts from it.

## A plugin, as its author writes it

A plugin is a module of functions. This one is complete:

```js
import Stripe from 'stripe'
import { settings, hosts, secret, text, money, read, action } from 'dashbin'

settings({ key: secret({ title: 'Secret key' }) })
hosts('api.stripe.com')

export const charges = read({
  params: { status: text({ optional: true }) },
  columns: { amount: money('currency') },    // minor units; the currency is that column
  precision: 'second',                       // so the kernel hands this read whole seconds
}, async ({ from, to, status }, { config, limit }) => {
  const stripe = new Stripe(config.key.reveal())
  const created = { gte: from.getTime() / 1000, lt: to.getTime() / 1000 }
  const rows = []
  for await (const c of stripe.charges.list({ created, limit: 100 })) {
    if (status && c.status !== status) continue    // the list takes no status filter
    rows.push({ time: new Date(c.created * 1000), amount: c.amount, currency: c.currency, status: c.status })
    if (rows.length > limit.rows) break
  }
  return rows
})

export const refund = action({
  params: { charge: text() },
}, async ({ charge }, { config, id }) => {
  const stripe = new Stripe(config.key.reveal())
  const r = await stripe.refunds.create({ charge }, { idempotencyKey: id })
  return { refund: r.id, status: r.status }
})
```

What each part is:

- **`settings`** declares what an instance must be given. The kernel reads it
  at install and builds the form; the values never reach the module's top
  level, only a call.
- **`hosts`** declares where the plugin talks to; nothing else is reachable
  (below).
- **A function's id is its export name**, forever. Renaming keeps the old one
  as an alias (`read({ was: 'payments' }, …)`), or every saved query naming it
  breaks. A title is optional; translations live in `locales/ru.json`, keyed by
  the same ids.
- **`params`** declares the function's own parameters, with chapter 3's
  types, for the same reason a saved query declares its own (chapter 5): the
  query editor builds its form from them and the kernel converts and refuses
  by name.
- **`columns`** declares what a column cannot say by itself: a unit, money in
  minor units with its currency column (chapter 3), the type of a column that
  may come back empty.
- **`precision`** is how finely the source takes a time; the kernel widens a
  finer range to it before the call and says so (chapter 5), which is why
  `from.getTime() / 1000` is whole here.
- **The first argument** holds the parameters: the kernel's (`from`, `to`,
  `fromUs`, `toUs`, `step`, `zone`) and the function's own, bound.
- **The second** holds the call's own values: `config` (this instance's
  settings, a secret as an object whose `reveal()` gives the text), `id` (the
  operation's id, below), `signal` (aborted when the call is cancelled, for
  code that wants to stop early) and `limit` (rows: fetch at most one more).
- **A read returns rows**, plain objects; the kernel builds the frame by
  chapter 3's table. An action returns any JSON value.

This example ran as written against `stripe-mock` in Dashbin's host
([plugin calls](../reports/plugin-calls-2026-10-25.md)): its declarations
read back exactly, both functions answered, and Stripe's worker build used
`fetch` without being told to.

## Installing reads declarations, a call gets settings

**The top level declares, functions do.** At install the kernel evaluates
the module once and reads what `settings`, `hosts` and the exported functions
declared; nothing is written twice, the code is the manifest. That evaluation
runs someone else's code, so it runs in a runtime of its own with:

- no network and no settings — there may be no instance yet, and there are
  no secrets to see;
- a memory limit and a stack limit;
- an interrupt handler with a deadline, so `while (true) {}` ends the install
  with an error instead of hanging the hub.

A `fetch` at the top level is refused, and so is a module that declares
nothing. Real settings reach only a call, in an instance's own runtime. All
six ways of breaking an install were refused with the process alive; a
memory bomb under a 24 MiB limit still took the process's RSS to 54 MiB for
two seconds, so **an install runs alone, with a smaller memory limit than an
instance**, the number from the load run.

## The four kinds

| Kind | Called by | Returns |
|---|---|---|
| `read` | a panel, a schedule, an alert, the query editor — again and again | rows |
| `action` | a click, a step of a flow — only on purpose | any JSON value |
| `webhook` | a request from outside to an address the hub gives the instance | a response, and rows the flow keeps |
| `stream` | the kernel, for as long as the instance is enabled | rows, as they come |

**A kind is the author's word, not a guarantee.** Nothing stops a `read`
from sending a `POST`; what covers it is the trust the admin gave the plugin.
The built-in database reads are the exception, read-only by transaction, as
in Go Dashbin.

A `webhook` gets the request as a `Request`, verifies it with its own
settings and returns rows; the flow around it (chapter 7) decides where they
go. A `stream` is for process plugins in the first version: MQTT, a log
tail. QuickJS streams wait until a QuickJS runtime can run more than one call
(chapter 5).

## What a function can reach

Web standards, the ones a browser and a Worker share, implemented in Rust
under the instance's rules:

| Available | Notes |
|---|---|
| `fetch`, `Request`, `Response`, `Headers`, `URL`, streams | under the instance's network policy, below |
| `AbortController`, `AbortSignal` | the call's `signal` is one |
| `setTimeout`, `setInterval` and their clears | the host's own; whatever a call leaves running is stopped when it ends |
| `TextEncoder`, `TextDecoder`, `crypto` (`subtle`, `randomUUID`), `structuredClone`, `atob`, `btoa` | — |
| `console` | goes to the hub's log with the instance and the call |

Not available: files, processes, raw sockets, `process.env`. A plugin that
needs TCP — the `pg` driver, Slack's SDK with its Node pieces — is a process
plugin, or the database is a built-in. `bun build --target=browser` already
refuses `pg` on its first line, so the plugin's build catches it before
Dashbin does.

**When a call ends or is cancelled, the host aborts every request it
started**, whether the SDK passes a `signal` along or not; Stripe's
documented request options have none. A runtime runs one call at a time
(chapter 5), so a call's requests are simply the runtime's requests in
flight. `signal` stays for the author's own code. In code, the end of a call
is four steps, each found necessary by the run:

1. requests start only inside a call, and those in flight are dropped;
2. timers do the same: the ones left are stopped, and one started after the
   end never fires — otherwise an SDK's retry backoff keeps the runtime busy;
3. the host drives the runtime until it is idle, since a dropped request is
   dropped only when its future is polled;
4. whatever still runs after a grace of half a second is interrupted.

Measured on seven plugins, Stripe's included: every connection closed
0.1–0.2 ms after the call's end, and no retry reached the server.

**Secrets are masked, not sealed off.** The kernel keeps a secret wrapped,
never prints the wrapper, and replaces the instance's secret values with
`***` in what `console` writes. Once `reveal()` has returned the text, it is
the plugin's: it can print it in pieces or send it to one of its hosts. What
bounds that is the list of hosts and the admin's trust, not the masking.

**Reads are by range in the first version.** The kernel gives a read its
window, and nothing persists between calls but what the module keeps in
memory while its runtime is loaded: a cached client, a token. An API that
syncs by a cursor — "changes after token X" — has no general support yet;
when it comes, the cursor lives in the collector's or the flow's state in
TinyStore, not in the plugin's memory.

## The network an instance may reach

**A plugin declares the hosts it talks to**, `hosts('api.stripe.com')`, or
`hosts('*')` for any public host. An instance may then reach those, plus the
hosts an admin typed into its `url` settings. Private and loopback addresses
are reachable only through a host typed into a setting; link-local and cloud
metadata never. Every redirect hop is checked. All of this is measured in
[the plugin host](../reports/plugin-host-2026-10-25.md): two instances of one
plugin in one process, one allowed and one refused.

**The host keeps the refusals it made during a call** and reports them beside
the plugin's error. An SDK catches what the host throws: Stripe's turned
"dashbin: plughost-stripe is not a host this plugin may reach" into "An error
occurred with our connection to Stripe", after retrying twice.

## Executors

| Executor | For | Cost, measured |
|---|---|---|
| **built-in**, Rust in the binary | databases, Dashbin's own data, scraping, probes, the first notifications | nothing beyond the binary |
| **QuickJS** in the hub's process, the default for JS | anything built on `fetch`: Stripe, Octokit, OpenAI, Notion | 3.4 MiB for the web APIs once; per loaded instance 0.2 MiB empty, Stripe +2.8, OpenAI +4.8, Notion +7.0 |
| **a process**, any language, over stdin and stdout | TCP drivers, Node-only SDKs, Python, heavy computation, streams | Bun 22–41 MiB, Node 44–93, Python its own |
| **in the browser** | visualizations | the page's, not the hub's |

**One QuickJS runtime per instance**, never shared between instances: a
module may keep a client holding its instance's key, and a second account in
the same runtime would inherit it. A runtime loads on its instance's first
call, runs one call at a time (chapter 5), has a memory limit, and is
unloaded after some minutes idle.

**The host compiles a bundle to bytecode at install** and keeps it as a
cache, recompiled when Dashbin upgrades; it never accepts bytecode from an
author. Bytecode saves the cold start (OpenAI 30 → 4 ms), not memory.

**LLRT's crypto is its `crypto-ring` backend.** Its `crypto-rust` pins
pre-releases of RustCrypto that cannot link beside TinyStore's `kv`, which
needs the released line; with `crypto-ring` both build into one binary.

**The host carries rquickjs-core 0.11.0 with one fix.** Unloading a runtime
after an interrupted job aborted the process in QuickJS's garbage collector,
because 0.11.0 frees a context it only borrowed; 0.12.0 fixed it, and LLRT
still requires 0.11. The fix sits in `vendor/` beside `llrt_fetch`, until
LLRT moves.

**A process plugin shares the agent's call protocol, not the agent.** What
both carry is the call: a request, its frames or result or error, and
`$cancel`, in JSON-RPC over the process's stdin and stdout, frames in chapter
3's wire. What only an agent has — dialling the hub, its token, the live
registry, reconnecting, local secrets — a process plugin neither has nor
needs. An MCP server is not the protocol; a bridge that exposes an MCP
server's read-only tools as reads can be a plugin later.

**The second consent for a process is not a sandbox.** A process plugin runs
as Dashbin's user and reaches whatever that user can, files included, until
the operating system isolates it; the admin is told so in those words.

## Memory, said plainly

QuickJS runs inside the hub's process, so a container's limit counts the hub
and every loaded JS instance as one number. Measured in a 64 MiB container
under load ([memory with plugins](../reports/plugin-memory-2026-10-25.md)):
the hub alone fits with room; eight instances of real SDKs, kept loaded the
simple way, did not; kept the way below, sixteen did, and with nothing loaded
the hub is 14 MiB. So the plugins' memory is kept this way:

- **QuickJS's heaps live in a jemalloc arena of their own that returns freed
  pages at once.** A growing heap leaves pages behind, and under the hub's
  one-second decay a runtime cost up to twice its limit for a moment; in its
  own arena a bomb installed beside eight loaded instances did not move the
  process at all. The rest of the hub keeps its decay and its speed; plugin
  calls pay about 11%.
- **A runtime's garbage is collected after each call.** QuickJS otherwise
  lets a heap grow by half before collecting, and under load the heaps grew
  by a third; collected after each call they stay at their loaded size, for
  about 8% of a call's speed.
- **An idle runtime is unloaded after a few seconds** and reloaded from
  bytecode when called, in 7–19 ms. The bytecode is one file a plugin, read
  at each load: it is 1.2–3.6 × its source, and kept in memory for every
  instance it was what killed the first attempt.
- **At most a few runtimes are loaded at once**, a number the admin sets
  against the container's limit; four held sixteen instances in 64 MiB. A
  call that finds no slot takes the one idle longest, or waits; the wait is
  the price, up to 2 s at the 99th percentile with eight busy instances. Each
  slot costs what its largest plugin holds: Octokit 1.4 MiB, Stripe 2.3,
  OpenAI 3.6, Notion 5.1.
- **An install's limit is the plugin's own**, about 1.5 × what it holds:
  2 MiB for Octokit, 8 for Notion. A runtime that fails at its limit is
  reported as out of memory by the host, since it cannot always build its own
  error.
- The interface shows the base, each loaded instance and the process's total.

A process plugin is a process of its own: its memory is shown beside the
hub's and is not the hub's.

## A package

```
stripe/
  package.json          name, version, "dashbin": { "runtime": "quickjs" }
  dist/server.js        one ES module, the plugin's code and its SDKs bundled
  dist/browser.js       optional: visualizations, as custom elements
  locales/ru.json       optional: titles in another language
  README.md
```

The package's name is the plugin's id. `dashbin` in `package.json` says only
what cannot be read from the code: which executor runs it, and for a process,
its command. `dashbin plugin build` is a thin wrapper over a bundler with the
right target: one ES module, no Node built-ins, the `dashbin` module left to
the host, and a smoke run in Dashbin's own runtime before it says done.

In the first version a plugin is installed from a file or a URL. A registry
comes when there are plugins to register.

## Trust, install, upgrade

**Installing is trusting**, as in Grafana, Home Assistant and n8n: the admin
sees what the plugin declares and accepts it.

| The admin is shown | Because |
|---|---|
| the functions, by kind | an `action` changes things outside, and a kind is only the author's word |
| the settings, secrets marked | the code sees them |
| the hosts | where those secrets can be sent |
| the executor | a process plugin is native code, running as Dashbin's user, and needs a second, separate consent |

**Every upgrade is the admin's act, in the first version.** Nothing updates
itself. An admin installs a new version the way they installed the first,
and is shown what its declarations change: functions, settings, hosts,
executor. That list is information, not proof: a version asking for exactly
the same hosts and secrets can do something new with them, so the same
permissions never mean an upgrade is accepted without the admin. Automatic
updates from a publisher the admin chose to trust can come later, by that
explicit choice.

For every installed version the hub keeps its number, the SHA-256 of the
package, where it came from, and who installed it when. The previous version
stays on disk, so an upgrade can be undone. A function that disappears while
a saved query still names it refuses the upgrade, unless the new version
names it as an alias. A new required setting leaves the instances waiting
for it, marked, rather than failing at their next call.

## Actions and their id

**An action's `id` belongs to the operation, not to an attempt.** A flow's
step that TinyStore's jobs run again after a crash keeps the id it had, so
the provider sees one operation twice and does it once. That holds only
inside the provider's own window: Stripe keeps an idempotency key for at
least 24 hours and may prune it after, and a retry past that is a new
refund. So a step whose outcome is unknown is not retried blindly once the
window may have passed; it is reconciled, or handed to a person. How is
chapter 7's, and it is the same for Telegram, GitHub or any action outside.

## Instances

An admin makes an instance: a name, the settings' values, labels, and where
it runs. Secrets are sealed, as Go Dashbin sealed source credentials, with
the key beside the database. Two accounts of one service are two instances.

## On an agent

**An agent runs only what its operator allowed, in its own configuration**:
built-ins by name, JS bundles by their hash, and which kinds — reads only,
unless the operator writes `action` too. The settings and secrets of an
instance placed on an agent stay in that agent's configuration, as in Go
Dashbin, and the hub knows its name, plugin and labels and nothing under
them. A bundle the hub offers whose hash the operator did not pin does not
run. The configuration file's format is chapter 12's.

## Visualizations

A plugin may carry visualizations: custom elements in its browser bundle,
each given frames and options and drawing them. How a page loads them, what
they may reach, and when they live in an iframe instead is chapter 9's; this
chapter only says where they come from.

## Built-ins in the first version

| Plugin | Reads | Actions |
|---|---|---|
| `postgres`, `mysql` | `sql`, with Go Dashbin's mappings and read-only rules | — |
| `datasets` | `series`, `events` (chapter 5) | — |
| `scrape` | `metrics`: Prometheus text, into TinyStore's reader | — |
| `probe` | `http`: a synthetic check, status and time | — |
| `notify` | — | `telegram`, `webhook` |

A built-in declares settings and functions in the same shape as a JS plugin,
through a Rust trait, so the interface treats both alike.

## Against three scenarios

### Stripe, written by someone else

The example above, built with `dashbin plugin build`, uploaded. The admin
sees two functions (a read and an action), one secret, `api.stripe.com`, and
QuickJS; accepts. Two instances, `stripe-eu` and `stripe-us`, each with its
key: two runtimes, 2.3 MiB of JavaScript each while loaded, inside the budget for loaded
runtimes. An alert reads `charges` on `all`; Dmitry changes the range while a
page of charges is loading, and the host aborts that request although the
SDK was never given a signal. A button calls `refund`; its flow step keeps
the operation's id across a retry, so Stripe refunds once within its window.
Version 1.0.1 arrives with the same hosts and the same secret: the admin
still installs it, sees that nothing they are asked to grant changed, and
can roll back.

### Postgres beside a closed database

The built-in `postgres`, placed on the agent of site 3. The operator's
configuration names `postgres` with the database's host and password and
allows reads. The hub stores `pg-site3`, its plugin and `site=kazan`, and
never the password.

### Slack, which QuickJS cannot run

`@slack/web-api` needs Node. Its author ships a package with
`"runtime": "process"` and a command that runs Bun. The admin is asked twice:
for the plugin, and for native code that can read what Dashbin's user can. It
costs 30-odd MiB while running, shown beside the hub rather than inside it.

## Forks

After the review:

1. **A function declares its own parameters** in code, so the query editor
   has a form. Kept.
2. **A plugin declares its hosts.** Kept; the example now does.
3. **No store per instance** — kept, and said plainly: reads by range only,
   no general cursor sync; a cursor later lives in a collector's or flow's
   state in TinyStore.
4. **Memory** — changed: one process, one number; measured, then kept by
   four rules: QuickJS in an arena that returns pages at once, garbage
   collected after a call, idle runtimes unloaded with their bytecode on disk,
   and a few slots for loaded runtimes.
5. **Process plugins** — changed: they share the agent's call protocol, not
   the agent; the second consent is not a sandbox.
6. **Install from a file or a URL.** Kept.
7. **Upgrades** — changed: every upgrade is the admin's act; unchanged
   permissions are information, not proof; version, hash, source and author
   of the install are kept, and the previous version for rollback.

Added: installing runs in a limited runtime with no settings; the host aborts
a call's requests whatever the SDK does; secrets are masked, not sealed off;
a kind is the author's word; an action's id belongs to the operation, inside
the provider's window.

## What it asks of other chapters

- Chapter 7: what a webhook's rows become; a flow step keeps an action's id
  across attempts and reconciles an unknown outcome past the provider's
  window.
- Chapter 8: the agent's configuration: pinned hashes, allowed kinds, local
  secrets.
- Chapter 9: visualizations as custom elements, and their isolation.
- Chapter 10: who may install and upgrade, and the second consent for native
  code.
- Chapter 12: the load profile behind 64 MiB, the budget for loaded
  runtimes, unloading idle ones, and the configuration file.

## Not measured yet

- A thread cache of the plugin thread's own in QuickJS's arena, which may
  take back some of the 11% the arena costs plugin calls.
- The hub's own cost of the QuickJS host, which shares tokio and hyper with
  it: the 9.19 MiB was a separate binary.
- A process plugin over stdio end to end, and its cancellation.
- How long an idle runtime should stay loaded, and how many slots a given
  container wants, on dashboards rather than on a synthetic load.
