# Memory with plugins, 25 October 2026

The plugin round left one number wrong-footed: a memory bomb installed under a
24 MiB JavaScript limit took the process to 54 MiB. This round asked four
questions in order: what an install limit holds the process to, what real
SDKs need, what a spike inside a call costs, and whether the hub with its
plugin instances fits the 64 MiB promise in one process under load. The last
answer was no, so it then measured four fixes, added one at a time.

| Question | Answer | What follows |
|---|---|---|
| Why did a 24 MiB limit cost 54? | jemalloc keeps the pages a growing heap freed: with the hub's `dirty_decay_ms:1000` a bomb costs about twice its limit plus the runtime, with `dirty_decay_ms:0` about its limit | a limit bounds JavaScript; the allocator bounds the rest |
| What do real SDKs need? | Octokit 2 MiB, Stripe 3, OpenAI 6, Notion 8; they hold 1.4, 2.3, 3.6 and 5.1 MiB once loaded, the install the same as the instance | a limit of about 1.5 × what a plugin holds, per plugin, measured at install |
| A spike inside a call? | The same as at install, and the runtime answers its next call | an out-of-memory call is a failed call, not a lost instance |
| The hub alone, 64 MiB? | Fits: 23 MiB after load, 33 at the peak, and it survived a bomb installed under an 8 MiB limit | the promise holds for Dashbin without plugins |
| The hub with eight real instances, 64 MiB? | **Does not hold.** 40 MiB idle, the container at its limit under load (95 reclaims), and a bomb installed after the load got the process OOM-killed | without the fixes below, the room is about four such instances, not eight; the earlier "eight" was wrong |
| And with pages returned at once? | It survived the load and the bomb, at 21–33% less throughput | a cost the whole hub should not pay for its plugins' heaps |
| QuickJS's heaps in an arena of their own that returns pages at once? | A bomb installed beside eight loaded instances no longer moves the RSS (58.1 → 58.2 MiB, against 58.0 → 71.6); the hub keeps its speed, plugin calls lose 11% | taken |
| Garbage collected after every call? | The heaps stay at their loaded size under load, 24.8 MiB instead of 33.2, and the RSS 10 MiB lower; plugin calls lose about 8% more | taken |
| Idle runtimes unloaded, bytecode a file a plugin? | 14 MiB with nothing loaded instead of 40, back to 24 three seconds after the load; a runtime reloads in 7–9 ms, 13–19 under load | taken; bytecode is 1.2–3.6 × its source and never sits in memory per instance |
| At most four runtimes loaded at once? | **Sixteen instances under the full load fit in 64 MiB**: 39 MiB after it, nine reclaims, the bomb harmless | taken; the price is the wait of an instance that finds no slot, p99 up to 2 s with eight busy instances |
| Unloaded after every call? | Memory stays low and plugin calls fall to 13 req/s an instance, 140 ms each | rejected under load |

## Environment and reproduction

- The plugin host of [the plugin calls round](plugin-calls-2026-10-25.md) with
  a `hub` feature: axum 0.8 and `tinystore` 0.1.1 (`async`, `sql`, `kv`,
  `metrics`, `records`, `jobs`) in the same binary, the plugin instances on a
  thread of their own; `rquickjs-core` 0.11.0 with the context fix; static
  musl, jemalloc with `background_thread:true,narenas:2,muzzy_decay_ms:0` and
  `dirty_decay_ms` 1000 or 0 as each table says.
- LLRT's crypto backend is `crypto-ring` in this round, not `crypto-rust`:
  TinyStore's `kv` needs `chacha20poly1305` 0.11 on RustCrypto's released 0.6
  line, LLRT 0.8.1-beta's `crypto-rust` pins its pre-releases, and the two do
  not compile into one binary. With `crypto-ring` and the RustCrypto set moved
  together (`crypto-common` 0.2.2, `cipher` 0.5.2, `digest` 0.11.3 and their
  neighbours) both build.
- SDKs as locked in `plugin-host/plugins/bun.lock`: stripe 23.0.1, openai
  7.43.0, @notionhq/client 5.28.0, @octokit/rest 22.0.1, each wrapped in
  chapter 6's shape (`plugins/*.js`) and bundled for a worker;
  `stripe/stripe-mock@sha256:aeb96b83…` and `sdk-probe/mock.mjs` in Bun 1.4.2
  answer them.
- Parts 1–3: one process a line, so a peak is that line's; "over the start"
  is VmHWM minus the VmRSS the process began with, about 3 MiB. Part 4: the
  hub in Docker with `--memory=64m --memory-swap=64m` on two cores, the load
  from `memory-probe`'s client on four others, the mocks on their own cores.
- Docker Desktop on WSL2, 16 vCPU, one pass a configuration; the hub's own
  throughput varied by about ±10% between runs of the same setting (2 044 to
  2 618 pages a second with the same knobs), so differences smaller than that
  are not claimed. The harness was committed before each run: 73f72e0,
  2245adf, 4e89026, 14e2265, 15a1675.
- The fixes are knobs of the same binary: `JS_ARENA` (QuickJS allocates
  through `mallocx` in a jemalloc arena created at start with both decays at
  zero, no thread cache), `GC_AFTER_CALL`, `IDLE_UNLOAD_MS`, `MAX_LOADED`
  (`plugin-host/src/arena.rs`, `src/hub.rs`). Before each set of runs a
  throwaway container reads the binary once, so the first hub is not charged
  the page cache of a freshly built binary and the others not.
- `03-fixes-keeping-every-source.txt` is a run stopped when its harness was
  found keeping every bundle's source in memory, 5 MiB the hub of part 4 did
  not hold; only its first variant is a measurement, and its later `DIED`
  lines are containers removed while stopping it.

```sh
cd <research>/dashbin/plugin-host
bash memory.sh   # parts 1-3
bash onebox.sh   # part 4
bash fixes.sh    # part 5, the fixes one at a time
bash budget.sh   # part 5, unloading and a budget
```

Raw output: [data/plugin-memory-2026-10-25/](data/plugin-memory-2026-10-25/).

## 1. Install limits against the process's peak

Each line installed one module under a limit. A module that only declares
cost 4.8 MiB over the start at every limit: the runtime and its web APIs.
The memory bomb, peak over the start, MiB:

| JavaScript limit | `dirty_decay_ms:1000` | `dirty_decay_ms:0` |
|---|---|---|
| 4 MiB | 12.4 | 8.7 |
| 8 MiB | 24.6 | 14.0 |
| 16 MiB | 35.6 | 18.9 |
| 24 MiB | 51.4 | 30.2 |

The limit held every time: `out of memory` in 9–52 ms. What it did not hold
is the process. A heap that grows by reallocation leaves the pages it moved
from behind, and jemalloc keeps them until its decay returns them; with the
hub's one-second decay that is about one more limit's worth, gone within two
seconds, and with a decay of zero it is nearly nothing. So a runtime may cost
up to twice its limit for a moment, and only the allocator decides how much
of that moment the container sees.

## 2. What real SDKs need

Each SDK installed and answered one call under the same limit, from 2 to
16 MiB:

| Plugin | Smallest limit that installs and answers | JavaScript held once loaded | Process RSS over the start |
|---|---|---|---|
| Octokit, `repos.get` | 2 MiB | 1.4 MiB | 7.0 MiB |
| Stripe, `charges.list` | 3 MiB | 2.3 MiB | 9.4 MiB |
| OpenAI, a completion | 6 MiB | 3.6 MiB | 12.5 MiB |
| Notion, a search | 8 MiB | 5.1 MiB | 15.6 MiB |

The install and the instance passed and failed at the same limits, and a
call added 0.1 MiB at most. Under a limit too small, the refusal did not
always say so: OpenAI and Notion failed as `Exception generated by quickjs:
Null"null"` or an empty `Error:`, because there was no memory left to build
the error. **The host says "out of memory" itself** when a runtime fails at
its limit, rather than passing on what the runtime managed to throw.

## 3. A spike inside a call

A read that allocates until its runtime runs out, under an instance's limit,
peak over the start, MiB:

| JavaScript limit | `dirty_decay_ms:1000` | `dirty_decay_ms:0` |
|---|---|---|
| 8 MiB | 24.4 | 13.9 |
| 16 MiB | 35.3 | 18.6 |
| 32 MiB | 51.3 | 34.9 |

The same shape as installing. The call failed with `out of memory` in
14–61 ms, the runtime was idle at once, and its next call returned: an
instance survives its own bomb.

## 4. The hub and its instances in one process

The hub as the memory round of 24 October ran it — sessions in KV, a listing
in SQL, an audit event, a log line, panels over an hour of metrics — with
plugin instances loaded at its start, each instance one runtime at a 32 MiB
limit, all of them on one plugin thread. One call each, then everything at
once: 10 000 page loads, 3 000 panels and 400 calls on every instance; then a
memory bomb installed under an 8 MiB limit, then Stripe installed.

| | no plugins, 64 MiB | 8 instances, no limit | 8 instances, 64 MiB | 8 instances, 64 MiB, decay 0 | 16 instances, 64 MiB |
|---|---|---|---|---|---|
| idle, instances loaded | 9.9 MiB | 39.9 | 40.0 | 39.6 | 64.5 |
| JavaScript held | — | 24.7 MiB | 24.7 | 24.7 | 49.5 |
| after the load, RSS (peak) | 23.0 (32.9) | 60.5 (70.6) | 60.6 (70.1) | 57.2 (63.4) | **OOM-killed** |
| JavaScript held after the load | — | 33.2 MiB | 33.2 | 33.2 | — |
| cgroup peak, times at its limit | 51 MiB, 0 | 81, — | 64, 95 | 64, 71 | — |
| pages, panels, req/s | 3 133, 1 124 | 2 282, 889 | 2 378, 935 | 1 604, 739 | — |
| plugin calls, req/s per instance | — | 81–89 | 89–99 | 78–84 | — |
| the bomb installed | refused, survived | refused, RSS 71.8 | **OOM-killed** | refused, survived | — |
| Stripe installed after it | declared | declared | — | declared | — |

What the table says:

- **Dashbin without plugins keeps the promise**, with room: 23 MiB after the
  load, 33 at its peak, and a bomb installed under an 8 MiB limit did not
  reach the container's limit.
- **Eight real instances already sit at 40 MiB idle.** Their JavaScript holds
  24.7 MiB, the sum of what part 2 measured one by one, and grows under load
  to 33.2 MiB — garbage QuickJS has not collected yet. With the hub's own load
  on top, the container met its limit 95 times and survived by reclaiming;
  the first spike after it, the bomb's install, killed it.
- **Returning pages at once saved it**, and cost 21–33% of the hub's
  throughput and 12–15% of the plugins'. That price is paid by every
  allocation in the process, for the sake of the plugins' heaps.
- **Sixteen instances do not fit at all**: 64.5 MiB before anything ran.

The "about eight active plugins beside a loaded hub" of
[the plugin host round](plugin-host-2026-10-25.md) was wrong: it added up
instances measured one at a time, with no hub load and no spike. Measured
together, 64 MiB holds the hub and roughly 20 MiB of loaded JavaScript with a
spike's reserve beside it — about four instances of Stripe's or OpenAI's
size, without the fixes of part 5.

The load client has no read timeout. When the sixteen-instance hub was
killed, one client container hung on its dead connection and was removed by
hand so the script could finish; the hub's death is in its exit status, not
in the client's.

## 5. The fixes

The same hub, eight instances, 64 MiB, the same load, each run adding one fix
to the one before:

| | as before | + own arena | + GC after a call | + unloaded after 2 s idle | + at most 4 loaded |
|---|---|---|---|---|---|
| started, idle | 40.4 MiB | 39.7 | 39.6 | 13.9 | 13.8 |
| JavaScript held after the load | 33.2 MiB | 33.2 | 24.8 | 24.8 | 15.3, 4 loaded |
| RSS after the load (peak) | 62.1 (72.0) | 60.0 (71.8) | 50.4 (61.2) | 49.7 (60.8) | 39.7 (58.2) |
| times the container met its limit | 150 | 100 | 26 | 55 | 42 |
| 3 s after the load | 58.0 | 58.1 | 49.5 | 23.9 | 23.4 |
| the bomb installed: RSS before → after | 58.0 → 71.6 | 58.1 → 58.2 | 49.5 → 49.5 | 23.9 → 23.9 | 23.4 → 23.4 |
| pages, panels, req/s | 2 487, 970 | 2 393, 963 | 2 044, 956 | 2 618, 1 012 | 2 461, 943 |
| plugin calls, req/s an instance | 92–102 | 82–90 | 75–83 | 76–82 | 75–93 |

The first column survived its bomb this time, after 150 reclaims; in part 4
the same setting was killed by it. It lives on the edge, and which side it
lands on is timing.

**The arena** is what removes the spike: the bomb's pages leave as it fails,
so the process does not move. The hub's own allocations keep their
one-second decay, and its throughput stayed within the noise; the plugins
lost about 11%, the price of allocating without a thread cache.

**Collecting garbage after a call** keeps each heap at what it holds when
loaded. QuickJS sets its next collection at one and a half times the heap it
just collected (`js_trigger_gc` in `quickjs.c`), which is the third the heaps
grew by under load; a collection after each call takes that third away.

**Unloading idle runtimes** is what the container sees most: with nothing
loaded the hub is back to 14 MiB, and three seconds after the load to 24.
Bytecode is larger than its source — Stripe 1.23 MiB from 0.55, OpenAI 1.71
from 0.79, Notion 3.52 from 0.97, Octokit 0.18 from 0.15 — so it is one file
a plugin, read at each load. The first attempt kept it in memory for every
instance and was OOM-killed under the load: 13.3 MiB of bytecode was the
difference (`04-fixes.txt`). A runtime reloads from bytecode in 7–9 ms, 13–19
under load.

**A budget of loaded runtimes** is what bounds the busy case, since unloading
only frees what is idle. With four slots and eight busy instances, an
instance that finds no slot takes the one idle longest or waits: 2 400 waits
and 25 loads over the load, the same throughput, and a tail — p99 up to 2 s
for the instances that waited. With sixteen instances the same four slots
held: 39 MiB after the load, nine reclaims, every call answered.

| | sixteen, 2 s idle, at most 4 | eight, unloaded after every call |
|---|---|---|
| started, idle | 13.8 MiB | 13.8 |
| RSS after the load (peak) | 39.0 (57.1) | 25.1 (64.1) |
| times at the limit | 9 | 128 |
| plugin calls, req/s an instance, p50 | 49–87, 5–13 ms | 13, 136–173 ms |
| loads during the load | 36, 11 455 waits | 3 208, 6.4 ms each |
| the bomb installed | harmless | harmless |

**Unloading after every call** keeps memory lowest and makes every call pay a
load and an unload: 13 calls a second an instance instead of 80. It is what
an instance a dashboard calls once a minute should get, through the idle
timeout, and never what a busy one gets.

## What follows

The promise, said as measured: **Dashbin runs in 64 MiB with its plugins
when they are kept as this round kept them** — QuickJS's heaps in an arena
of their own that returns pages at once, garbage collected after each call,
idle runtimes unloaded after a few seconds with their bytecode on disk, and
at most a few runtimes loaded at once. Measured with sixteen instances of
four real SDKs under the full load; with nothing loaded the hub is 14 MiB.

- **How many slots** is the admin's number, against the container's limit:
  four held sixteen instances of these SDKs in 64 MiB with room; each slot
  costs what its largest plugin holds, 1.4–5.1 MiB here.
- **The idle timeout** is seconds, not minutes: a reload costs 7–19 ms, and an
  instance a dashboard calls once a minute should not be resident between.
- **Bytecode is a file a plugin**, read at each load; it is 1.2–3.6 × its
  source and must not sit in memory per instance.
- **The limit an install runs under is per plugin**, about 1.5 × what it
  holds, and the host says "out of memory" itself when a runtime fails at it.
- **The arena allocates without a thread cache**, which is the plugins' 11%;
  a thread cache of its own for the plugin thread may take some of it back,
  not measured.
- **LLRT runs on `crypto-ring`**, since `crypto-rust` cannot link beside
  TinyStore's `kv`.
