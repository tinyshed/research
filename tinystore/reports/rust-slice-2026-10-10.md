# The Rust slice against the Go engines — 2026-10-10

The first round of the Rust core's own engines, kv, sql and jobs as the `rust`
branch has them, beside the Go engines they replace, under one load: in
process, as a Rust or a Go program calls them, and through Bun, as each side's
SDK calls them. Both sides are the product's code and neither is a prototype.

One caller at a time, Rust reads 1.5–1.7× as fast as Go, hands back 25,000
rows 2.2× as fast and drains a queue 1.8× as fast. A durable write alone costs
the same on both sides. With 64 callers at once Go is ahead everywhere: 2–3× on
reads, 1.2–1.4× on writes that share a commit. The round found two bounds of
the Rust core, and a later commit that moves both is measured beside the
round's own at the end. It closes the jobs' one and half of the readers'; what
still holds Rust's reads back with many callers is not known.

| Question                                  | Observation                                                                                                                                          | What follows                                                                                       |
|-------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|
| Is the core faster a call at a time?      | kv get 1.69×, sql point read 1.48×, 25,000 rows 2.18×, a queue's drain 1.83× of Go; a durable write alone 0.95–1.04×.                                | Nothing: a lone write is its sync on both sides.                                                   |
| And with 64 callers?                      | Reads 0.31× (kv) and 0.50× (sql) of Go; grouped writes 0.73–0.82×.                                                                                   | The two rows below for reads; the writes are not looked into yet.                                  |
| Where do the reads fall?                  | Where callers outnumber a file's readers: kv, four readers, is 1.09× of Go at 4 callers and 0.46× at 8; sql, eight readers, 1.00× at 8, 0.57× at 16. | `6e31865` opens a reader a processor, four to sixteen.                                             |
| Did more readers close it?                | In part: kv get 1.81× of the round's commit at 8 callers, 1.37× at 64; sql 1.18× at 64. Reads still do not grow with callers as Go's do.             | Find what contends at 16 callers on 16 readers before changing anything else.                      |
| Does the core inside Bun pay?             | A call at a time, 11.6× the Go sidecar and 7.7× its own sidecar. At 64 in flight it is the sidecar's rate, 0.75× of Go's.                            | A point read is answered on one thread of a session; measure spreading it when many are in flight. |
| Why were jobs slow through Rust's server? | 0.30× of Go at 64 in flight: each add held one of a session's sixteen threads until its commit.                                                      | `08b36e4` answers from the commit's completion: 2.38× through the sidecar, 2.72× in process.       |
| What does it cost in memory?              | Rust's peak resident memory is 13–23 MiB in every case in process, Go's 13–73 MiB.                                                                   | Nothing.                                                                                           |

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

| What           | Commit or version                                                                           |
|----------------|---------------------------------------------------------------------------------------------|
| TinyStore Rust | `51b25920a9d14886f327789d2ad29e25c17539d1`, branch `rust`                                   |
| later commit   | `6e318652225a0720e24758f69ba22572e2f70e42`, branch `rust`: `08b36e4` and `6e31865` together |
| TinyStore Go   | `e81a0503cb5ba2d12a359662ab0163578ddfe581`                                                  |
| Rust           | rustc 1.99.0, the release profile, rusqlite 0.40.2 with its bundled SQLite                  |
| Go             | go1.27.1, `CGO_ENABLED=0`, ncruces/go-sqlite3 v0.35.6                                       |
| Bun            | 1.4.2                                                                                       |

The Rust side builds with the flags CI gives SQLite on Linux,
`LIBSQLITE3_FLAGS='SQLITE_DQS=0 -DHAVE_FDATASYNC=1'`. Both sides keep their own
defaults otherwise: WAL and `synchronous=FULL`, a write returned once it is
synced, each side's own readers, group commit and threads or goroutines.

The harness is [slice-bench](../slice-bench/README.md). It was not committed
while the round ran, against the `measure` skill; it is committed with this
report, unchanged but for `run.py` recording the later commit's hash, which
the two `environment-next-*.json` files here do not carry.

| Run                           | UTC, 10 October   | Rows | File                |
|-------------------------------|-------------------|------|---------------------|
| the round                     | 01:26:02–01:35:54 | 126  | `runs.jsonl`        |
| reads by the callers at once  | 01:36:23–01:39:43 | 36   | `scaling.jsonl`     |
| the later commit, in process  | 01:47:40–01:53:37 | 72   | `next-native.jsonl` |
| the later commit, through Bun | 01:53:37–01:56:59 | 36   | `next-bun.jsonl`    |

The raw rows and each run's environment are in
[data/rust-slice-2026-10-10](data/rust-slice-2026-10-10). The later commit's
tree was built between the second run and the third.

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

## What follows

Nothing below is built.

- **What holds reads back past eight callers.** With a reader for every
  caller the rate still falls from 8 callers to 16. The candidates are not
  separated: a reader's handoff through the pool's mutex, the `begin` and
  `commit` the core puts round a lone statement where Go lets the statement
  be its own snapshot, sixteen page caches where calls take whichever reader
  is free, and two threads a core past eight callers, which Go has too. A
  profile at 16 callers comes first.
- **Writes that share a commit**, 0.73–0.82× of Go's at 64 callers on all
  three engines. Count commits a second and writes a commit on each side.
- **Point reads through a session at many in flight**, 0.51–0.75× of Go's
  sidecar: measure answering them off the reading thread once several wait.
- **The other writes that hold a session's thread until their commit**: kv's
  durable counter, `kv.clear`, `kv.tx`, a write sent through `sql.query`.
  Jobs' were found because a case measured them; these have no case yet.
- **The Bun process's memory for 25,000 rows**, 1.3–1.8× what Go's SDK takes.
- **Latencies.** The raw rows carry each case's p50 and p99. They are not
  quoted here: a tail measured under WSL2 says more of the host than of the
  store. They wait for bare Linux.
