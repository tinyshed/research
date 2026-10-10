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

Two more bounds fell after those, in commits the last sections measure. Two
to four threads reading one file were held to half of what they could read
by a mutex of SQLite's own, which parks a thread the moment it finds it held:
the core now gives SQLite mutexes that spin first, and two readers read 3.7×
of what they did, four 2.3×. And whatever went through Bun with many calls in
flight was 0.5–0.95× of Go's sidecar, since every call woke a thread of the
connection and point reads took their turn on one: with one wake for a crowd
and the crowd's reads on the store's threads it is 1.0–1.3× of Go's.

A rented server under KVM, four processors, says how much of this is the
host the round ran on. There the mutexes give two readers 1.3× and four
1.1×, and the callers waking one another give 64 writers nothing; the Rust
core is ahead of Go there in every case measured, before and after.

The round's last part is the Bun program again, whose bound is its one
thread. With the core inside it and 64 reads in flight, that thread was on a
processor 93% of the time, two fifths of it reading keys itself while the
store's sixteen threads slept between jobs. Five commits follow from that: a
worker that looks for the next job before it sleeps, a crowd's reads sent to
the workers whole, an SDK that writes a message without a view of its buffer
and makes a call for one promise where it made five, and C functions that
write into the program's own bytes. With the first four a key is read by 64
at once at 289,000 a second where it was 118,000, 2.5× and 2.7× Go's
sidecar; the fifth, in a run of its own, gives 1.19× more. A query for one
row, one at a time, went from 16,000 to 73,000. A bare loop over the C
functions, with no SDK, makes 949,000 with 64 in flight: the SDK is at 37%
of what a Bun program can make of the core.

A commit that does not wait for the disk, which the plan calls `'os'`, was
measured before the engines offered it, as they do since `9cb490f`: one
writer writes 53 to 96 times as many keys on this host and 22 times on a
rented server, 64 writers 1.2 to 2.0 times as many, and a thousand no more.

| Question                                                             | Observation                                                                                                                                                                                                                                                                                                                                                       | What follows                                                                                       |
|----------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|
| Is the core faster a call at a time?                                 | kv get 1.69×, sql point read 1.48×, 25,000 rows 2.18×, a queue's drain 1.83× of Go; a durable write alone 0.95–1.04×.                                                                                                                                                                                                                                             | Nothing: a lone write is its sync on both sides.                                                   |
| And with 64 callers?                                                 | Reads 0.31× (kv) and 0.50× (sql) of Go; grouped writes 0.73–0.82×.                                                                                                                                                                                                                                                                                                | The two rows below for reads; the writes are not looked into yet.                                  |
| Where do the reads fall?                                             | Where callers outnumber a file's readers: kv, four readers, is 1.09× of Go at 4 callers and 0.46× at 8; sql, eight readers, 1.00× at 8, 0.57× at 16.                                                                                                                                                                                                              | `6e31865` opens a reader a processor, four to sixteen.                                             |
| Did more readers close it?                                           | In part: kv get 1.81× of the round's commit at 8 callers, 1.37× at 64; sql 1.18× at 64. Reads still do not grow with callers as Go's do.                                                                                                                                                                                                                          | A profile at 16 callers, which found the row below.                                                |
| What held the reads back?                                            | One mutex of the process, taken for each page a read fetches and lets go: 56% of the samples at 16 callers were in futex calls. Without `SQLITE_ENABLE_MEMORY_MANAGEMENT` the same commit reads a key 4.6× as fast at 8 callers, 9.3× at 16, 7.8× at 64: 3.7×, 5.2× and 3.3× Go.                                                                                  | `1c85629` builds without the flag, and a test fails where it is defined.                           |
| Why were grouped writes behind Go's?                                 | A commit's leader woke its callers one at a time: 45% of the samples of 64 writers were in `futex_wake`, and a commit carried 34 writes of 64 where Go's carried 47. With the callers waking one another it carries 46: kv set 1.33×, sql insert 1.35×, jobs add 1.53× of before, 1.02–1.12× Go. One to four writers are as before.                               | `cbe4a9e`, `2ecab48` and `c729a30`.                                                                |
| Why do four readers give half of eight?                              | The mutex of SQLite's WAL index parks a thread that finds it held: 99% of four readers' context switches. Two readers read 0.7× of what one does. With the core's own mutexes, which spin first, and a readers' pool that wakes nobody for nothing, two read 3.7× of before and four 2.3×: 2.5× and 2.7× Go.                                                      | `63cdb4c` and `df103b6`.                                                                           |
| Why was Bun with many calls in flight behind Go?                     | Every call a connection handed to the store's threads woke one, and a wake cost more than the call; point reads waited their turn on the thread that read them. With one wake for the crowd, which the woken thread passes on, and a crowd's reads on those threads: kv get 1.4×, sql point read 2.5×, jobs add 1.3× of before inside Bun, 1.0–1.3× Go's sidecar. | `760231e`.                                                                                         |
| How much of it is this host?                                         | Much of the size. On a rented KVM server of four processors the mutexes give two readers 1.31× and four 1.08×, and the callers waking one another give 64 writers 1.00×. Rust is ahead of Go there throughout: 2.4–2.7× on reads, 1.1–1.35× on writes.                                                                                                            | Quote this host's ratios as this host's.                                                           |
| What bounds a Bun program with many calls in flight?                 | Its one thread, which read keys itself and paid a wake for each job it handed over. With a worker that lingers, a crowd sent to the workers whole, and an SDK that makes a call in place, 64 reads in flight are 2.5× of before and 2.7× Go's sidecar; one query at a time is 4.6×. In process nothing moves: 0.96–1.02×.                                         | `6239381`, `d019713`, `3d91315`, `6bb1f6a`.                                                        |
| Does it pay to write the core's frames into the program's own bytes? | Yes: 1.18× one read at a time, 1.24× with 8 in flight, 1.19× with 64. The C functions are four, and none frees.                                                                                                                                                                                                                                                   | `597ecd5`.                                                                                         |
| What can a Bun program make of the core at most?                     | A loop with no SDK makes 949,000 reads a second with 64 in flight and 1,150,000 with 256. The SDK is at 37% of it.                                                                                                                                                                                                                                                | What is left is the SDK's own work a call.                                                         |
| Is a larger cache a connection worth its memory?                     | No. 16 MiB a connection reads a key 1.23× as fast for one caller and 0.88× for sixteen, in 316 MiB where 1 MiB takes 54.                                                                                                                                                                                                                                          | The cache stays at 1 MiB.                                                                          |
| Does a result of 25,000 rows stay in the Bun process?                | No. After a collection the JS heap is 8–10 MiB whichever core answered; the peak of a loop of reads follows how fast it reads.                                                                                                                                                                                                                                    | Nothing to build.                                                                                  |
| What is a commit worth that does not wait for the disk?              | One writer writes 53–96× as many keys here, a sync taking 1.7 to 5.9 ms, and 22× on the server, where it takes 0.7–0.9; 64 writers 1.6–2.0× here and 1.2× there; 1,024 no more; a queue's adds by 64 are 4.5–4.7× here and 1.9× there. Past the disk, a file's one writer makes some 30,000 small writes a second here.                                           | Built since, in `9cb490f`: a store's at `open`, and a database's own.                              |
| Does the core inside Bun pay?                                        | A call at a time, 11.6× the Go sidecar and 7.7× its own sidecar. At 64 in flight it is the sidecar's rate, 0.75× of Go's.                                                                                                                                                                                                                                         | A point read is answered on one thread of a session; measure spreading it when many are in flight. |
| Why were jobs slow through Rust's server?                            | 0.30× of Go at 64 in flight: each add held one of a session's sixteen threads until its commit.                                                                                                                                                                                                                                                                   | `08b36e4` answers from the commit's completion: 2.38× through the sidecar, 2.72× in process.       |
| What does it cost in memory?                                         | Rust's peak resident memory is 13–23 MiB in every case in process, Go's 13–73 MiB; with a page cache a connection, sixteen readers hold 54–57 MiB, Go's level.                                                                                                                                                                                                    | Nothing.                                                                                           |

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

| What                      | Commit or version                                                                                                                                                                                                                                               |
|---------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| TinyStore Rust            | `51b25920a9d14886f327789d2ad29e25c17539d1`, branch `rust`                                                                                                                                                                                                       |
| later commit              | `6e318652225a0720e24758f69ba22572e2f70e42`, branch `rust`: `08b36e4` and `6e31865` together                                                                                                                                                                     |
| the build's flag          | the later commit twice, with and without `-USQLITE_ENABLE_MEMORY_MANAGEMENT`; `1c85629695772baff9974e83f4d4cbdc60bfd2c8` on `rust` puts it in the build                                                                                                         |
| the commit's callers      | `1c85629…` before; `cbe4a9ef3804d3290793db7a56549a33b6de911f`, `2ecab48c6532cbd551ee51f1f8cc6a8cad962565` and `c729a30059d65d17db67b5308d3fb3125346ebc7` on `rust`, each on the one before                                                                      |
| the readers and the crowd | `63cdb4c38ec6ed37cfa5bd676eb8edc5eb3061a0`, `df103b63f42992718923f44d0c207ddb457ee567` and `760231eef9b14a2bd5d7b53f1a897076dcfba02e` on `rust`, each on the one before                                                                                         |
| the other writes, the log | `d568ede32e41cfaf59028aa4efe8cd4a6fed9d9b` and `5f944e25570f12a4ada9c9da230839e1e593a00e` on `rust`                                                                                                                                                             |
| the Bun program's thread  | `62393812b9df66f02dbf7ac598417161197ab4ab`, `d019713b89b10af9bff1675d252331ace5e45b87`, `3d9131593227484c9c4493eb160fba7f0a67f66c`, `6bb1f6a9562e20b2b9a3a9366f7f99da2649ba0c` and `597ecd5240f22cb45af67252fcfcaf52346389e8` on `rust`, each on the one before |
| TinyStore Go              | `e81a0503cb5ba2d12a359662ab0163578ddfe581`                                                                                                                                                                                                                      |
| Rust                      | rustc 1.99.0, the release profile, rusqlite 0.40.2 with its bundled SQLite                                                                                                                                                                                      |
| Go                        | go1.27.1, `CGO_ENABLED=0`, ncruces/go-sqlite3 v0.35.6                                                                                                                                                                                                           |
| Bun                       | 1.4.2                                                                                                                                                                                                                                                           |

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

| Run                                     | UTC, 10 October   | Rows | File                                                   |
|-----------------------------------------|-------------------|------|--------------------------------------------------------|
| the round                               | 01:26:02–01:35:54 | 126  | `runs.jsonl`                                           |
| reads by the callers at once            | 01:36:23–01:39:43 | 36   | `scaling.jsonl`                                        |
| the later commit, in process            | 01:47:40–01:53:37 | 72   | `next-native.jsonl`                                    |
| the later commit, through Bun           | 01:53:37–01:56:59 | 36   | `next-bun.jsonl`                                       |
| the build's flag, in process            | 02:28:44–02:39:13 | 126  | `pcache-native.jsonl`                                  |
| the build's flag, through Bun           | 02:39:13–02:44:52 | 45   | `pcache-bun.jsonl`                                     |
| the first of the callers' commits alone | 08:56:27–09:03:34 | 120  | `gather-native.jsonl`, `gather-bun.jsonl`              |
| the callers' commits, in process        | 09:12:43–09:20:32 | 132  | `writes-native.jsonl`                                  |
| the callers' commits, through Bun       | 09:20:32–09:22:22 | 30   | `writes-bun.jsonl`                                     |
| shares and halves                       | 09:36:15–09:39:53 | 72   | `halves-native.jsonl`                                  |
| the core's mutexes and the pool         | 10:02:23–10:15:02 | 156  | `readers-native.jsonl`                                 |
| a crowd through Bun                     | 10:23:43–10:36:38 | 120  | `crowd-bun.jsonl`                                      |
| a rented server, KVM                    | 10:15:42–10:21:50 | 72   | `kvm-reads.jsonl`, `kvm-writes.jsonl`                  |
| the leftovers' control, in process      | 10:48:23–10:54:23 | 72   | `crowd-native.jsonl`                                   |
| a connection's cache                    | 10:54:23–10:58:44 | 56   | `cache-native.jsonl`                                   |
| the rows' memory in Bun                 | 10:59:18–11:00:06 | 18   | `memory-bun.jsonl`                                     |
| reads inside Bun, and the bare loop     | 15:04:37–15:17:21 | 108  | `place-reads.jsonl`                                    |
| the other cases inside Bun              | 15:17:22–15:22:09 | 84   | `place-others.jsonl`                                   |
| the server beside Bun                   | 15:22:09–15:28:04 | 54   | `place-sidecar.jsonl`                                  |
| the last commits, in process            | 15:28:05–15:33:09 | 63   | `place-native.jsonl`                                   |
| the program's own bytes                 | 16:01:08–16:08:15 | 60   | `into-reads.jsonl`                                     |
| the program's own bytes, other cases    | 15:55:07–15:56:23 | 24   | `into-others.jsonl`                                    |
| commits that do not wait for the disk   | 15:56:23–16:00:05 | 72   | `os-writes.jsonl`                                      |
| the rented server, durability           | 15:26:55–15:35:09 | 36   | `kvm-durability-full.jsonl`, `kvm-durability-os.jsonl` |

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

# the Bun program's thread: trees built the same way, /perf/slice-then at 5f944e2, -linger at d019713,
# -place at 6bb1f6a, -into at 597ecd5; the same -e ALSO=then=/perf/slice-then,linger=/perf/slice-linger,place=/perf/slice-place,into=/perf/slice-into
# goes with each run below
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench -e RUNS=place-reads \
  -e ONLY=bun-go-sidecar,bun-rust-embedded-then,bun-rust-embedded-linger,bun-rust-embedded-place,bun-raw-then,bun-raw-place \
  -e CASES=kv-get:1,kv-get:4,kv-get:8,kv-get:16,kv-get:64,kv-get:256 golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench -e RUNS=place-others \
  -e ONLY=bun-go-sidecar,bun-rust-embedded-then,bun-rust-embedded-linger,bun-rust-embedded-place \
  -e CASES=sql-point:1,sql-point:8,sql-point:64,kv-set:64,jobs-add:64,sql-rows:1,nothing:64 golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench -e RUNS=place-sidecar \
  -e ONLY=bun-go-sidecar,bun-rust-sidecar-then,bun-rust-sidecar-place \
  -e CASES=kv-get:1,kv-get:8,kv-get:64,sql-point:1,sql-point:64,kv-set:64 golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench -e RUNS=place-native \
  -e ONLY=go,rust-then,rust-place \
  -e CASES=kv-get:1,kv-get:4,kv-get:64,sql-point:16,kv-set:64,jobs-add:64,jobs-drain:8 golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench -e RUNS=into-reads \
  -e ONLY=bun-go-sidecar,bun-rust-embedded-place,bun-rust-embedded-into,bun-raw-place,bun-raw-into \
  -e CASES=kv-get:1,kv-get:8,kv-get:64,kv-get:256 golang:1.27 python3 run.py
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench -e RUNS=into-others \
  -e ONLY=bun-rust-embedded-place,bun-rust-embedded-into \
  -e CASES=sql-point:1,sql-point:64,sql-rows:1,kv-set:64 golang:1.27 python3 run.py

# a commit that does not wait for the disk: /perf/slice-os is /perf/slice-place copied, the default of
# crates/tinystore/src/sqlite/config.rs made Durability::Os, and built again; -e ALSO=place=/perf/slice-place,os=/perf/slice-os
docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench -e RUNS=os-writes \
  -e ONLY=rust-place,rust-os,bun-rust-embedded-place,bun-rust-embedded-os \
  -e CASES=kv-set:1,kv-set:16,kv-set:64,kv-set:256,kv-set:1024,jobs-add:64 golang:1.27 python3 run.py

# the rented server's durability: /perf/slice/bin/go-slice, and /perf/slice-place's and /perf/slice-os's
# bin/rust-slice as rust-full and rust-os, copied into <dir>/bin with remote.sh beside it, then there
PASSES=2 sh remote.sh <dir> kvm-durability-full.jsonl "go=go-slice rust=rust-full" \
  "kv-set:1 kv-set:64 kv-set:256 kv-set:1024"
PASSES=2 sh remote.sh <dir> kvm-durability-os.jsonl "full=rust-full os=rust-os" \
  "kv-set:1 kv-set:16 kv-set:64 kv-set:256 jobs-add:64"

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

SQLite lets a program give it mutexes before it starts, through
`SQLITE_CONFIG_MUTEX`, and `63cdb4c` does: the core's own, which look at a
held lock 300 times before they park. A program that started SQLite itself
before the store opened keeps the ones it started with. `df103b6` takes the
last system call out of a read that waits for nothing: a reader that came
back woke a waiter whether one waited or not. The three commits one on
another, with Go, calls a second, each pass:

| Case       | In flight | go                        | before                          | the core's mutexes              | and the pool                    | last to before | last to go |
|------------|-----------|---------------------------|---------------------------------|---------------------------------|---------------------------------|----------------|------------|
| kv-get     | 1         | 122,565; 123,347; 120,746 | 213,336; 206,306; 203,468       | 210,196; 211,871; 196,689       | 223,494; 217,261; 212,544       | 1.05×          | 1.77×      |
| kv-get     | 2         | 218,287; 225,148; 215,619 | 146,471; 148,243; 146,202       | 480,488; 473,783; 477,209       | 552,187; 546,458; 535,693       | 3.73×          | 2.50×      |
| kv-get     | 4         | 373,611; 353,377; 341,201 | 422,718; 415,962; 414,264       | 879,700; 875,453; 875,565       | 943,822; 963,475; 987,231       | 2.32×          | 2.73×      |
| kv-get     | 8         | 375,738; 395,284; 422,016 | 1,449,218; 1,509,694; 1,564,929 | 1,650,322; 1,673,734; 1,761,126 | 1,820,167; 1,863,736; 1,872,380 | 1.23×          | 4.71×      |
| kv-get     | 16        | 425,752; 455,243; 455,686 | 2,134,732; 2,137,493; 2,197,882 | 1,938,812; 2,035,566; 2,214,684 | 2,293,282; 2,002,745; 2,366,503 | 1.07×          | 5.04×      |
| kv-get     | 64        | 534,558; 530,128; 554,889 | 1,610,052; 1,706,032; 1,730,234 | 1,647,226; 1,708,880; 1,774,600 | 1,672,532; 1,643,916; 1,352,276 | 0.96×          | 3.08×      |
| sql-point  | 2         | 286,457; 281,778; 268,223 | 138,189; 133,763; 140,763       | 553,837; 540,188; 550,561       | 620,411; 613,831; 622,913       | 4.49×          | 2.20×      |
| sql-point  | 4         | 453,135; 436,449; 443,508 | 359,433; 359,327; 355,055       | 1,003,753; 1,009,250; 997,452   | 1,105,973; 1,085,005; 1,060,274 | 3.02×          | 2.45×      |
| sql-point  | 16        | 516,199; 465,430; 473,800 | 2,253,145; 2,049,842; 2,209,728 | 2,378,190; 2,305,858; 2,209,760 | 2,529,442; 2,371,078; 2,405,401 | 1.09×          | 5.08×      |
| kv-set     | 64        | 17,506; 17,426; 17,292    | 18,922; 18,745; 18,897          | 19,055; 19,033; 19,204          | 19,123; 18,620; 18,813          | 1.00×          | 1.08×      |
| jobs-add   | 64        | 23,769; 22,981; 23,734    | 24,346; 24,631; 24,363          | 24,278; 24,506; 24,280          | 24,543; 24,662; 24,192          | 1.01×          | 1.03×      |
| sql-rows   | 1         | 69.9; 70.2; 69.4          | 155; 153; 152                   | 150; 153; 154                   | 157; 154; 153                   | 1.01×          | 2.20×      |
| jobs-drain | 8         | 4,621; 4,639; 4,613       | 8,325; 8,385; 8,257             | 8,240; 8,383; 8,312             | 8,366; 8,396; 8,105             | 1.00×          | 1.81×      |

Two readers read 3.7× of what they did through kv and 4.5× through sql, four
2.3× and 3.0×, and one to four now read in a line: 217,000, 546,000 and
963,000 keys a second. Sixteen and 64 callers, the writes and the rows are as
before.

## A crowd of calls on one connection

Through Bun with 64 calls in flight everything stayed behind Go's sidecar,
and the Bun process was not the bound: with the Rust server beside it it
used a third of a processor. The bound was how a connection hands its calls
to the store's sixteen threads. Each call queued woke a thread, a system call
a call on the one thread that reads the connection, which inside Bun is the
program's own; and a point read, answered where it is read, took its turn on
that one thread however many waited behind it.

`760231e` wakes one thread for the calls queued, which wakes the next when
calls still wait, as a commit's callers wake one another. A point read with
four requests waiting behind it goes to those threads, and one alone is
still answered where it is read. Calls a second, each pass:

| Case      | In flight | Go beside                 | Rust beside, before    | Rust beside, after        | Rust inside, before    | Rust inside, after        | beside, after to Go | inside, after to before | inside, after to Go |
|-----------|-----------|---------------------------|------------------------|---------------------------|------------------------|---------------------------|---------------------|-------------------------|---------------------|
| kv-get    | 1         | 6,298; 6,263; 6,198       | 9,584; 9,293; 9,341    | 9,591; 9,166; 9,168       | 74,103; 68,856; 68,787 | 68,576; 61,870; 66,330    | 1.46×               | 0.96×                   | 10.59×              |
| kv-get    | 4         | 22,014; 23,545; 22,548    | 25,394; 28,001; 24,825 | 25,861; 25,079; 26,787    | 68,120; 70,313; 68,187 | 71,391; 75,177; 70,690    | 1.15×               | 1.05×                   | 3.17×               |
| kv-get    | 16        | 64,181; 65,411; 64,696    | 54,765; 55,263; 55,944 | 73,876; 68,795; 72,926    | 78,840; 82,329; 82,698 | 86,766; 78,888; 82,418    | 1.13×               | 1.00×                   | 1.27×               |
| kv-get    | 64        | 107,373; 101,629; 106,047 | 78,004; 71,438; 76,786 | 117,092; 112,059; 115,208 | 83,132; 74,473; 83,412 | 116,902; 104,231; 118,141 | 1.09×               | 1.41×                   | 1.10×               |
| sql-point | 64        | 87,492; 89,132; 86,621    | 60,595; 58,814; 60,251 | 90,919; 96,159; 100,346   | 44,672; 42,857; 45,676 | 111,797; 106,868; 113,281 | 1.10×               | 2.50×                   | 1.28×               |
| kv-set    | 64        | 14,089; 14,156; 14,217    | 12,540; 12,562; 12,738 | 15,338; 15,098; 15,709    | 13,953; 13,691; 13,792 | 16,803; 16,889; 16,358    | 1.08×               | 1.22×                   | 1.19×               |
| jobs-add  | 64        | 18,742; 18,887; 18,967    | 14,402; 13,960; 14,366 | 19,402; 19,840; 19,573    | 15,745; 15,734; 15,711 | 20,675; 20,984; 20,350    | 1.04×               | 1.31×                   | 1.09×               |
| sql-rows  | 1         | 18.4; 18.7; 18.9          | 30.3; 31.0; 30.2       | 31.8; 29.3; 30.8          | 42.0; 42.2; 41.9       | 41.3; 42.9; 40.9          | 1.65×               | 0.98×                   | 2.21×               |

A call at a time is within its passes' spread, 0.96× inside Bun. With 64 in
flight, kv's point read through the sidecar and inside Bun, the sql point
read, the writes and the jobs are all past Go's sidecar, 1.04–1.28× of it,
where they had been 0.5–0.95×.

## A rented server

The round's host is Windows with Linux under its hypervisor, where waking a
thread costs tens of microseconds, and three of the round's findings are
about wakes. A rented server says what they are worth elsewhere: four
processors of an AMD EPYC 7502P under KVM, Linux 7.0, glibc 2.43, a server
that does other work, load 0.3 before the run. The programs are the round's
own binaries, copied there and run without a container by `remote.sh`; three
passes of three seconds.

| Case   | In flight | go                        | before                    | the core's mutexes        | and the pool              | last to before | last to go |
|--------|-----------|---------------------------|---------------------------|---------------------------|---------------------------|----------------|------------|
| kv-get | 1         | 44,271; 46,681; 46,206    | 111,098; 112,925; 112,527 | 114,418; 107,641; 112,716 | 123,228; 112,939; 107,824 | 1.00×          | 2.44×      |
| kv-get | 2         | 74,072; 64,202; 80,515    | 171,692; 151,003; 138,150 | 189,574; 161,738; 206,320 | 190,697; 197,738; 225,857 | 1.31×          | 2.67×      |
| kv-get | 4         | 101,311; 107,255; 114,318 | 257,495; 274,270; 264,368 | 253,188; 278,529; 274,315 | 256,925; 291,379; 285,557 | 1.08×          | 2.66×      |

| Case   | In flight | go                     | the leader wakes each  | callers wake callers   | last to before | last to go |
|--------|-----------|------------------------|------------------------|------------------------|----------------|------------|
| kv-set | 1         | 988; 943; 1,079        | 1,440; 1,129; 1,017    | 1,361; 1,282; 1,215    | 1.14×          | 1.30×      |
| kv-set | 4         | 3,109; 2,335; 2,571    | 3,894; 3,762; 3,036    | 3,646; 3,147; 3,462    | 0.92×          | 1.35×      |
| kv-set | 16        | 6,504; 6,092; 6,149    | 6,944; 8,272; 7,756    | 7,648; 7,806; 8,987    | 1.01×          | 1.27×      |
| kv-set | 64        | 10,643; 11,192; 10,297 | 12,140; 12,727; 11,662 | 13,828; 12,134; 11,513 | 1.00×          | 1.14×      |

The readers' mutexes are worth 1.31× to two readers there and 1.08× to four,
and the commit before them did not fall below one reader's rate at two. The
callers waking one another are worth nothing to 64 writers there, in either
direction. So the size of both findings is this host's, and what holds
elsewhere is their sign: neither costs anything, and the mutexes still gain.
The Rust core is ahead of Go on that server in every row, 2.4–2.7× on reads
and 1.1–1.35× on writes. Four processors shared with other work make a noisy
run: the passes spread by a fifth.

## The round's leftovers

Three things the round had found and left were closed before its last part.

**The other writes.** `d568ede` answers kv's `clear` and `tx`, a durable
counter's add, a quota's use and a write sent through `sql.query` from their
commit's completion, as jobs' adds were: none holds a thread of its session
while its group waits. Two tests send more such writes than a session has
threads while a transaction holds the file's writer, and a read sent after
them is still answered. `5f944e2` says once in the log when the SQLite linked
in shares a page cache or keeps its own mutexes, for a program that builds
the crate without the workspace's flags. Neither moves a case of the round.
The three commits one on another, in process, calls a second, each pass:

| Case       | In flight | go                        | `df103b6`                       | `760231e`                       | `5f944e2`                       | last to first |
|------------|-----------|---------------------------|---------------------------------|---------------------------------|---------------------------------|---------------|
| kv-get     | 1         | 121,032; 122,779; 120,766 | 223,750; 224,969; 217,492       | 222,984; 218,181; 223,206       | 212,214; 219,638; 218,690       | 0.98×         |
| kv-get     | 4         | 355,531; 363,969; 353,150 | 983,136; 973,962; 969,725       | 940,146; 959,516; 970,355       | 937,273; 947,342; 960,309       | 0.97×         |
| kv-get     | 64        | 505,869; 542,240; 541,289 | 1,565,830; 1,684,361; 1,742,819 | 1,636,921; 1,536,109; 1,667,770 | 1,737,912; 1,696,891; 1,685,861 | 1.01×         |
| kv-set     | 64        | 17,234; 17,250; 17,181    | 19,147; 18,835; 19,067          | 18,849; 19,009; 19,099          | 19,094; 19,008; 19,094          | 1.00×         |
| jobs-add   | 64        | 23,442; 23,627; 23,429    | 24,303; 24,163; 24,187          | 24,429; 24,250; 24,458          | 24,315; 24,444; 24,368          | 1.01×         |
| jobs-drain | 8         | 4,553; 4,577; 4,527       | 7,997; 8,465; 8,193             | 8,243; 8,243; 8,268             | 8,264; 8,189; 8,356             | 1.01×         |

**A connection's cache.** `5f944e2` built with 4 MiB and 16 MiB of cache a
connection in place of 1, and with its readers reading through a memory map
of 256 MiB: medians of two passes, each beside the first column's, and each
program's peak resident memory in the columns' order.

| Case      | Callers | 1 MiB     | 4 MiB             | 16 MiB            | 1 MiB, readers mapped | peak resident, MiB |
|-----------|---------|-----------|-------------------|-------------------|-----------------------|--------------------|
| kv-get    | 1       | 219,442   | 236,030 (1.08×)   | 270,946 (1.23×)   | 280,329 (1.28×)       | 16; 22; 48; 32     |
| kv-get    | 4       | 951,962   | 1,023,587 (1.08×) | 1,048,061 (1.10×) | 1,027,421 (1.08×)     | 27; 44; 106; 92    |
| kv-get    | 16      | 2,302,448 | 2,198,145 (0.95×) | 2,021,020 (0.88×) | 2,454,689 (1.07×)     | 54; 107; 316; 313  |
| sql-point | 16      | 2,434,029 | 2,331,488 (0.96×) | 2,094,488 (0.86×) | 2,402,588 (0.99×)     | 53; 105; 261; 224  |
| sql-rows  | 1       | 154       | 171 (1.11×)       | 167 (1.09×)       | 154 (1.00×)           | 16; 21; 21; 16     |
| kv-set    | 64      | 18,972    | 19,117 (1.01×)    | 18,954 (1.00×)    | 19,105 (1.01×)        | 13; 13; 19; 13     |
| jobs-add  | 64      | 24,440    | 24,223 (0.99×)    | 24,189 (0.99×)    | 24,093 (0.99×)        | 13; 14; 17; 13     |

A larger cache reads a key 8–23% faster for one caller and 5–12% slower for
sixteen, and its memory is by the connection: sixteen readers with 16 MiB
each are 316 MiB where they were 54. A map gives sixteen callers 7% for as
much. The tenth that jobs' adds had seemed to lose to a cache each is not
there. The cache stays at 1 MiB.

**The Bun process's memory for 25,000 rows.** The round's loop of reads, and
the same reads with a collection forced after each, three passes:

| Program     | reads a second   | peak resident, MiB | with a collection after each read: reads a second | resident after it, MiB | JS heap after it, MiB |
|-------------|------------------|--------------------|---------------------------------------------------|------------------------|-----------------------|
| Go beside   | 18.7; 18.8; 19.0 | 82; 83; 79         | 17.6; 17.5; 17.6                                  | 86; 87; 85             | 10.4; 10.4; 10.4      |
| Rust beside | 30.8; 31.0; 31.2 | 102; 121; 103      | 25.7; 24.4; 25.6                                  | 89; 90; 87             | 8.4; 8.4; 8.4         |
| Rust inside | 43.3; 42.0; 41.9 | 136; 140; 138      | 33.3; 31.0; 31.0                                  | 106; 106; 116          | 9.0; 9.0; 9.0         |

After a collection the JS heap is 8–10 MiB whichever core answered: a result
that is dropped holds nothing. With a collection after each read the Bun
process beside either server is 85–90 MiB, where the loop without one peaked
at 80 beside Go's and 103 beside Rust's: the peak follows how fast a loop
makes garbage, 19 reads a second against 31. With the core inside, the
process is 106–116 MiB, since the core's memory is its own, which a sidecar
keeps in another process. The row's `result_mib` reads 0 in every pass, the
heap with a result held read as the heap without: Bun's count does not move
between two readings a collection apart, and the report does not use it.

## The Bun program's one thread

Through Bun with 64 reads in flight the round had reached 117,000 a second
with the core inside, where a Rust program's threads read 1.7 million. A
Bun program has one thread, and a profile of whole stacks says what it did
with it at `5f944e2`: it was on a processor 93% of the time, 39% of its
samples under the core's read of a key, which the session ran where it read
the request; each of the store's sixteen workers was on one 7% of the time,
and a fifth of their samples were in `futex_wake`. The session sent a point
read to a worker only with four requests waiting behind it, so the last four
of every write were read by the program's thread, and a write of the SDK
held twelve calls. And a worker slept the moment it found no job: a query
for one row, which always runs on a worker, took 56 microseconds with one in
flight, most of it the wake.

`6239381` keeps one worker looking for the next job for 200 microseconds
after the queue empties, while jobs come closer together than that: the job
that comes is its own, with no wake and no thread started. `d019713` sends
a crowd's point reads to the workers whole, six requests or more in one
read of the connection, and each point read after it while the workers still
have reads of that connection: the host comes back for those answers anyway.
A read alone, with nothing of its connection away, is answered where it is
read, as before. The linger and the crowd's size come from looks, single
runs kept in `looks-bun.txt`: six is where the workers begin to win on this
host, five in flight being as fast read in place, and 200 microseconds reads
as fast inside Bun as 100 and half as fast again beside it, where a round
trip is longer.

The SDK's share was found in a profile of its JavaScript: 13% of the
thread's samples were in asking a small array for its buffer, which makes
the engine move the array to one, for every message written and read.
`3d91315` writes and reads a message's bytes without a view of them.
`6bb1f6a` makes a bucket's call in place: its body is written into the
bytes that leave next, its answer read where it arrived, for one promise
where a call through a stream made five and copied its body three times.

Four commits one on another, and a bare loop over the C functions with no
SDK, on the core before and after. Calls a second, each pass:

| Case   | In flight | Go beside                 | Rust inside, before       | the core's two commits    | and the SDK's two         | bare loop, before         | bare loop, after          | last SDK to before | to Go  | to the bare loop |
|--------|-----------|---------------------------|---------------------------|---------------------------|---------------------------|---------------------------|---------------------------|--------------------|--------|------------------|
| kv-get | 1         | 6,311; 6,367; 6,421       | 68,800; 70,846; 73,583    | 70,790; 75,012; 76,168    | 114,303; 114,162; 109,597 | 156,016; 155,638; 143,961 | 149,682; 154,260; 146,939 | 1.61×              | 17.93× | 0.76×            |
| kv-get | 4         | 22,749; 23,504; 22,687    | 71,778; 72,242; 77,405    | 68,740; 71,381; 79,167    | 105,277; 109,622; 115,775 | 164,532; 162,932; 169,045 | 163,195; 160,018; 172,407 | 1.52×              | 4.82×  | 0.67×            |
| kv-get | 8         | 43,251; 42,831; 44,552    | 68,201; 67,789; 67,704    | 83,731; 95,131; 91,243    | 179,580; 184,960; 176,482 | 145,028; 144,386; 133,117 | 298,036; 250,256; 307,593 | 2.65×              | 4.15×  | 0.60×            |
| kv-get | 16        | 64,638; 67,355; 66,911    | 83,740; 87,578; 86,196    | 125,966; 122,853; 121,746 | 251,458; 248,798; 235,637 | 256,497; 262,390; 244,090 | 478,988; 437,931; 384,510 | 2.89×              | 3.72×  | 0.57×            |
| kv-get | 64        | 99,328; 109,429; 108,847  | 120,324; 117,699; 112,499 | 147,487; 141,075; 141,317 | 288,986; 287,548; 299,831 | 334,759; 307,641; 324,688 | 504,925; 547,385; 475,841 | 2.46×              | 2.65×  | 0.57×            |
| kv-get | 256       | 120,641; 122,270; 107,838 | 139,575; 128,493; 131,975 | 140,110; 146,026; 133,288 | 341,141; 331,171; 309,749 | 461,846; 480,644; 478,858 | 791,435; 724,754; 612,942 | 2.51×              | 2.75×  | 0.46×            |

The harness changed before these runs, for all its programs alike. It chose
a key through a bigint, spelled it, and read the clock three times a call,
which cost a call more than the SDK did; it now does the same arithmetic in
halves of 32 bits, spells the keys before the clock starts, and reads the
clock once. Its loops alone, the case `nothing` below, make 3 million a
second. So the Bun rates of these runs are not the earlier sections': the
programs of one run compare.

The other cases, inside Bun:

| Case      | In flight | Go beside                       | Rust inside, before             | the core's two commits          | and the SDK's two               | core to before | last to before | last to Go |
|-----------|-----------|---------------------------------|---------------------------------|---------------------------------|---------------------------------|----------------|----------------|------------|
| sql-point | 1         | 6,148; 6,150; 6,054             | 15,819; 15,803; 15,879          | 55,758; 60,701; 57,743          | 69,771; 72,755; 75,037          | 3.65×          | 4.60×          | 11.83×     |
| sql-point | 8         | 41,264; 41,847; 42,331          | 69,419; 66,692; 71,245          | 76,242; 70,413; 68,104          | 100,230; 113,226; 100,693       | 1.01×          | 1.45×          | 2.41×      |
| sql-point | 64        | 89,725; 98,287; 90,856          | 109,991; 120,525; 112,257       | 117,065; 112,894; 116,060       | 175,951; 183,729; 169,037       | 1.03×          | 1.57×          | 1.94×      |
| kv-set    | 64        | 14,266; 14,673; 14,449          | 17,352; 16,881; 17,450          | 17,405; 17,004; 17,086          | 17,927; 18,442; 18,590          | 0.98×          | 1.06×          | 1.28×      |
| jobs-add  | 64        | 18,569; 19,576; 19,324          | 21,236; 21,244; 21,328          | 21,083; 21,303; 21,145          | 22,211; 23,094; 22,900          | 1.00×          | 1.08×          | 1.19×      |
| sql-rows  | 1         | 20.2; 20.1; 20.3                | 42.8; 43.9; 40.1                | 42.6; 44.0; 41.8                | 43.1; 43.2; 41.8                | 1.00×          | 1.01×          | 2.13×      |
| nothing   | 64        | 2,890,500; 3,320,265; 2,996,179 | 3,032,697; 3,271,973; 3,256,181 | 2,957,838; 3,435,986; 3,154,993 | 2,925,390; 3,264,993; 3,374,982 | 0.97×          | 1.00×          | 1.09×      |

A query for one row at a time is 3.7× with the core's two commits alone, 13
microseconds where it took 56: its worker no longer sleeps between two.
Writes and the rows are as before.

Beside Bun, through the server, which has the same session:

| Case      | In flight | Go beside                 | Rust beside, before       | Rust beside, after        | after to before | after to Go |
|-----------|-----------|---------------------------|---------------------------|---------------------------|-----------------|-------------|
| kv-get    | 1         | 6,385; 6,137; 6,335       | 9,826; 9,264; 9,672       | 10,989; 11,359; 11,230    | 1.16×           | 1.77×       |
| kv-get    | 8         | 42,544; 42,036; 43,279    | 56,146; 57,369; 60,450    | 72,114; 68,972; 66,441    | 1.20×           | 1.62×       |
| kv-get    | 64        | 103,177; 113,901; 103,604 | 116,147; 124,135; 124,450 | 213,179; 213,778; 214,289 | 1.72×           | 2.06×       |
| sql-point | 1         | 6,022; 5,997; 6,058       | 5,589; 5,645; 5,584       | 6,580; 6,835; 6,713       | 1.20×           | 1.11×       |
| sql-point | 64        | 90,448; 93,119; 91,315    | 99,903; 112,001; 102,990  | 126,642; 144,497; 121,822 | 1.23×           | 1.39×       |
| kv-set    | 64        | 13,550; 13,817; 13,409    | 15,311; 15,261; 15,365    | 17,196; 17,897; 17,781    | 1.16×           | 1.31×       |

And in process, where no session is:

| Case       | In flight | go                        | before                          | after                           | after to before | after to go |
|------------|-----------|---------------------------|---------------------------------|---------------------------------|-----------------|-------------|
| kv-get     | 1         | 123,628; 121,771; 124,348 | 223,940; 219,339; 217,409       | 211,825; 222,817; 205,441       | 0.97×           | 1.71×       |
| kv-get     | 4         | 371,248; 365,738; 242,718 | 965,078; 973,785; 856,012       | 931,111; 972,266; 896,783       | 0.96×           | 2.55×       |
| kv-get     | 64        | 527,911; 530,218; 508,691 | 1,410,542; 1,715,917; 1,713,994 | 1,520,266; 1,685,592; 1,699,188 | 0.98×           | 3.19×       |
| sql-point  | 16        | 500,707; 466,743; 508,113 | 2,408,208; 2,324,318; 2,478,572 | 2,386,364; 2,459,482; 2,553,345 | 1.02×           | 4.91×       |
| kv-set     | 64        | 17,389; 17,238; 17,008    | 19,078; 19,004; 18,910          | 18,919; 19,088; 19,109          | 1.00×           | 1.11×       |
| jobs-add   | 64        | 22,895; 23,632; 23,114    | 23,902; 24,032; 24,730          | 24,224; 24,078; 24,660          | 1.01×           | 1.05×       |
| jobs-drain | 8         | 4,577; 4,562; 4,574       | 8,164; 8,135; 8,058             | 8,182; 8,145; 8,129             | 1.00×           | 1.78×       |

## The core's bytes in the program's own

The C functions handed the host a buffer of the core's each call, which
bun:ffi wrapped, copied and gave back: three crossings, and two buffers
made and freed, for a write of a few calls. `597ecd5` turns it round. The
host gives the bytes its frames are in and the bytes the core's are written
into, and the core keeps neither past the call; what it writes is a stream,
a frame ending in the next read where it does not fit. The functions are
four, and none frees. The SDK reads an answer in the bytes the core wrote
it into, before its next call writes them over. Reads a second, each pass:

| Case   | In flight | Go beside                 | Rust inside, before       | Rust inside, after        | bare loop, before         | bare loop, after                | after to before | to Go  | bare loop, after to before | SDK to the bare loop |
|--------|-----------|---------------------------|---------------------------|---------------------------|---------------------------|---------------------------------|-----------------|--------|----------------------------|----------------------|
| kv-get | 1         | 6,222; 6,351; 6,436       | 69,439; 112,893; 109,962  | 94,402; 129,362; 129,903  | 128,235; 156,362; 150,573 | 163,469; 172,415; 163,391       | 1.18×           | 20.37× | 1.09×                      | 0.79×                |
| kv-get | 8         | 42,929; 41,137; 42,777    | 175,389; 156,213; 186,067 | 239,628; 217,957; 209,000 | 314,264; 295,141; 318,453 | 370,825; 349,705; 346,923       | 1.24×           | 5.10×  | 1.11×                      | 0.62×                |
| kv-get | 64        | 119,552; 104,865; 112,364 | 317,984; 292,506; 278,317 | 364,089; 333,872; 348,305 | 544,220; 452,142; 528,454 | 951,960; 934,650; 948,886       | 1.19×           | 3.10×  | 1.80×                      | 0.37×                |
| kv-get | 256       | 122,288; 122,863; 124,396 | 362,704; 348,369; 321,570 | 384,687; 378,821; 384,719 | 680,984; 621,927; 845,056 | 1,214,385; 1,086,176; 1,150,037 | 1.10×           | 3.13×  | 1.69×                      | 0.33×                |

The columns before read as the section above's, within 5%. Other cases,
inside Bun, each pass:

| Case      | In flight | before                    | after                     | after to before |
|-----------|-----------|---------------------------|---------------------------|-----------------|
| sql-point | 1         | 70,203; 69,451; 72,490    | 82,469; 79,633; 83,025    | 1.17×           |
| sql-point | 64        | 173,784; 161,354; 180,021 | 166,458; 164,507; 179,520 | 0.96×           |
| sql-rows  | 1         | 40.3; 39.5; 42.1          | 44.2; 41.0; 42.7          | 1.06×           |
| kv-set    | 64        | 18,503; 18,888; 18,943    | 18,409; 18,847; 18,803    | 1.00×           |

A query for one row at a time gains as a read does. With 64 in flight, the
rows and the writes, the change is within a pass's spread.

## A commit that does not wait for the disk

A durable write waits for the disk, a sync of 1.8 milliseconds in the
section on shared commits, and a file has one writer: the round's 19,000
writes a second by 64 callers are 46 writes a sync. The plan has a second
mode for a file, `'os'`: WAL with `synchronous=NORMAL`, a commit written to
the operating system and synced at checkpoints, which survives the
program's crash and may lose its last commits to a power loss. The adapter
had it and no engine offered it. To see what it is worth, `6bb1f6a` was
built with it as every file's default. Writes a second, each pass:

| Case     | Writers | in process, `'full'`   | in process, `'os'`     | through Bun, `'full'`  | through Bun, `'os'`    | in process | through Bun |
|----------|---------|------------------------|------------------------|------------------------|------------------------|------------|-------------|
| kv-set   | 1       | 479; 197; 197          | 27,672; 18,831; 18,732 | 457; 215; 214          | 24,303; 16,823; 17,085 | 95.66×     | 79.64×      |
| kv-set   | 16      | 7,848; 4,034; 4,225    | 26,284; 19,120; 20,121 | 7,604; 4,149; 3,968    | 25,853; 19,303; 19,144 | 4.76×      | 4.65×       |
| kv-set   | 64      | 18,603; 11,329; 12,011 | 30,086; 22,242; 23,106 | 18,493; 11,475; 11,001 | 30,155; 22,173; 21,572 | 1.92×      | 1.93×       |
| kv-set   | 256     | 25,613; 19,832; 20,639 | 28,817; 23,037; 24,193 | 25,409; 19,338; 19,583 | 29,997; 22,434; 22,849 | 1.17×      | 1.17×       |
| kv-set   | 1024    | 34,051; 28,461; 28,282 | 34,051; 27,909; 30,351 | 34,048; 28,305; 28,573 | 33,933; 29,037; 28,578 | 1.07×      | 1.02×       |
| jobs-add | 64      | 14,240; 14,188; 14,126 | 66,816; 65,487; 66,268 | 13,282; 13,035; 13,227 | 59,505; 58,536; 63,419 | 4.67×      | 4.50×       |

Every program's first pass of the sets is faster than its next two, in
either order: one durable writer's write took 1.7 milliseconds at the
median in the first and 5.5 to 5.9 in the others, so this host's disk syncs
slower after a minute of syncs. A run before this one, not kept, did the
same. A queue's adds did not move. The ratios are of medians, the later
passes'.

On the rented server of the section above, whose disk syncs one writer's
write in 0.7 to 0.9 milliseconds, two passes, in process:

| Case     | Writers | `'full'`       | `'os'`         | `'os'` to `'full'` |
|----------|---------|----------------|----------------|--------------------|
| kv-set   | 1       | 837; 652       | 15,047; 17,358 | 21.76×             |
| kv-set   | 16      | 7,203; 6,965   | 12,559; 12,298 | 1.75×              |
| kv-set   | 64      | 13,230; 12,626 | 15,274; 15,411 | 1.19×              |
| kv-set   | 256     | 13,477; 12,546 | 13,610; 14,442 | 1.08×              |
| jobs-add | 64      | 10,898; 12,176 | 22,704; 20,916 | 1.89×              |

One writer writes 53 to 96 times as many keys on this host, as its sync
takes 1.7 to 5.9 milliseconds, and 22 times on the server: its write takes
13 microseconds in process here, 17 through Bun and 23 on the server, where
it waited for a sync. With more writers the gain goes, and past some 30,000
a second here, 15,000 on the server's four processors, the modes are one: a
file's writer is then the bound, not the disk. A queue's adds reach 66,000
here and 22,000 there; why a key's set makes less than half of that was not
looked into.

More durable writers than the round's 64 do raise its rate, the group
growing with them. On the server, two passes:

| Case   | Writers | go             | rust           | rust to go |
|--------|---------|----------------|----------------|------------|
| kv-set | 1       | 726; 872       | 974; 1,323     | 1.44×      |
| kv-set | 64      | 8,323; 10,791  | 13,008; 11,882 | 1.30×      |
| kv-set | 256     | 13,645; 12,132 | 13,458; 14,780 | 1.10×      |
| kv-set | 1024    | 14,965; 14,229 | 17,916; 17,818 | 1.22×      |

The engines offer it since `9cb490f`, which this round did not measure: a
store opened with `durability: 'os'` gives it to `kv.db`, `jobs.db` and
every database that does not say its own, and a database may say its own.

## Tried and left out

The looks are single runs, in `looks-bun.txt`.

- **No wake for what a write returns.** A read answered before its write
  returns also called the host's wake, which then read nothing. Leaving the
  wake out changed no rate.
- **One wake for half a crowd.** The host woken only once as many of a
  crowd's answers are ready as reads are still with the workers: built with
  its test, measured, and taken out. A busy host takes most answers with its
  next write, not by a wake: a write of the SDK holds 12 calls with 64 in
  flight and 61 with 256.
- **A get that reads only its value, and a message written in one pass** are
  in `6bb1f6a`, and changed no rate measured.
- **One thread for Bun's collector.** With `BUN_JSC_numberOfGCMarkers=1` the
  SDK read 370,000 keys a second where it read 311,000, and its p99 fell
  from 2.0 to 0.8 milliseconds: the collector wakes its helpers for every
  collection, and a wake is what this host makes dear. It is the runtime's
  setting, not the SDK's.

## What follows

Nothing below is built.

- **What a file's one writer costs a small write.** Past the disk it makes
  some 30,000 sets a second where it makes 66,000 adds of a queue.
- **The SDK's own work a call.** It is at 37% of the bare loop with 64 in
  flight. What is left is spread thin: a message's fields
  walked through a table, a promise's closures, a map of streams. Codecs
  written out straight by the generator would take the first of them, in
  every language's SDK.
- **A read that waits while thousands pass.** In a run of 1.3 million reads
  with 256 in flight, one stayed unanswered while 4,096 later ones were. A
  reader that comes back goes to whoever asks first, not to who waited
  longest. It is a tail and no rate, and nothing here measured it apart.

- **The last writes a sync could carry**: 46 of 64 on both sides.
- **Latencies.** The raw rows carry each case's p50 and p99. They are not
  quoted here: a tail measured under WSL2 says more of the host than of the
  store. They wait for bare Linux.
