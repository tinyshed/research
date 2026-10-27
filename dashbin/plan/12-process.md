# 12. The running process

**Status: draft, first round, 27 October 2026.** One file and its allocator
were decided on 24 and 25 October by measurement; the rest is a draft, and
nothing in it is agreed until Dmitry says so. The choices a review should
look at are under *Forks*.

What runs on a server: the one binary, the verbs it takes, how it starts and
stops, where its settings come from, how it keeps inside its memory and its
disk, what it says about itself, how it is upgraded, and how it is shipped.
Chapters 4 to 10 each left something here.

## One binary, its verbs

**One file is the hub, the agent and the tools around them**, decided on 25
October: jemalloc with `narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0` and
its background thread for the hub; the agent mode sets the decay to zero at
start and costs about 6 MiB against 5 for a musl build
([memory and allocators](../reports/memory-and-allocators-2026-10-24.md)).

| Verb | What it does |
|---|---|
| (none), `serve` | the hub |
| `agent run --config <file>` | an agent beside someone's database (chapter 8) |
| `see`, `show`, `query` | a store, a dashboard or one question drawn in a terminal (chapters 9 and 11) |
| `backup <file>`, `restore <file> <dir>` | the store as one checked zip, and back (chapter 4) |
| `passwd <login>`, `owner <login>` | the way back in from the machine (chapter 10) |
| `status` | version, uptime, the store's files and sizes, agents connected, flows paused, memory |
| `config explain` | every setting, its value and where it came from |
| `plugin …` | a plugin written, tested, built, installed and upgraded (chapter 6) |
| `openapi` | the api's schema, which the browser's types come from (chapter 11) |
| `telemetry show`, `status`, `enable`, `disable` | below |
| `version` | the version, its commit, how it was built |

These are the verbs a server's operator meets; the whole map, nouns for
everything the api manages included, is chapter 11's. **An error's kind is the
exit code**, as in Go Dashbin, so a script can branch on it. A verb that needs the store takes the data directory and its `LOCK`;
one that needs the hub talks to it over its api with a personal token.

## Starting and stopping

```
start   configuration → the store → sql/dashbin.db, migrations checked → metrics, records
        → secrets → plugins, loaded on first use → the agents' door → flows' workers → the listener
stop    the listener refuses new requests → running calls cancelled (chapter 5) → workers stop
        claiming, their runs left to their leases → agents told the hub is going → the store closed
```

- **`dashbin.db` opens first** because everything else in the hub reads it;
  a backup no longer depends on the order, since TinyStore 0.4.0 copies the
  whole store at one moment (chapter 4).
- **Migrations run before anything serves**; a hub whose migrations fail does
  not listen, and says which.
- **The listener comes last**, so the first request finds every part in place.
- **Stopping never waits for a slow call to finish**: it is cancelled, and a
  flow's run that was cut is run again by its job, as chapter 7 makes safe.

## Settings

**Two kinds, kept apart**, both read through TinyStore's config so that the
same layers, the same `Secret` and the same errors serve both:

| Kind | Who sets it | Where it lives | Examples |
|---|---|---|---|
| the operator's | whoever runs the server | `dashbin.toml`, then `DASHBIN_*` over it | the address to listen on, TLS's certificate and key, the data directory, the encryption key's file, the proxies trusted for `X-Forwarded-Proto`, a memory limit when the container gives none, the disk's free space to keep, telemetry |
| the admin's | an admin, in the interface | live settings in the store, which reach the running hub without a restart | slots for loaded runtimes, the shortest refresh a dashboard may ask, how long history and the audit are kept, the log's level |

- **`config explain`** prints every setting with where it came from — the
  file, a variable, the store, the default — and masks a secret.
- **The agent reads its file without a store** (`tinystore::config::load`,
  chapter 8); it has no live settings.
- **A file readable by others is refused**, the agent's as Go Dashbin's was,
  and the hub's when it holds a secret.

## Memory

**The promise is 64 MiB for Dashbin without third-party plugins** (chapter
1). Four parts share it (chapter 4): TinyStore's engines, frames in flight,
loaded JavaScript runtimes, and the rest.

**The container's limit is read, not waited for**, as Go Dashbin's
`memlimit` did: at start the hub reads its cgroup's limit and sizes the three
bounded parts from it — TinyStore's `Options::memory`, the budget for frames
(chapter 5), the slots for runtimes (chapter 6) — so `-m 128m` means more
room and `-m 64m` the promise, with nobody setting three numbers. Without a
limit the hub sizes itself for 64 MiB, and the operator's setting says
otherwise. Below the smallest limit the load run proves, it refuses to start
and says what it needs. The proportions are the load run's to set.

**The disk keeps 1 GiB free unless told** (chapter 4); on a small server the
operator says less, and a write that would go past it fails by name, never as
a broken save.

## Logs

**TinyStore's logger, kept and printed.** It prints a line for a person at a
terminal and one JSON object a line for a collector, the same lines
TinyStore's other SDKs print, and it keeps them in `records.db` as the hub's
`log` stream, with a keep:

- **an admin reads the hub's own log in the interface**, through `datasets`,
  like any other events, which is where a person without SSH looks first;
- **the level is a live setting**: turning on `debug` for ten minutes needs no
  restart;
- **a secret never reaches a line**: TinyStore's console redacts the fields it
  is told are secret, Dashbin adds every setting a plugin declares secret, and
  a test writes each kind of secret through every logging path and fails on
  any that arrives — Go Dashbin's gate, kept as a gate;
- **one failure is one line**, logged by whatever stops it — the handler, the
  flow, the verb — and never again by every layer it passed, as in Go Dashbin.

## The hub measuring itself

TinyStore measures its commits, waits, files and memory into `metrics.db`
under `tinystore_` when the store is opened with `self_metrics`; the hub opens
it so, and adds its own under `dashbin_`: calls by executor and their time,
frames in flight against their budget, runtimes loaded, agents connected,
flows run and failed, sign-ins refused. A built-in dashboard, **Dashbin**,
draws them — the product watching itself through its own store — and the load
run reads the same numbers.

`/healthz` answers while the process is alive; `/readyz` once the store is
open, migrations have passed and the listener is up. Both say nothing else,
and neither needs a session.

## Upgrades

- **An upgrade is a new binary and a restart.** Migrations run at start;
  documents written by an older version are read and upgraded as they are
  read (chapter 4).
- **Before a migration the hub writes a backup** into `backups/` in its data
  directory, without the key, which stays beside it; the last two are kept. A
  version that has to be undone is the previous binary and that backup.
- **Agents are not upgraded with the hub**: the hub speaks the call
  protocol and the frame wire of its own version and the one before (chapter
  8), and an agent with nothing in common says so.
- **Plugins are untouched by an upgrade**; an official plugin in the binary
  is a new version of it, shown like any other (chapter 6).
- **Nothing upgrades itself**: no download, no update that reaches a server
  on its own.

## Telemetry

As Go Dashbin decided, and before the first public release, because consent
cannot be added afterwards:

- **Opt-in, counts only**: the installation's id, the version, the operating
  system and architecture, whether a container capped it, and counts —
  instances by plugin, dashboards, panels, flows, accounts, agents.
- **Never anything naming the user's world**: no query, result, host,
  database, table, dashboard's or panel's name, URL, login or label. That is
  kept by construction: the telemetry reads through an interface that returns
  numbers and a fixed set of kinds, so there is no name in scope to leak, and
  a test reads the assembled document's fields.
- **One snapshot a day**, sent by a task with a two-second timeout whose
  failure is a debug line, so a server of ours being down never slows a hub.
- **`dashbin telemetry show` prints the exact document** that would be sent;
  `status`, `enable` and `disable` do the obvious; the id can be reset, which
  is the honest way to say it is a pseudonym.

## Shipping

- **Static musl binaries** for Linux on x86-64 and ARM64, which is what a
  small server runs; macOS and Windows builds for trying it and for agents.
- **A container image `FROM scratch`** with the binary alone, migrations and
  the web inside it. It has no system certificates, so the binary carries
  Mozilla's roots and uses them when the system has none.
- **systemd units** for the hub and the agent, each with its own user, its
  data directory the only place it may write, and nothing else it may reach on
  the disk.
- **A recipe for Cloudflare Tunnel** in front of a hub with no public address
  (chapter 8), and one for Caddy in front of a hub that has one; the hub's own
  TLS for neither.
- **One port**: the browser, the api, the agents' door `/agent` and the
  webhooks' `/hooks/` share it.

## Against three scenarios

### A 64 MiB container

`docker run -m 64m` with a volume for the data. The hub reads 64 MiB from its
cgroup, sizes TinyStore's work, the frames' budget and two runtime slots for
it, opens `dashbin.db` first, migrates, and listens. A dashboard of ten sites
and a collector every 15 s run inside it; the built-in **Dashbin** dashboard
shows how near the budgets they come.

### An upgrade that goes wrong

The operator replaces the binary with a newer one. Before its migration the
hub writes `backups/pre-0.2.0.zip`, migrates, and a bug shows. The operator
stops it, puts the previous binary back, restores the backup into a new
directory and starts it there; the ten agents, a version behind the new hub
and the same as the old one, connect to either.

### A night with `debug`

An admin turns the log's level to `debug` in the interface for an hour while
a flow misbehaves, reads the lines in the hub's own log there, and turns it
back. No restart, no SSH, and no password among the lines, because the test
that would have caught one ran on every commit.

## Forks

1. **One binary with verbs, the hub when none is given** — decided on 25
   October.
2. **The operator's settings in a file and the environment, the admin's live
   in the store** — against everything in the file, which makes every tuning a
   restart and every admin an operator.
3. **Budgets sized from the container's limit at start** — against fixed
   numbers an operator has to know to change.
4. **Logs kept in Records and read in the interface, their level live** —
   against standard error only, which leaves an admin without SSH blind.
5. **A backup before every migration, the last two kept** — against leaving
   it to the operator, who learns of it after the first bad upgrade.
6. **Telemetry as Go Dashbin decided**: opt-in counts, a daily snapshot,
   nothing that names the user's world.
7. **A container from scratch, with Mozilla's roots in the binary as a
   fallback** — against a base image with a system, which adds a second thing
   to patch.

## What it asks of other chapters

- Chapter 4: the backups before a migration.
- Chapter 5: the budget for frames sized from the container's limit.
- Chapter 6: the slots for loaded runtimes as a live setting.
- Chapter 10: who may change a live setting and read the log; the log's
  redaction among the gates.
- Chapter 11: `/healthz`, `/readyz`, and the api the verbs that need the hub
  call.
- Chapter 13: the logging gate, the telemetry's field test, the build flags
  TinyStore recommends, and the targets.

## Not checked yet

- The load run against 64 MiB, and the proportions of the four parts it sets.
- The binary's size with everything in it: the web, QuickJS and LLRT's
  modules, rustls and Mozilla's roots, the terminal's drawing.
- How long a hub takes to start on a store holding a month of ten sites, and
  how long the backup before a migration takes there.
- TinyStore's live settings reaching a running hub: how soon, and what a
  change of the runtimes' slots does to runtimes already loaded.
