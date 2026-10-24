# Memory of a Rust Dashbin, and its allocator, 24 October 2026

The Rust rewrite promised a small process. Before designing it, a prototype
of each part was measured: a hub on axum with `tinystore::aio` and five
engines, QuickJS instances for plugins, a plugin as a Bun or Node process,
and an agent with no web part. Then the same programs under four allocators,
on one, two and eight cores, and under container memory limits.

| Question | Answer | What follows |
|---|---|---|
| What does the hub cost? | 9.4 MiB idle with five engines open, about 22 MiB after heavy load and flat from there (6 × 1000 panels: 20.1 → 21.8) | room for a 64 MiB promise |
| What does a plugin cost in-process? | QuickJS: 1 MiB for the first instance, about 0.2 MiB for each further one | third-party JS runs in QuickJS by default |
| What does it cost as a process? | Bun 22 MiB idle, 36 after work; Node 44 and 93 | a process plugin is an escape hatch, its memory shown |
| How slow is QuickJS? | 12.8 ms for the work that takes Bun 2.1 ms and Node 4.1 | heavy computation belongs in built-in Rust |
| What does an agent cost? | 4.6 MiB binary (the Go agent was 11.4), 5 MiB connected, 12 after work on glibc | the agent is the same crate built without the web |
| Does freed memory come back? | Not on glibc: 100 unloaded QuickJS instances keep 22.4 MiB until `malloc_trim` | the allocator matters more than the code |
| Which allocator? | musl's own: least memory, gives back at once, 2.4–8.9× slower under concurrency; jemalloc: glibc's speed, back within a minute | see the decision below |
| Is mimalloc a candidate? | Not as it comes: with THP on `madvise` the hub grew to 176 MiB; 35.8 with THP off, keeping it | rejected |
| Does tuning jemalloc help? | `narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0`: same speed on 1, 2 and 8 cores, peak 42.7 → 30.6 MiB on two | compiled into the hub |
| Does it fit a container? | 32 MiB: the default jemalloc was OOM-killed, the tuned one and musl lived at the limit; 64 MiB: no pressure | promise 64, not 32 |

## Environment and reproduction

- Docker Desktop 29.6.2, kernel 6.18.33.2-microsoft-standard-WSL2, 16 vCPU,
  15.2 GiB, Ryzen 7 7700; transparent huge pages `madvise`.
- `rust:1.99-bookworm@sha256:e12566d5…`; the musl image adds `musl-tools`
  and `cmake` to it (`memory-probe/docker/Dockerfile`).
- Crates as locked in `memory-probe/Cargo.lock`: tinystore 0.1.1, axum 0.8,
  rquickjs 0.14.0, tokio-tungstenite 0.30, tokio-postgres 0.7, mysql_async 0.37,
  mimalloc 0.1.52 with `override`, tikv-jemallocator 0.7 (jemalloc 5.3.1) with
  `unprefixed_malloc_on_supported_platforms`. Both allocators replace `malloc`
  for the whole process, SQLite's and QuickJS's C included: mimalloc's own
  statistics counted QuickJS's 52 MiB, jemalloc read `MALLOC_CONF`.
- `oven/bun:1.4.2-slim@sha256:cb3bbbb0…`, `node:24-slim@sha256:0e0ff40c…`
  (Node 24.21.0), `postgres:18-alpine@sha256:77f58511…`, `mysql:9@sha256:29abb0a1…`.
- Release builds with fat LTO, one codegen unit, stripped. `LIBSQLITE3_FLAGS`
  as TinyStore 0.1.1 recommends.
- Memory is `VmRSS`, `RssAnon`, `RssFile` and `VmHWM` from `/proc/self/status`
  of the measured process; cgroup counters from `/sys/fs/cgroup/memory.*`.
  Speed is `memory-probe/src/load.rs`, ten keep-alive connections, run in a
  container sharing the hub's network namespace and pinned to other cores
  from 05 on.
- The harness was not committed before it ran: it lived in a session's
  scratchpad and is committed afterwards. Runs 01–03 used an earlier state of
  `src/` without `malloc_trim` checkpoints or allocator features; `src/` here
  is the final state, which 04–08 ran.
- One pass a configuration unless a table shows two; 01 and 02 repeated each
  other within 1.5 MiB.

```sh
# 01-03: glibc, one binary each in /target/out
MSYS_NO_PATHCONV=1 docker run --rm -v <research>/dashbin/memory-probe:/src -v rssprobe-cargo:/usr/local/cargo/registry \
  -v rssprobe-target:/target -e CARGO_TARGET_DIR=/target \
  -e LIBSQLITE3_FLAGS="SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT SQLITE_ENABLE_DBPAGE_VTAB -DHAVE_FDATASYNC=1" \
  -w /src rust:1.99-bookworm bash -c 'for p in bare:bare hub:hub qjs:qjs agent:agent buffer:agent-buffer; do \
  cargo build --release --features ${p%%:*} --bin ${p##*:}; done; mkdir -p /target/out; cp /target/release/{bare,hub,qjs,agent,agent-buffer} /target/out/'
cd <research>/dashbin/memory-probe && bash measure.sh && bash plateau.sh

# 04-08: glibc, musl, musl+mimalloc, musl+jemalloc into /target/out/<variant>
docker build -t rssprobe-musl docker
MSYS_NO_PATHCONV=1 docker run --rm -v <research>/dashbin/memory-probe:/src -v rssprobe-cargo:/usr/local/cargo/registry \
  -v rssprobe-target:/target -e CARGO_TARGET_DIR=/target -w /src rssprobe-musl bash build.sh
bash measure2.sh   # 04 five allocators
bash measure3.sh   # 05 one and two cores
bash measure4.sh   # 06 tuning, agent, container limits
bash measure5.sh   # 07 decay against arenas, eight cores
```

Raw output: [data/memory-and-allocators-2026-10-24/](data/memory-and-allocators-2026-10-24/), one file a run, numbered as above.

## The first pass, glibc (01, 02, 03)

| | binary | idle | after load |
|---|---|---|---|
| bare axum | 0.92 MiB | 3.2 | 3.8 |
| hub: axum + `tinystore::aio`, five engines | 6.72 MiB | 9.4; 9.4 | 21.2; 22.6 |
| agent: socket + Postgres + MySQL + QuickJS | 4.56 MiB | 5.1; 5.4 | 12.3; 12.4 |
| agent + TinyStore metrics as a buffer | 8.03 MiB | 5.3; 5.6 | 16.2; 16.7 |
| Bun 1.4.2, plugin as a process | — | 22.4 | 50.8, 36.4 after GC |
| Node 24, plugin as a process | — | 43.6 | 99.0, 92.8 after GC |

QuickJS: linked and idle 2.5 MiB, one instance with the plugin 3.5, after one
call 5.4; 10 instances 7.5, 100 instances 24.3, so about 0.19 MiB an instance
after a call and a GC (QuickJS's own count: 0.1). The plugin's work is the same
in every runtime: a JSON answer of 10 000 objects parsed into two columns.
100 calls: QuickJS 1.28 s, Bun 210 ms, Node 408 ms.

Tokio's worker count (16 against 2) moved the hub by under 1 MiB.

## glibc keeps what it frees (02)

Dropping all 100 QuickJS instances left the process at 22.4 MiB;
`malloc_trim(0)` took it to 4.3. The agent after its rounds: 12.3 → 7.7 with
`malloc_trim`. Unloading idle plugins saves nothing on glibc without a trim.

## Five allocators (04)

Hub, nothing pinned, all 16 cores visible:

| | binary | idle | after 12 000 pages + 6000 panels | 90 s later | pages/s | panels/s |
|---|---|---|---|---|---|---|
| glibc, dynamic | 6.73 MiB | 9.4 | 28.1 | 28.4 (trim 25.3) | 20 487 | 3 637; 3 736 |
| musl, its mallocng | 6.86 MiB | 7.1 | 14.1 | 14.9 | 8 396 | 471; 464 |
| musl + mimalloc | 7.02 MiB | 21.5 | 160.4 | 165.1 | 20 423 | 3 484; 3 602 |
| musl + jemalloc | 7.33 MiB | 10.0 | 37.3 | 27.3 | 20 022 | 3 518; 3 586 |
| musl + jemalloc, `background_thread:true` | 7.33 MiB | 10.0 | 39.1 | 25.0 (27.5 at 30 s) | 19 961 | 3 553; 3 639 |

QuickJS, 100 instances unloaded: musl gave everything back at once
(21.6 → 1.5); jemalloc with a background thread gave back half in 12 s
(24.8 → 11.0); glibc, mimalloc and plain jemalloc kept it until asked
(`malloc_trim`, `mi_collect`, `arena.4096.purge`: 2–4 MiB). 100 calls: glibc
1.31 s, jemalloc 1.67, mimalloc 1.70, musl 1.92. Agent after 100 rounds:
musl 5.0 MiB, jemalloc 8.5 ten seconds later, glibc 12.9, mimalloc 20.

## mimalloc and huge pages (08)

With `MIMALLOC_ALLOW_THP=0` the same hub stayed at 35.8 MiB after the load
instead of 176, and one QuickJS instance after a call was 6.0 MiB instead of
17.5. The inflation is transparent huge pages; the fix is a setting, and
mimalloc still kept 35.8 MiB after 30 s and `mi_collect(true)` returned 2 MiB
of it, since it collects the calling thread's heap.

## A small VPS (05)

The hub pinned with `--cpuset-cpus`, `nproc` inside 2 or 1, the load client on
other cores:

| | pages/s | panels/s | panel p50 | idle | peak | 60 s later | agent 10 s later |
|---|---|---|---|---|---|---|---|
| 2 cores, musl | 7 845 | 570; 567 | 17.3 ms | 7.0 | 14.7 | 12.3 | 5.0 |
| 2 cores, jemalloc + background thread | 14 406 | 1 275; 1 264 | 5.7 ms | 8.3 | 47.5 | 22.9 | 9.0 |
| 1 core, musl | 6 834 | 525; 517 | 9.8–11.8 ms | 7.0 | 14.1 | 12.4 | 4.9 |
| 1 core, jemalloc + background thread | 9 077 | 678; 677 | 8.1 ms | 7.9 | 27.8 | 22.1 | 8.0 |

On one core musl's median is lower than on two while its p99 is 80 ms against
30: ten connections over 520 and 570 panels a second are 19 and 17.5 ms on
average, so only the distribution's shape changed.

## Tuning jemalloc, and a container's limit (06, 07)

Panels a second and peak `VmHWM`; `GPT` is `background_thread:true,narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0`:

| | 1 core | 2 cores | 8 cores |
|---|---|---|---|
| musl | 511, 14.2 MiB | 590, 14.9 MiB | 401, 16.1 MiB |
| jemalloc, `background_thread:true` | 680, 28.6 MiB | 1 270, 42.7 MiB | 3 555, 39.0 MiB |
| the same + decay only | — | 1 283, 33.7 MiB | 3 483, 34.8 MiB |
| `GPT` | 667, 29.2 MiB | 1 274, 30.6 MiB | 3 547, 28.4 MiB |
| `narenas:1`, decay 0 | 570, 19.8 MiB | 1 036, 20.3 MiB | — |

Thirty seconds after the load every jemalloc setting sat at 20–26 MiB and
musl at 11–13. The decay settings cut most of the peak and two arenas the rest,
at no cost in speed on eight cores.

Under `--memory=32m` on two cores, jemalloc with only a background thread was
OOM-killed during the panels; `GPT`, decay-only and musl lived at the limit
(`memory.events max` 125–222) with pages falling from 13–14 thousand to 4.3–5.7
thousand a second while the kernel reclaimed the page cache. At 48 MiB the
default jemalloc came within 3 MiB of the limit; at 64 MiB `GPT` met no
pressure. Without a limit `memory.current` read 50–100 MiB: SQLite's files in
the page cache, which the kernel takes back under a limit.

The agent on jemalloc: 8.0 MiB with a background thread, 6.3 with both decays
at 0, 5.9 with one arena as well, against 5.0 on musl; speed the same, since
the agent waits on databases.

## Decision

Dmitry left the allocator to the measurements. One file for hub and agent:
static musl with jemalloc and the `GPT` settings compiled in; the agent mode
sets both decays to 0 at start through `mallctl`. It costs the agent about
1 MiB against musl and buys the hub glibc's speed, 2.2× musl's on two cores
and 8.9× on eight. Measured through `MALLOC_CONF`; the `mallctl` path is to be
checked when the real binary is built. The memory promise is 64 MiB; 32 is not
promised until a sign-in hashing a password is measured on top of the load.
