# The Rust slice against the Go engines — 2026-10-10

The first round of the Rust core's own engines, kv, sql and jobs as the `rust`
branch has them, beside the Go engines they replace, under one load: in
process, as a Rust or a Go program calls them, and through Bun, as each side's
SDK calls them. Both sides are the product's code and neither is a prototype.

One caller at a time, Rust reads 1.5–1.7× as fast as Go, hands back 25,000
rows 2.2× as fast and drains a queue 1.8× as fast. A durable write alone costs
the same on both sides. In the round's own commit, with 64 callers at once Go
is ahead everywhere: 2–3× on reads, 1.2–1.4× on writes that share a commit.

The round found four bounds and moved them, each measured against what it
replaces. Two were plain: a job's write that held a thread of its session, and
a file's count of readers. The third is a flag of the SQLite build.
libsqlite3-sys defines `SQLITE_ENABLE_MEMORY_MANAGEMENT`, which makes one page
cache of every connection's in the process, behind one mutex. The same commit
built without it reads 4.6–9.3× as fast with 8 to 64 callers, which is
3.3–5.2× Go where it had been 0.42–0.82×. The fourth is how a commit answers
its callers: one thread woke each of 64, which on this host took as long as
their commit. With the callers waking one another, 64 writers write 1.3–1.5×
as fast, 1.02–1.12× Go where they had been 0.67–0.84×.

What is left behind Go is whatever goes through Bun with many calls in flight,
0.5–0.95× of Go's sidecar, and what is left on the table is reads by two to
four callers, which a mutex of SQLite's own holds to half of what they could
be. Neither is built here.

| Question                                  | Observation                                                                                                                                                                                                                                                                                                                         | What follows                                                                                       |
|-------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|
| Is the core faster a call at a time?      | kv get 1.69×, sql point read 1.48×, 25,000 rows 2.18×, a queue's drain 1.83× of Go; a durable write alone 0.95–1.04×.                                                                                                                                                                                                               | Nothing: a lone write is its sync on both sides.                                                   |
| And with 64 callers?                      | Reads 0.31× (kv) and 0.50× (sql) of Go; grouped writes 0.73–0.82×.                                                                                                                                                                                                                                                                  | The two rows below for reads; the writes are not looked into yet.                                  |
| Where do the reads fall?                  | Where callers outnumber a file's readers: kv, four readers, is 1.09× of Go at 4 callers and 0.46× at 8; sql, eight readers, 1.00× at 8, 0.57× at 16.                                                                                                                                                                                | `6e31865` opens a reader a processor, four to sixteen.                                             |
| Did more readers close it?                | In part: kv get 1.81× of the round's commit at 8 callers, 1.37× at 64; sql 1.18× at 64. Reads still do not grow with callers as Go's do.                                                                                                                                                                                            | A profile at 16 callers, which found the row below.                                                |
| What held the reads back?                 | One mutex of the process, taken for each page a read fetches and lets go: 56% of the samples at 16 callers were in futex calls. Without `SQLITE_ENABLE_MEMORY_MANAGEMENT` the same commit reads a key 4.6× as fast at 8 callers, 9.3× at 16, 7.8× at 64: 3.7×, 5.2× and 3.3× Go.                                                    | `1c85629` builds without the flag, and a test fails where it is defined.                           |
| Why were grouped writes behind Go's?      | A commit's leader woke its callers one at a time: 45% of the samples of 64 writers were in `futex_wake`, and a commit carried 34 writes of 64 where Go's carried 47. With the callers waking one another it carries 46: kv set 1.33×, sql insert 1.35×, jobs add 1.53× of before, 1.02–1.12× Go. One to four writers are as before. | `cbe4a9e`, `2ecab48` and `c729a30`.                                                                |
| Why do four readers give half of eight?   | The mutex of SQLite's WAL index parks a thread that finds it held: 99% of four readers' context switches. Two readers read 0.7× of what one does. With that mutex made to spin first, by a shim, two read 2.1× of one and four 3.7×.                                                                                                | Not built: the core would give SQLite its mutexes through `SQLITE_CONFIG_MUTEX`.                   |
| Does the core inside Bun pay?             | A call at a time, 11.6× the Go sidecar and 7.7× its own sidecar. At 64 in flight it is the sidecar's rate, 0.75× of Go's.                                                                                                                                                                                                           | A point read is answered on one thread of a session; measure spreading it when many are in flight. |
| Why were jobs slow through Rust's server? | 0.30× of Go at 64 in flight: each add held one of a session's sixteen threads until its commit.                                                                                                                                                                                                                                     | `08b36e4` answers from the commit's completion: 2.38× through the sidecar, 2.72× in process.       |
| What does it cost in memory?              | Rust's peak resident memory is 13–23 MiB in every case in process, Go's 13–73 MiB; with a page cache a connection, sixteen readers hold 54–57 MiB, Go's level.                                                                                                                                                                      | Nothing.                                                                                           |

## Environment and reproduction

Everything ran in one evening on one AMD Ryzen 7 7700 (8 cores, 16 logical
processors) through Docker Desktop on WSL2: Linux 6.18.33.2, glibc 2.41, the
`golang:1.27` image, which saw all 16 processors. The stores were on a Docker
volume on WSL2's ext4, never the bind mount. This is not bare Linux, and the
host ran its usual background work: read the ratios of one run, and no number
here as a promise for a deployment. Nothing else built, tested or measured
while a run was timed.

The round ran twice. A patch of the runner failed to apply, so the command
meant for the follow-up on callers repeated the whole round and wrote over the
first run's rows. The two runs agreed within a few percent case by case; the
second is the one kept, and the first is gone.

| What                 | Commit or version                                                                                                                                                                          |
|----------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| TinyStore Rust       | `51b25920a9d14886f327789d2ad29e25c17539d1`, branch `rust`                                                                                                                                  |
| later commit         | `6e318652225a0720e24758f69ba22572e2f70e42`, branch `rust`: `08b36e4` and `6e31865` together                                                                                                |
| the build's flag     | the later commit twice, with and without `-USQLITE_ENABLE_MEMORY_MANAGEMENT`; `1c85629695772baff9974e83f4d4cbdc60bfd2c8` on `rust` puts it in the build                                    |
| the commit's callers | `1c85629…` before; `cbe4a9ef3804d3290793db7a56549a33b6de911f`, `2ecab48c6532cbd551ee51f1f8cc6a8cad962565` and `c729a30059d65d17db67b5308d3fb3125346ebc7` on `rust`, each on the one before |
| TinyStore Go         | `e81a0503cb5ba2d12a359662ab0163578ddfe581`                                                                                                                                                 |
| Rust                 | rustc 1.99.0, the release profile, rusqlite 0.40.2 with its bundled SQLite                                                                                                                 |
| Go                   | go1.27.1, `CGO_ENABLED=0`, ncruces/go-sqlite3 v0.35.6                                                                                                                                      |
| Bun                  | 1.4.2                                                                                                                                                                                      |

The Rust side builds with the flags CI gives SQLite on Linux,
`LIBSQLITE3_FLAGS='SQLITE_DQS=0 -DHAVE_FDATASYNC=1'`. Both sides keep their own
defaults otherwise: WAL and `synchronous=FULL`, a write returned once it is
synced, each side's own readers, group commit and threads or goroutines.

The harness is [slice-bench](../slice-bench/README.md). It was not committed
while the round ran, against the `measure` skill; it is committed with this
report, unchanged but for `run.py` recording the later commit's hash, which
the two `environment-next-*.json` files here do not carry. For the last two
runs, of the build's flag, the harness was committed first, research
`c8b3fee81209629206ebd0393169c21858955784`, and their environment files name
each tree's commit and flags.

| Run                                     | UTC, 10 October   | Rows | File                                      |
|-----------------------------------------|-------------------|------|-------------------------------------------|
| the round                               | 01:26:02–01:35:54 | 126  | `runs.jsonl`                              |
| reads by the callers at once            | 01:36:23–01:39:43 | 36   | `scaling.jsonl`                           |
| the later commit, in process            | 01:47:40–01:53:37 | 72   | `next-native.jsonl`                       |
| the later commit, through Bun           | 01:53:37–01:56:59 | 36   | `next-bun.jsonl`                          |
| the build's flag, in process            | 02:28:44–02:39:13 | 126  | `pcache-native.jsonl`                     |
| the build's flag, through Bun           | 02:39:13–02:44:52 | 45   | `pcache-bun.jsonl`                        |
| the first of the callers' commits alone | 08:56:27–09:03:34 | 120  | `gather-native.jsonl`, `gather-bun.jsonl` |
| the callers' commits, in process        | 09:12:43–09:20:32 | 132  | `writes-native.jsonl`                     |
| the callers' commits, through Bun       | 09:20:32–09:22:22 | 30   | `writes-bun.jsonl`                        |
| shares and halves                       | 09:36:15–09:39:53 | 72   | `halves-native.jsonl`                     |

The raw rows and each run's environment are in
[data/rust-slice-2026-10-10](data/rust-slice-2026-10-10). The later commit's
tree was built between the second run and the third.

The last four runs build with CI's flags as `1c85629` has them,
`SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT -DHAVE_FDATASYNC=1`. A first
run of the shares and halves lost its last pass to something else on the
host, every program at half its rate and Go among them: it was run again, and
its rows are not kept. The profiles, the counts of syncs and the mutex's shim
beside the runs are diagnoses, one run each unless they say otherwise.

A case is a process of its own and a fresh store, timed for three seconds. A
pass runs every case, the programs in turn case by case, and every other pass
turns their order round. Every table gives the three passes as they ran, and a
ratio is the ratio of their medians.

```sh
# from Git Bash, MSYS_NO_PATHCONV=1; <tinystore> has every commit named above
docker run --rm -v <tinystore>:/tiny:ro -v <research>:/src -v tinystore-perf:/perf \
  golang:1.27 sh /src/tinystore/slice-bench/build.sh sources
docker run --rm -v <research>:/src -v tinystore-perf:/perf -v tinystore-go:/go \
  -v tinystore-gocache:/root/.cache/go-build -e GOWORK=off golang:1.27 sh /src/tinystore/slice-bench/build.sh go
docker run --rm -v <research>:/src -v tinystore-perf:/perf -v tinystore-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/perf/slice/target rust:1.99 sh /src/tinystore/slice-bench/build.sh rust

# the round, then the reads by the callers at once
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e RUNS=scaling -e CASES=kv-get:4,kv-get:8,kv-get:16,sql-point:4,sql-point:8,sql-point:16 golang:1.27 python3 run.py

# the later commit: the same builds with -e WORK=/perf/slice-next -e RUST_COMMIT=6e31865
# and CARGO_TARGET_DIR=/perf/slice-next/target, then
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e NEXT=/perf/slice-next -e RUNS=next-native -e ONLY=rust,rust-next \
  -e CASES=kv-get:1,kv-get:8,kv-get:16,kv-get:64,kv-set:64,sql-point:1,sql-point:16,sql-point:64,sql-insert:64,jobs-add:1,jobs-add:64,jobs-drain:8 \
  golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e NEXT=/perf/slice-next -e RUNS=next-bun \
  -e ONLY=bun-rust-sidecar,bun-rust-sidecar-next,bun-rust-embedded,bun-rust-embedded-next \
  -e CASES=kv-get:64,sql-point:64,jobs-add:64 golang:1.27 python3 run.py

# the build's flag: the later commit again into /perf/slice-nomm, with -e WORK=/perf/slice-nomm
# -e RUST_COMMIT=6e31865 and, for build.sh rust, CARGO_TARGET_DIR=/perf/slice-nomm/target and
# -e 'SQLITE_FLAGS=SQLITE_DQS=0 -DHAVE_FDATASYNC=1 -USQLITE_ENABLE_MEMORY_MANAGEMENT', then
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e ALSO=next=/perf/slice-next,nomm=/perf/slice-nomm -e RUNS=pcache-native -e ONLY=go,rust-next,rust-nomm \
  -e CASES=kv-get:1,kv-get:4,kv-get:8,kv-get:16,kv-get:64,sql-point:1,sql-point:8,sql-point:16,sql-point:64,kv-set:64,sql-insert:64,sql-rows:1,jobs-add:64,jobs-drain:8 \
  golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e ALSO=next=/perf/slice-next,nomm=/perf/slice-nomm -e RUNS=pcache-bun \
  -e ONLY=bun-go-sidecar,bun-rust-sidecar-next,bun-rust-sidecar-nomm,bun-rust-embedded-next,bun-rust-embedded-nomm \
  -e CASES=kv-get:1,kv-get:64,sql-point:64 golang:1.27 python3 run.py

# the commit's callers: four trees built the same way with
# -e 'SQLITE_FLAGS=SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT -DHAVE_FDATASYNC=1',
# /perf/slice-base at 1c85629, -gather at cbe4a9e, -wake at 2ecab48, -halves at c729a30, then
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e ALSO=base=/perf/slice-base,gather=/perf/slice-gather,wake=/perf/slice-wake -e RUNS=writes-native \
  -e ONLY=go,rust-base,rust-gather,rust-wake \
  -e CASES=kv-set:1,kv-set:2,kv-set:4,kv-set:16,kv-set:64,sql-insert:1,sql-insert:64,jobs-add:1,jobs-add:64,jobs-drain:8,kv-get:64 \
  golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e ALSO=base=/perf/slice-base,wake=/perf/slice-wake -e RUNS=writes-bun \
  -e ONLY=bun-go-sidecar,bun-rust-sidecar-base,bun-rust-sidecar-wake,bun-rust-embedded-base,bun-rust-embedded-wake \
  -e CASES=kv-set:64,jobs-add:64 golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
  -e ALSO=base=/perf/slice-base,shares=/perf/slice-wake,halves=/perf/slice-halves -e RUNS=halves-native \
  -e ONLY=go,rust-base,rust-shares,rust-halves \
  -e CASES=kv-set:1,kv-set:4,kv-set:16,kv-set:64,sql-insert:64,jobs-add:64 golang:1.27 python3 run.py

# SQLite's mutexes made to spin, on a tree already built
docker run --rm -v <research>:/src -v tinystore-perf:/perf golang:1.27 \
  sh /src/tinystore/slice-bench/mutex.sh /perf/slice-wake /src/tinystore/reports/data/rust-slice-2026-10-10/mutex.jsonl

python3 summarize.py
```

## The load

| Case       | What a call is                                                                                           |
|------------|----------------------------------------------------------------------------------------------------------|
| kv-get     | one key of 100,000, each a value of 128 bytes; the key chosen by a xorshift every program shares         |
| kv-set     | a new key with 128 bytes, returned once it is synced                                                     |
| sql-point  | `select id, title, body from note where id = ?` on 100,000 rows of about 130 bytes, decoded to a struct  |
| sql-insert | one row, returned once it is synced                                                                      |
| sql-rows   | 25,000 rows, `order by id limit 25000`, as a list of structs; the rate is reads a second                 |
| jobs-add   | one job of two fields, returned once it is synced                                                        |
| jobs-drain | 20,000 jobs added beforehand, run by eight workers whose handler does nothing; the rate is jobs a second |

"In flight" is the callers at once: threads in Rust, goroutines in Go, promises
awaited in Bun. Through Bun, Go's SDK speaks to Go's `tinystore serve` beside
it, and Rust's to its own `tinystore serve` beside it or to the core inside the
Bun process.

## In process

Calls a second, each pass.

| Case       | In flight | go                        | rust                      | rust to go |
|------------|-----------|---------------------------|---------------------------|------------|
| kv-get     | 1         | 132,070; 130,711; 131,985 | 223,253; 224,307; 220,003 | 1.69×      |
| kv-get     | 64        | 571,810; 563,631; 574,288 | 177,834; 182,780; 176,746 | 0.31×      |
| kv-set     | 1         | 495; 476; 482             | 501; 510; 502             | 1.04×      |
| kv-set     | 64        | 17,652; 17,890; 17,851    | 14,672; 14,453; 14,908    | 0.82×      |
| sql-point  | 1         | 170,825; 169,134; 168,876 | 251,847; 245,646; 250,557 | 1.48×      |
| sql-point  | 64        | 618,097; 611,791; 625,912 | 306,162; 306,094; 303,837 | 0.50×      |
| sql-insert | 1         | 308; 302; 309             | 299; 293; 292             | 0.95×      |
| sql-insert | 64        | 21,451; 21,584; 21,435    | 15,900; 15,739; 15,690    | 0.73×      |
| sql-rows   | 1         | 73.2; 73.0; 73.2          | 159; 159; 159             | 2.18×      |
| jobs-add   | 1         | 478; 469; 471             | 489; 483; 493             | 1.04×      |
| jobs-add   | 64        | 24,109; 23,645; 24,033    | 17,654; 17,368; 17,701    | 0.73×      |
| jobs-drain | 8         | 4,521; 4,583; 4,578       | 8,256; 8,367; 8,366       | 1.83×      |

A call at a time, what the Rust core spends around SQLite is less: a read
takes about 4.5 µs where Go's takes 7.6 µs, and 25,000 rows 6.3 ms where Go's
take 13.7 ms. A write alone is 2 to 3.4 ms on both sides, which is the sync.

With 64 callers the order turns over. Go's reads grow 3.7–4.3× from one caller
to 64 and Rust's fall, which the next section takes apart. Writes that share a
commit are 0.73–0.82× of Go's and this round did not look for why: how many
writes a commit carries on each side is the first thing to count.

The program's peak resident memory, MiB, each pass:

| Case       | In flight | go         | rust       |
|------------|-----------|------------|------------|
| kv-get     | 1         | 24; 24; 24 | 17; 17; 17 |
| kv-get     | 64        | 67; 69; 68 | 15; 16; 15 |
| kv-set     | 64        | 36; 36; 36 | 13; 13; 13 |
| sql-point  | 64        | 66; 73; 64 | 23; 23; 23 |
| sql-insert | 64        | 29; 31; 29 | 13; 13; 13 |
| sql-rows   | 1         | 25; 25; 26 | 16; 16; 16 |
| jobs-add   | 64        | 33; 33; 34 | 13; 13; 13 |
| jobs-drain | 8         | 19; 18; 19 | 13; 13; 13 |

The cases with one caller that the table leaves out are 13–20 MiB on Go and
13 MiB on Rust.

## Reads by the callers at once

The round's own commit: kv opens four readers a file, sql eight.

| Case      | In flight | go                        | rust                      | rust to go |
|-----------|-----------|---------------------------|---------------------------|------------|
| kv-get    | 1         | 132,070; 130,711; 131,985 | 223,253; 224,307; 220,003 | 1.69×      |
| kv-get    | 4         | 383,132; 373,981; 386,921 | 419,037; 420,422; 412,590 | 1.09×      |
| kv-get    | 8         | 441,406; 434,199; 445,411 | 201,994; 201,602; 196,466 | 0.46×      |
| kv-get    | 16        | 473,826; 440,511; 470,387 | 172,147; 173,317; 172,632 | 0.37×      |
| kv-get    | 64        | 571,810; 563,631; 574,288 | 177,834; 182,780; 176,746 | 0.31×      |
| sql-point | 1         | 170,825; 169,134; 168,876 | 251,847; 245,646; 250,557 | 1.48×      |
| sql-point | 4         | 484,596; 480,760; 473,100 | 434,532; 443,012; 440,206 | 0.92×      |
| sql-point | 8         | 504,488; 487,877; 509,876 | 505,720; 500,393; 508,861 | 1.00×      |
| sql-point | 16        | 541,763; 530,134; 534,658 | 305,665; 312,109; 300,356 | 0.57×      |
| sql-point | 64        | 618,097; 611,791; 625,912 | 306,162; 306,094; 303,837 | 0.50×      |

The rows of 1 and 64 are the round's, an earlier run of the same evening; the
others ran together.

Each file keeps Go's pace until its callers pass its readers, and halves on
the next step: kv between 4 and 8, sql between 8 and 16. That separates the
readers' bound from the engine: the same point read through sql, with twice
the readers, falls one step later. A read that has to wait for a reader costs
several reads. Why it does is a hypothesis: the waiter sleeps and is woken
through a mutex and a condition, which takes longer than the 4 µs of a read.

## Through Bun

Calls a second, each pass.

| Case      | In flight | Go beside                 | Rust beside            | Rust inside            | Rust inside to Go beside |
|-----------|-----------|---------------------------|------------------------|------------------------|--------------------------|
| kv-get    | 1         | 6,819; 6,787; 6,807       | 9,492; 10,245; 10,177  | 78,936; 79,562; 77,919 | 11.60×                   |
| kv-get    | 64        | 119,413; 118,722; 119,840 | 87,814; 86,014; 88,156 | 89,341; 90,122; 87,200 | 0.75×                    |
| kv-set    | 64        | 14,960; 14,959; 14,758    | 12,962; 13,030; 13,391 | 13,490; 13,261; 13,478 | 0.90×                    |
| sql-point | 64        | 98,637; 95,631; 98,137    | 62,982; 62,467; 62,544 | 49,661; 50,417; 51,569 | 0.51×                    |
| sql-rows  | 1         | 19.4; 19.4; 19.5          | 31.7; 32.1; 33.1       | 43.3; 42.8; 44.0       | 2.23×                    |
| jobs-add  | 64        | 19,644; 19,575; 19,978    | 6,454; 6,128; 6,318    | 5,935; 5,962; 6,092    | 0.30×                    |

A call at a time, the core inside the process answers a get in about 13 µs
where a server beside it takes 100–150 µs: there is no socket between them.
That is the case an embedded store is for, and it is 11.6× Go's sidecar.

At 64 in flight the socket is no longer the bound and the core inside gives
what its own sidecar gives, both below Go's. A session answers a point read
of kv on the thread that reads its frames, so 64 reads in flight are served
one after another; that this is the bound is a hypothesis, which a route that
spreads reads would separate. The sql point read is 0.64× of Go beside and
0.51× inside, and nothing here says why inside is the slower of the two.

25,000 rows come 1.6× as fast from Rust's server as from Go's and 2.2× as fast
from the core inside. The Bun process's peak memory in that case is 77–82 MiB
with Go's SDK, 101–105 MiB with Rust's beside and 131–142 MiB with the core
inside, where the core's own memory counts too; every other case is within
17 MiB across the three.

Jobs through Rust's server were 0.30× of Go's, though the same adds in process
are 0.73×. An add over the wire ran on one of a session's sixteen threads and
held it until its commit, so no commit carried more than sixteen adds.

## The later commit

`6e31865` on `rust` holds two changes the round asked for: `08b36e4`, which
answers a job's write over the wire from its commit's completion, with no
thread held, and takes the job's id inside the write; and `6e31865`, which
opens a reader a processor, at least four and at most sixteen, where kv had
four and sql eight. The two were measured together and not apart. The reads
are given to the second and the jobs to the first because neither touches the
other's path, which is an argument and not a measurement.

In process, calls a second, each pass; `rust` is the round's commit again, run
in turn with the later one.

| Case       | In flight | rust                      | later                     | later to rust |
|------------|-----------|---------------------------|---------------------------|---------------|
| kv-get     | 1         | 225,480; 219,918; 221,296 | 223,678; 224,364; 219,648 | 1.01×         |
| kv-get     | 8         | 189,798; 199,870; 197,175 | 355,398; 358,448; 356,037 | 1.81×         |
| kv-get     | 16        | 172,286; 175,153; 174,687 | 247,248; 249,381; 248,094 | 1.42×         |
| kv-get     | 64        | 176,336; 164,755; 175,505 | 237,932; 240,752; 245,223 | 1.37×         |
| kv-set     | 64        | 14,619; 14,516; 14,667    | 14,612; 14,942; 14,813    | 1.01×         |
| sql-point  | 1         | 248,397; 248,435; 251,618 | 250,702; 249,965; 252,854 | 1.01×         |
| sql-point  | 16        | 290,846; 303,659; 304,950 | 353,380; 355,825; 360,948 | 1.17×         |
| sql-point  | 64        | 298,589; 288,005; 293,528 | 344,949; 345,179; 332,841 | 1.18×         |
| sql-insert | 64        | 15,826; 16,186; 16,202    | 16,070; 16,114; 16,015    | 0.99×         |
| jobs-add   | 1         | 499; 487; 503             | 529; 522; 520             | 1.05×         |
| jobs-add   | 64        | 17,605; 17,373; 17,553    | 17,091; 17,275; 18,104    | 0.98×         |
| jobs-drain | 8         | 8,349; 8,340; 8,296       | 8,197; 8,242; 8,281       | 0.99×         |

More readers help where callers had outnumbered them and cost nothing a call
at a time. They do not make the reads grow with callers. With sixteen readers
and sixteen callers no read waits for a reader, and kv still gives 248,000 a
second, less than the 356,000 it gives eight callers and the 419,000 the
round's commit gave four. Go grows through the same steps, to 470,000 at 16.
Beside the round's Go figures, which are another run of the same evening, the
later commit is about 0.8× at 8 callers, 0.5× at 16 and 0.4× at 64 for kv, and
0.56× at 64 for sql. Something other than the count of readers contends, and
this round does not say what.

Through Bun, calls a second, each pass.

| Case      | In flight | Rust beside            | later beside           | Rust inside            | later inside           | later to Rust, beside; inside |
|-----------|-----------|------------------------|------------------------|------------------------|------------------------|-------------------------------|
| kv-get    | 64        | 86,973; 85,200; 86,987 | 84,876; 81,552; 85,941 | 89,714; 86,095; 90,701 | 91,087; 89,740; 90,070 | 0.98×; 1.00×                  |
| sql-point | 64        | 62,812; 62,764; 62,169 | 61,341; 62,160; 60,619 | 50,972; 50,614; 49,506 | 51,736; 50,433; 51,020 | 0.98×; 1.01×                  |
| jobs-add  | 64        | 6,560; 6,595; 6,314    | 15,591; 15,632; 15,651 | 5,805; 6,019; 6,093    | 16,370; 16,372; 16,124 | 2.38×; 2.72×                  |

Jobs through the server are 2.4–2.7× what they were, and about 0.8× of the
round's figure for Go's sidecar where they were 0.30×. Reads through Bun do
not move with the readers, which fits their bound being the session's one
thread and not the file.

## The page cache's mutex

A profile of the later commit reading keys with 16 callers,
`perf record -e cpu-clock --call-graph dwarf` in the same container, put 56%
of the processor's samples in futex calls: 36% waking a thread and 18% parking
one. They come from `pthread_mutex_lock` and its unlock under `pcache1Fetch`,
41% with what it calls, and `pcache1Unpin`, 10%. The profile is of the whole
process, the fill by 64 writers before the reads among it; it is kept in
`profile-16-callers-next.txt`.

SQLite takes that mutex only when its page caches are one group. They are one
group when the build defines `SQLITE_ENABLE_MEMORY_MANAGEMENT`, which
libsqlite3-sys's bundled build does and SQLite's own default does not: then
every connection of the process shares one cache and one LRU list, and each
page a statement fetches or lets go takes the group's mutex, a pthread mutex
that parks a thread that finds it held. Sixteen readers of one file spend
their time waking one another.

The same commit, `6e31865`, built twice, the second with
`-USQLITE_ENABLE_MEMORY_MANAGEMENT` after the flags of the first, and Go
beside them in the same run. Calls a second, each pass.

| Case       | In flight | go                        | with the flag             | without it                      | without to with | without to go |
|------------|-----------|---------------------------|---------------------------|---------------------------------|-----------------|---------------|
| kv-get     | 1         | 130,340; 130,429; 130,886 | 221,400; 225,608; 224,046 | 218,985; 217,201; 211,911       | 0.97×           | 1.67×         |
| kv-get     | 4         | 371,577; 372,979; 379,744 | 421,190; 423,232; 415,168 | 410,714; 420,835; 407,872       | 0.98×           | 1.10×         |
| kv-get     | 8         | 425,201; 431,323; 428,845 | 351,164; 354,304; 349,912 | 1,602,393; 1,593,501; 1,631,889 | 4.56×           | 3.74×         |
| kv-get     | 16        | 456,630; 460,818; 447,216 | 272,704; 248,875; 255,800 | 2,372,148; 2,377,247; 2,267,038 | 9.27×           | 5.19×         |
| kv-get     | 64        | 537,064; 557,154; 555,278 | 243,167; 232,076; 226,065 | 1,879,154; 1,709,134; 1,821,306 | 7.85×           | 3.28×         |
| sql-point  | 1         | 168,618; 169,105; 169,861 | 252,400; 252,411; 255,108 | 250,909; 248,362; 252,965       | 0.99×           | 1.48×         |
| sql-point  | 8         | 462,726; 484,342; 487,089 | 488,284; 502,260; 498,858 | 1,572,546; 1,538,787; 1,542,039 | 3.09×           | 3.18×         |
| sql-point  | 16        | 517,475; 559,342; 526,177 | 354,666; 366,435; 352,826 | 2,559,735; 2,538,483; 2,529,963 | 7.16×           | 4.82×         |
| sql-point  | 64        | 595,964; 615,643; 603,182 | 319,974; 338,351; 339,622 | 2,061,818; 1,926,428; 2,021,115 | 5.97×           | 3.35×         |
| kv-set     | 64        | 17,887; 17,924; 17,766    | 14,580; 14,516; 14,934    | 14,750; 14,674; 14,702          | 1.01×           | 0.82×         |
| sql-insert | 64        | 21,522; 21,554; 21,590    | 15,707; 15,862; 16,014    | 16,167; 16,187; 15,977          | 1.02×           | 0.75×         |
| sql-rows   | 1         | 72.4; 72.4; 73.4          | 147; 157; 164             | 165; 165; 164                   | 1.05×           | 2.27×         |
| jobs-add   | 64        | 23,724; 23,649; 24,055    | 18,266; 18,093; 17,929    | 16,470; 16,373; 16,142          | 0.90×           | 0.69×         |
| jobs-drain | 8         | 4,583; 4,592; 4,562       | 8,203; 8,302; 8,520       | 8,119; 8,336; 8,200             | 0.99×           | 1.79×         |

Reads by eight callers or more are 3–9× what they were and 3.2–5.2× Go's. One
caller reads as before, within 3%. The same profile of the build without the
flag, `profile-16-callers-nomm.txt`, has 11% of its samples in futex calls
where there were 56%, and 22% in `pread`.

Two cases did not gain. Four callers give 410,000–420,000 gets a second in
both builds, half of what eight give without the flag. Pinning the process to
two cores of the sixteen or to four changed nothing, 411,000–420,000 in each
of two runs, so it is not where the host puts four threads; a later section
finds it in a mutex of SQLite's. And jobs' adds by 64 callers fell a tenth, 18,093 to 16,373 a
second. A writer that could borrow idle readers' share of the one cache now
has its own 1 MiB alone, which fits and was not measured apart.

The program's peak resident memory, MiB, the median of the passes:

| Case      | In flight | go | with the flag | without it |
|-----------|-----------|----|---------------|------------|
| kv-get    | 1         | 24 | 17            | 17         |
| kv-get    | 8         | 55 | 27            | 40         |
| kv-get    | 16        | 58 | 36            | 57         |
| kv-get    | 64        | 67 | 36            | 47         |
| sql-point | 16        | 53 | 31            | 54         |
| sql-point | 64        | 65 | 36            | 48         |
| jobs-add  | 64        | 34 | 13            | 17         |

Sixteen readers each with a cache of their own hold some 20 MiB more than
sixteen sharing one, which is Go's level; the readers past the first close
after a minute unused.

Through Bun nothing moved, calls a second, each pass:

| Case      | In flight | Go beside                 | Rust beside, with      | Rust beside, without   | Rust inside, with      | Rust inside, without   |
|-----------|-----------|---------------------------|------------------------|------------------------|------------------------|------------------------|
| kv-get    | 1         | 6,744; 6,681; 6,671       | 10,173; 10,237; 10,034 | 10,140; 10,110; 10,019 | 78,065; 79,447; 78,019 | 77,695; 78,340; 78,665 |
| kv-get    | 64        | 120,073; 118,261; 119,329 | 85,943; 86,024; 87,699 | 85,944; 83,709; 86,439 | 90,005; 86,348; 90,485 | 89,223; 86,302; 86,482 |
| sql-point | 64        | 95,387; 96,197; 96,979    | 61,796; 62,834; 61,634 | 63,451; 63,033; 62,721 | 50,376; 50,459; 51,794 | 49,275; 50,984; 49,817 |

Reads through a session were not waiting for the file, so freeing the file
gives them nothing: their bound is ahead of it, in the session or the SDK.

`1c85629` on `rust` undefines the flag in the workspace's build and in CI, and
a test of the SQLite adapter fails where the build still has it. A Rust
program that builds the crate outside the workspace sets `LIBSQLITE3_FLAGS`
itself, as it does for `HAVE_FDATASYNC`.

## Writes that share a commit

A file has one writer, and its callers take turns to lead: the one that leads
commits every write queued behind it, each in a savepoint, and one sync
carries them all. With 64 callers a sync should carry 64. Counting
`fdatasync` under `strace`, one run a program, a sync of Go's carried 47
writes and a sync of the Rust core's 34:

| Program                         | Writes | Syncs | Writes a sync | A sync, µs |
|---------------------------------|--------|-------|---------------|------------|
| go                              | 47,156 | 1,003 | 47.0          | 1,814      |
| before                          | 25,477 | 753   | 33.8          | 1,752      |
| gathers both                    | 25,604 | 666   | 38.4          | 1,917      |
| callers wake callers, by shares | 35,042 | 760   | 46.1          | 1,861      |
| callers wake callers, in halves | 35,152 | 770   | 45.7          | 1,818      |

The first commit, `cbe4a9e`, mends what the leader waits for. It waited for
as many writes as the last commit had answered, and with 64 callers that many
already stood in the queue, the half that had come while the other half's
commit ran: the two halves took turns for good. It now waits for both. A test
holds it to that, and on this host it changed little: 38 writes a sync.

A profile says why, `profile-64-writers-before.txt`: 45% of the processor's
samples of 64 writers are in `futex_wake`, under the leader, which woke each
caller its commit answered, one after another. Under this host's hypervisor
a wake that lands on a sleeping processor costs tens of microseconds, so the
callers came back over about as long as their commit had taken, and the next
commit left before most of them. Another 14% went to every arriving write
waking the leader that gathered.

The second commit, `2ecab48`, takes both off the writer. No write wakes a
gathering leader but the one it waits for, and the leader wakes a few callers,
each of which wakes others as it returns, its answer travelling with its wake.
There the leader woke the square root of them and each the rest of its share.
The third, `c729a30`, says the same in one rule: the leader answers two
halves, each through its first caller, which answers the rest of its half the
same way. Calls a second, each pass, the three commits one on another:

| Case       | In flight | go                     | before                 | gathers both           | callers wake callers   | last to before | last to go |
|------------|-----------|------------------------|------------------------|------------------------|------------------------|----------------|------------|
| kv-set     | 1         | 473; 473; 474          | 493; 497; 490          | 489; 475; 497          | 486; 494; 497          | 1.00×          | 1.04×      |
| kv-set     | 2         | 1,036; 1,035; 974      | 1,060; 1,066; 1,063    | 1,047; 1,063; 1,061    | 1,028; 1,068; 1,038    | 0.98×          | 1.00×      |
| kv-set     | 4         | 2,067; 2,095; 2,099    | 2,099; 2,084; 2,082    | 2,104; 2,072; 2,099    | 2,150; 2,141; 2,168    | 1.03×          | 1.03×      |
| kv-set     | 16        | 7,576; 7,495; 7,578    | 6,930; 6,899; 7,204    | 7,199; 7,127; 7,252    | 7,942; 7,850; 7,900    | 1.14×          | 1.04×      |
| kv-set     | 64        | 17,270; 17,313; 17,397 | 14,502; 14,337; 14,291 | 14,562; 14,597; 15,002 | 19,254; 18,475; 18,822 | 1.31×          | 1.09×      |
| sql-insert | 1         | 308; 304; 302          | 310; 309; 310          | 310; 281; 297          | 308; 284; 309          | 0.99×          | 1.01×      |
| sql-insert | 64        | 21,456; 20,461; 20,191 | 15,887; 15,743; 15,620 | 16,024; 16,038; 16,089 | 21,656; 21,544; 21,345 | 1.37×          | 1.05×      |
| jobs-add   | 1         | 466; 472; 461          | 520; 523; 521          | 515; 528; 518          | 535; 516; 520          | 1.00×          | 1.12×      |
| jobs-add   | 64        | 23,201; 23,598; 22,916 | 16,695; 13,853; 16,281 | 16,722; 15,591; 16,648 | 24,507; 24,443; 24,193 | 1.50×          | 1.05×      |
| jobs-drain | 8         | 4,563; 4,595; 4,491    | 8,210; 8,172; 8,217    | 8,350; 8,436; 8,270    | 8,372; 8,302; 8,241    | 1.01×          | 1.82×      |

The first commit alone is within what two builds of one code differ by. The
second gives 64 writers 1.31–1.50× and puts them ahead of Go; 16 writers gain
1.14×, and one to four write as before, a sync a write. The read beside them,
`kv-get` by 64 callers, spread by a quarter between its passes in every
program of this run, Go among them, and says nothing here.

Shares against halves, with the commit before them and Go:

| Case       | In flight | go                     | before                 | by shares              | in halves              | halves to before | halves to go |
|------------|-----------|------------------------|------------------------|------------------------|------------------------|------------------|--------------|
| kv-set     | 1         | 255; 478; 486          | 500; 492; 501          | 499; 479; 494          | 501; 502; 481          | 1.00×            | 1.05×        |
| kv-set     | 4         | 2,107; 2,112; 2,096    | 2,083; 2,123; 2,107    | 2,157; 2,171; 2,047    | 2,150; 2,164; 2,133    | 1.02×            | 1.02×        |
| kv-set     | 16        | 7,544; 7,548; 7,637    | 6,869; 6,873; 7,043    | 7,811; 7,883; 7,936    | 7,795; 7,900; 7,807    | 1.14×            | 1.03×        |
| kv-set     | 64        | 16,875; 17,010; 17,083 | 13,859; 14,304; 14,478 | 18,359; 18,914; 18,637 | 18,282; 18,977; 19,080 | 1.33×            | 1.12×        |
| sql-insert | 64        | 20,408; 20,719; 20,841 | 15,408; 15,770; 15,630 | 20,825; 21,635; 21,423 | 21,165; 21,517; 21,063 | 1.35×            | 1.02×        |
| jobs-add   | 64        | 23,626; 23,698; 23,503 | 15,912; 16,230; 15,797 | 24,166; 24,183; 24,067 | 24,595; 24,306; 24,183 | 1.53×            | 1.03×        |

They are the same within a pass's spread, and a sync carries 46 writes either
way. The halves are what the branch keeps, as the rule with no number to
choose. Their cost is that a caller slow to wake delays the answers it
carries, up to half a commit's; the caller that leads delays every write in
the same way already. In this run a write by 64 callers took 4.0 ms at the
median before and 2.6 ms after, where it had waited two commits and now waits
one; the tail belongs to a bare host.

Through Bun a write is answered by a completion and no caller sleeps, so there
is little to gain, calls a second, each pass, before and after `2ecab48`:

| Case     | In flight | Go beside              | Rust beside, before    | Rust beside, after     | Rust inside, before    | Rust inside, after     | inside, after to before | inside, after to Go |
|----------|-----------|------------------------|------------------------|------------------------|------------------------|------------------------|-------------------------|---------------------|
| kv-set   | 64        | 14,545; 14,422; 14,676 | 12,772; 12,985; 13,018 | 12,808; 12,640; 12,629 | 13,265; 13,499; 13,113 | 13,884; 13,936; 13,823 | 1.05×                   | 0.95×               |
| jobs-add | 64        | 19,361; 19,473; 18,648 | 14,208; 14,415; 13,967 | 14,360; 14,362; 14,097 | 14,906; 14,964; 15,345 | 15,761; 15,717; 15,915 | 1.05×                   | 0.81×               |

Writes through Bun stay 0.74–0.95× of Go's sidecar, and the bound is not the
file: the same writes in process are ahead of Go.

## Readers that wait for SQLite's mutex

Four callers read half of what eight do, and two read less than one. A record
of every context switch of four readers, `switches-4-readers.txt`, puts 99% of
them in one place: `pthread_mutex_lock` under `unixShmLock`, which every read
transaction calls as it begins and as it ends, to take and let go its lock on
the WAL's index. SQLite keeps those locks of one process behind one mutex a
file, a pthread mutex of the default kind, which parks a thread the moment it
finds the mutex held. The mutex is held for well under a microsecond, and a
parked thread on this host is tens of microseconds from running again.

A shim says how much that costs without touching SQLite. `mutex.sh` loads a
library before the program that gives every `pthread_mutex_init` without
attributes glibc's adaptive kind, which spins before it parks, and glibc's
tunable says for how long. The tree is `2ecab48`'s; calls a second, two
passes:

| Case      | Callers | parks at once        | spins 100 times first | spins 1,000 times first | 1,000 to parking |
|-----------|---------|----------------------|-----------------------|-------------------------|------------------|
| kv-get    | 1       | 202,062; 205,075     | 201,454; 197,516      | 193,044; 200,525        | 0.97×            |
| kv-get    | 2       | 145,528; 149,010     | 316,460; 329,645      | 429,027; 434,508        | 2.93×            |
| kv-get    | 3       | 282,661; 287,160     | 379,255; 365,379      | 555,712; 564,841        | 1.97×            |
| kv-get    | 4       | 425,991; 423,509     | 498,797; 503,962      | 750,377; 742,364        | 1.76×            |
| kv-get    | 6       | 923,471; 897,553     | 1,009,961; 1,013,096  | 999,434; 1,000,413      | 1.10×            |
| kv-get    | 8       | 1,479,021; 1,462,594 | 1,544,157; 1,447,436  | 1,571,628; 1,553,495    | 1.06×            |
| kv-get    | 16      | 2,038,913; 1,873,256 | 2,137,629; 2,138,807  | 2,069,880; 2,007,516    | 1.04×            |
| sql-point | 2       | 139,976              | —                     | 426,799                 | 3.05×            |
| sql-point | 4       | 366,992              | —                     | 765,953                 | 2.09×            |

One caller is as before, and eight and more gain 4–6%, which two passes do
not tell from their spread. Between them a mutex that spins a thousand times
first gives two callers 2.9× and four 1.8×, and brings the reads close to a
line: two callers read 2.1× of one and four 3.7×. Four callers by how long it
spins, one run each: 429,174 at 30 times, 703,111 at 300, 738,742 at 3,000
and 743,527 at 30,000. The gain is there by 300 and flat after.

It is not built. SQLite's mutexes are its build's own, and the core would
have to give it others before it starts, through `SQLITE_CONFIG_MUTEX`, in
unsafe code and for the whole process. How much of the parked thread's cost
is this host's hypervisor is not known either: a bare host would say.

## What follows

Nothing below is built.

- **A connection's cache.** With a cache each, 22% of the samples at 16
  callers are `pread` of pages a reader's 1 MiB does not hold, and jobs' adds
  lost a tenth. Measure a larger cache for the writer and for readers, and
  reads through a memory map, against the memory each holds.
- **SQLite's mutexes, spinning before they park**, for two to four readers,
  as the section above has it: first on a bare host, to see what is left of
  the cost there.
- **A reader's handoff.** The readers' pool wakes a waiter each time a reader
  comes back, whether one waits or not: a system call a read, with nobody to
  wake.
- **Writes through Bun at many in flight**, 0.74–0.95× of Go's sidecar, where
  the same writes in process are ahead of Go: the bound is in the session or
  the SDK, as it is for reads.
- **The last writes a sync could carry**: 46 of 64 on both sides.
- **Point reads through a session at many in flight**, 0.51–0.75× of Go's
  sidecar, and now 20–40 times below the same reads in process: measure
  answering them off the reading thread once several wait.
- **The other writes that hold a session's thread until their commit**: kv's
  durable counter, `kv.clear`, `kv.tx`, a write sent through `sql.query`.
  Jobs' were found because a case measured them; these have no case yet.
- **The Bun process's memory for 25,000 rows**, 1.3–1.8× what Go's SDK takes.
- **Latencies.** The raw rows carry each case's p50 and p99. They are not
  quoted here: a tail measured under WSL2 says more of the host than of the
  store. They wait for bare Linux.
