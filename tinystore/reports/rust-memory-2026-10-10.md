# What a store holds in memory — 2026-10-10

The memory of the Rust core's store beside the Go engines' on the host of the
[slice round](rust-slice-2026-10-10.md): a program's own process before its
store opens, once open, right after a case and after 130 seconds of rest with
the store still open, in process, through Bun with the core inside, and the
server beside Bun.

A Rust program with its store open holds 3.4 MiB where Go's holds 5.5; Rust's
server, started, 5.8 where Go's holds 11. After a case of 64 callers and a
rest, Rust's process holds less than Go's or as much in every case but one,
and its server 1.3 to 3.1 times less. The one was a bug: an SQL database never
closed the readers a burst of reads opened, as kv and jobs do after a minute,
and `78fb22e` closes them. What they held stayed with the process anyway:
glibc keeps freed memory for later, and handing it back takes another 3 to 9
MiB from the Rust process after a rest of reads. With it handed back on both sides,
Rust holds less than Go in every case.

| Question                               | Observation                                                                                                                                                                    | What follows                                                                  |
|----------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------|
| What does an open store cost?          | Rust's process 3.4 MiB with its store open, 0.3 more than without; Go's 5.5. Rust's server 5.8, Go's 11.                                                                       | Nothing.                                                                      |
| And after a case and a rest?           | In process Rust holds 20 on kv where Go holds 22–24, 16 to Go's 17–19 after 25,000 rows, 10–11 to Go's 17–19 after jobs; 25 to Go's 20 after sql point reads.                  | The sql reads: the row below.                                                 |
| Why did sql reads keep their memory?   | A database's readers never closed: kv and jobs close theirs in their maintenance, and sql had none. With them swept, the Rust program holds 23 where it held 25.               | `78fb22e`.                                                                    |
| Where does the rest go?                | glibc keeps freed memory: `malloc_trim` hands back 3–9 MiB after the rest of reads, where Go's `FreeOSMemory` hands back 2–5. After it Rust holds 10–16 MiB to Go's 14–20.     | Give memory back once a sweep closed readers, where the allocator is glibc's. |
| And the server beside Bun?             | Rust's holds 10–28 MiB after a rest, Go's 25–41; after a case 10–28 to Go's 37–53.                                                                                             | Nothing.                                                                      |
| What does the core cost a Bun program? | Bun holds 42 MiB, 60 with the SDK loaded and a store open whichever way it reaches the store; with the core inside it holds about what Bun and a server beside it do together. | Nothing.                                                                      |

## Environment and reproduction

Everything ran in one evening on one AMD Ryzen 7 7700 (8 cores, 16 logical
processors) through Docker Desktop on WSL2: Linux 6.18.33.2, glibc 2.41, the
`golang:1.27` image, the stores on a Docker volume on WSL2's ext4. Memory is
less the host's than a rate is, but glibc's is this host's allocator: a
program on musl, or on another allocator, keeps another amount.

| What           | Commit or version                                                                                                                                                                                   |
|----------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| TinyStore Rust | `51d7eeed44fece735e205d9219867d9bc56319ed`, branch `rust`; `78fb22ef623ebe462415586ee4546182a85cf9e8`, on it, for the sql cases swept and the memory given back                                     |
| TinyStore Go   | `e81a0503cb5ba2d12a359662ab0163578ddfe581`                                                                                                                                                          |
| the harness    | research `c460c4f66510c24076ad57a04a48b8b9ab5d40a2`; `94d795a98157b5170d7174f1b333aa3c33c3abc2` for the runs of some programs, `09f2e7f4d278ddeec30166b34fbedd082a3c2cc5` for the memory given back |
| Rust           | rustc 1.99.0, the release profile, rusqlite 0.40.2 with its bundled SQLite, `SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT -DHAVE_FDATASYNC=1`                                                     |
| Go             | go1.27.1, `CGO_ENABLED=0`, ncruces/go-sqlite3 v0.35.6                                                                                                                                               |
| Bun            | 1.4.2                                                                                                                                                                                               |

The programs are [slice-bench](../slice-bench/README.md)'s, given `--rest`:
each reads its own `/proc/self/smaps_rollup` before its store opens, once it
is open, right after its case and after the rest, and the Bun program reads
the server's beside it, the process with a file of its store open. A program
that rests keeps no latencies, which it would hold. [`memory.py`](../slice-bench/memory.py)
loads one case at a time, and a program rests while the next ones load: a rest
is idle, and what a process holds is its own. The figures are the resident
set, `Rss`, which counts the pages of a binary that two processes share; the
rows keep `Pss`, private and anonymous memory beside it.

| Run                                   | UTC, 10 October   | Rows | File                 |
|---------------------------------------|-------------------|------|----------------------|
| every program, the first order        | 18:51:56–18:55:29 | 20   | `memory-a.jsonl`     |
| every program, the order turned round | 18:55:34–18:59:06 | 20   | `memory-b.jsonl`     |
| the sql cases, before the sweep       | 19:04:14–19:06:38 | 4    | `sweep-before.jsonl` |
| the sql cases, swept                  | 19:06:38–19:09:02 | 4    | `sweep-after.jsonl`  |
| in process, memory given back         | 19:10:28–19:15:53 | 16   | `trim.jsonl`         |

The rows and each run's environment are in
[data/rust-memory-2026-10-10](data/rust-memory-2026-10-10). Every cell gives
the runs as they ran: the two passes of every program, or the two of the sql
cases and the memory given back.

```sh
# from Git Bash, MSYS_NO_PATHCONV=1; <tinystore> has every commit named above
# a tree a commit: /perf/slice-mem at 51d7eee, /perf/slice-sweep at 78fb22e, each built as
docker run --rm -v <tinystore>:/tiny:ro -v <research>:/src -v tinystore-perf:/perf \
  -e WORK=/perf/<tree> -e RUST_COMMIT=<commit> golang:1.27 sh /src/tinystore/slice-bench/build.sh sources
docker run --rm -v <research>:/src -v tinystore-perf:/perf -v tinystore-go:/go \
  -v tinystore-gocache:/root/.cache/go-build -e GOWORK=off -e WORK=/perf/<tree> golang:1.27 \
  sh /src/tinystore/slice-bench/build.sh go
docker run --rm -v <research>:/src -v tinystore-perf:/perf -v tinystore-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/perf/<tree>/target -e WORK=/perf/<tree> \
  -e 'SQLITE_FLAGS=SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT -DHAVE_FDATASYNC=1' \
  rust:1.99 sh /src/tinystore/slice-bench/build.sh rust

run() {
  docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
    -e OUT=/src/tinystore/reports/data/rust-memory-2026-10-10 -e REST=130 "$@" golang:1.27 python3 memory.py
}
run -e TREE=/perf/slice-mem -e RUNS=memory-a
run -e TREE=/perf/slice-mem -e RUNS=memory-b -e FIRST=1
run -e TREE=/perf/slice-mem -e RUNS=sweep-before -e ONLY=rust,bun-rust-sidecar -e CASES=sql-point:64,sql-rows:1
run -e TREE=/perf/slice-sweep -e RUNS=sweep-after -e ONLY=rust,bun-rust-sidecar -e CASES=sql-point:64,sql-rows:1
run -e TREE=/perf/slice-sweep -e RUNS=trim -e ONLY=rust,go -e PASSES=2
```

The cases are the slice round's, as [its load](rust-slice-2026-10-10.md#the-load)
has them: kv-get fills 100,000 keys of 128 bytes and reads them with 64
callers, sql-point fills 100,000 rows and reads one by its id with 64,
sql-rows reads 25,000 rows at a time, jobs-add adds jobs with 64; each for
three seconds. A rest is 130 seconds, past the minute after which an idle
reader closes and the minute of maintenance that closes it.

## In process

The Rust and the Go program, MiB resident:

| Case         | Program | before   | open     | after the case | after the rest |
|--------------|---------|----------|----------|----------------|----------------|
| kv-get 64    | rust    | 3.1; 3.2 | 3.3; 3.4 | 25.4; 25.3     | 20.3; 20.5     |
| kv-get 64    | go      | 5.3; 5.3 | 5.4; 5.4 | 32.9; 32.6     | 21.5; 23.9     |
| sql-point 64 | rust    | 3.2; 3.1 | 3.4; 3.5 | 24.8; 24.9     | 24.8; 24.9     |
| sql-point 64 | go      | 5.5; 5.5 | 5.5; 5.5 | 28.8; 29.0     | 20.2; 20.3     |
| sql-rows 1   | rust    | 3.1; 3.1 | 3.4; 3.4 | 16.1; 16.2     | 16.1; 16.2     |
| sql-rows 1   | go      | 5.5; 5.5 | 5.5; 5.5 | 21.5; 25.0     | 18.6; 16.9     |
| jobs-add 64  | rust    | 3.0; 3.1 | 3.4; 3.3 | 9.6; 10.7      | 9.9; 11.0      |
| jobs-add 64  | go      | 5.5; 5.5 | 5.5; 5.6 | 18.0; 18.2     | 17.4; 18.6     |

A Rust program holds 3.1 MiB before its store opens and 3.4 once it is open,
where Go's holds 5.3–5.5 and 5.4–5.6: an engine opens with its first handle.
After a case Rust's process holds less than Go's in every case, 9.6–25 MiB to
Go's 18–33. After the rest it holds less or as much, but for sql point reads:
25 where Go's holds 20, as right after the case.

## The sql cases swept

Kv and jobs close a reader left unused for a minute in their maintenance, and
an SQL database had no maintenance: every reader a burst of reads opened stayed
open, its cache with it. `78fb22e` closes a database's idle readers every
minute on the store's background thread. The sql cases on both commits, MiB
resident:

| Case         | Process       | after the case, before | after the rest, before | after the case, swept | after the rest, swept |
|--------------|---------------|------------------------|------------------------|-----------------------|-----------------------|
| sql-point 64 | Rust program  | 24.9                   | 24.9                   | 24.9                  | 22.9                  |
| sql-point 64 | Rust's server | 27.8                   | 27.8                   | 27.9                  | 27.9                  |
| sql-rows 1   | Rust program  | 16.2                   | 16.2                   | 16.2                  | 16.2                  |
| sql-rows 1   | Rust's server | 24.7                   | 24.7                   | 24.8                  | 24.8                  |

The program's point reads hold 22.9 MiB after the rest where they held 24.9;
the server holds as much as before. The readers closed, and their memory stayed
with the process.

## Memory given back

glibc keeps memory freed for later allocations rather than handing it back to
the system, in every arena a thread used. The programs at `78fb22e`, after the
rest and once more after `malloc_trim(0)` in Rust and `debug.FreeOSMemory()` in
Go, resident and anonymous MiB:

| Case         | Program | after the rest | given back | heap after the rest | heap given back |
|--------------|---------|----------------|------------|---------------------|-----------------|
| kv-get 64    | rust    | 21.6; 22.5     | 15.8; 15.0 | 16.3; 17.0          | 10.4; 9.5       |
| kv-get 64    | go      | 21.7; 21.5     | 19.6; 19.5 | 12.8; 12.9          | 10.7; 10.8      |
| sql-point 64 | rust    | 22.3; 23.0     | 14.4; 13.6 | 16.9; 17.6          | 8.9; 8.2        |
| sql-point 64 | go      | 19.9; 20.4     | 15.2; 16.0 | 11.3; 11.6          | 6.6; 7.2        |
| sql-rows 1   | rust    | 16.3; 16.2     | 11.4; 13.6 | 10.9; 10.9          | 6.1; 8.4        |
| sql-rows 1   | go      | 16.9; 16.4     | 14.2; 13.9 | 9.2; 8.6            | 6.4; 6.1        |
| jobs-add 64  | rust    | 10.2; 11.3     | 9.8; 10.8  | 4.7; 6.0            | 4.3; 5.5        |
| jobs-add 64  | go      | 17.4; 17.9     | 14.5; 14.7 | 9.1; 9.5            | 6.2; 6.3        |

`malloc_trim` takes 6–9 MiB from the Rust process after kv and sql point
reads, the readers' caches among it, 3–5 after 25,000 rows, and little after
jobs. After it Rust holds 10–16 MiB
where Go holds 14–20, less in every case; its heap is 4–10 MiB. What is left
of the difference before it is glibc's keeping, not the store's.

## Beside Bun

The server beside the Bun program, MiB resident:

| Case         | Server | open       | after the case | after the rest |
|--------------|--------|------------|----------------|----------------|
| kv-get 64    | rust   | 5.8; 5.8   | 27.9; 28.0     | 18.4; 28.0     |
| kv-get 64    | go     | 11.2; 11.1 | 52.9; 52.8     | 38.1; 41.1     |
| sql-point 64 | rust   | 5.8; 5.8   | 27.8; 27.8     | 27.8; 27.8     |
| sql-point 64 | go     | 11.0; 11.1 | 48.5; 48.1     | 36.1; 36.8     |
| sql-rows 1   | rust   | 5.8; 5.9   | 24.7; 24.8     | 24.4; 24.8     |
| sql-rows 1   | go     | 11.2; 11.5 | 38.2; 37.8     | 36.8; 37.9     |
| jobs-add 64  | rust   | 5.8; 5.8   | 9.6; 9.6       | 9.9; 9.9       |
| jobs-add 64  | go     | 11.0; 11.1 | 37.0; 37.5     | 24.9; 36.8     |

Rust's server starts at 5.8 MiB to Go's 11 and holds 1.5–3.9× less after
every case, 1.3–3.1× after the rest. After a rest it keeps its sql readers, which `78fb22e` closes, and
what glibc keeps of them.

The Bun program itself:

| Case         | Bun with        | before the SDK | open       | after the case | after the rest |
|--------------|-----------------|----------------|------------|----------------|----------------|
| kv-get 64    | the core inside | 42.5; 41.8     | 66.6; 60.5 | 120.2; 123.5   | 85.3; 82.7     |
| kv-get 64    | Rust's server   | 42.6; 42.1     | 60.4; 64.7 | 90.8; 92.0     | 60.0; 60.9     |
| kv-get 64    | Go's server     | 42.6; 43.2     | 62.5; 64.9 | 92.4; 92.1     | 60.6; 61.4     |
| sql-point 64 | the core inside | 41.9; 41.8     | 59.4; 60.5 | 120.7; 122.2   | 86.9; 86.1     |
| sql-point 64 | Rust's server   | 42.5; 42.5     | 60.0; 60.1 | 95.9; 97.3     | 63.2; 63.9     |
| sql-point 64 | Go's server     | 42.6; 42.5     | 59.7; 59.7 | 92.5; 96.7     | 62.9; 62.9     |
| sql-rows 1   | the core inside | 42.0; 42.0     | 59.7; 60.1 | 182.6; 179.6   | 82.5; 82.3     |
| sql-rows 1   | Rust's server   | 42.5; 42.5     | 60.1; 60.4 | 129.5; 145.3   | 67.5; 67.1     |
| sql-rows 1   | Go's server     | 42.6; 42.5     | 59.7; 59.9 | 110.9; 111.9   | 63.2; 63.8     |
| jobs-add 64  | the core inside | 41.8; 41.7     | 60.0; 60.3 | 94.3; 93.9     | 67.4; 67.2     |
| jobs-add 64  | Rust's server   | 42.6; 42.7     | 60.3; 60.3 | 87.8; 86.9     | 62.6; 62.9     |
| jobs-add 64  | Go's server     | 42.5; 42.5     | 60.2; 60.1 | 82.4; 82.7     | 62.6; 62.5     |

Bun holds 42 MiB before it loads the SDK and 60 once a store is open,
whichever way it reaches it: the SDK's own JavaScript is most of the step. With
the core inside, the process holds after a case about what the program and a
server beside it hold together, 94–183 MiB, and after a rest 67–87.

## What follows

Nothing below is built.

- **Memory given back after a sweep.** Where the allocator is glibc's, the
  store could call `malloc_trim` once a sweep closed readers: 3–9 MiB of a
  resting Rust process here. It is a call into libc, in the module that owns
  SQLite's memory.
- **What the rest of the heap is.** 4–10 MiB after everything is given back:
  a reader and the writer of each file, their statements, the engines' own.
  A count by SQLite's own memory figures would divide it.

