# The protocol's codecs written out — 2026-10-10

The wire protocol's codecs, which `crates/protocol` writes from
`protocol/*.wire` for the Rust core and the Bun SDK, as they were and written
out a field at a time, measured through Bun and in process on the host of the
[slice round](rust-slice-2026-10-10.md). Both are the product's code.

Until `9cb490f` the core read a message into a tree of values and picked each
field out of it by number, and wrote one by building a tree and writing that;
the SDK walked a table of the message's fields, a number and a codec each, for
every message it wrote or read. `71d6dde` writes each message's reader and
writer out. Through Bun, keys are read 1.03–1.07× as fast with one to 64 in
flight and 25,000 rows 1.18–1.25×; writes, which wait for the disk, do not
move.

It cost the bare loop over the C functions, the most a Bun program makes of
the core: 0.83–0.93× of before with 64 reads in flight and 0.70–0.73× with
256. The core's answers grew their buffers as they were written, and glibc's
`realloc` takes its arena's lock: under perf, threads waking one another for
that lock and waiting for it were 18.7% of the samples. `2633261` works out a
message's size before writing it and reserves it once. The bare loop then
reads 1.07–1.12× of before, and the SDK 1.05–1.08×.

The SDK is at the bare loop's 37% with 64 reads in flight, where it was, since
the core gave both the same. What is left on its thread is spread thin; under
the bare loop, the core is SQLite reading pages.

| Question                               | Observation                                                                                                                                                                                        | What follows                                                      |
|----------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------|
| Do codecs written out pay through Bun? | Keys inside Bun 1.03–1.07× with one to 64 in flight, 0.99× with 256; 25,000 rows 1.25× inside Bun and 1.18× beside it; writes 0.98–0.99×.                                                          | `71d6dde`, with the row below.                                    |
| What did they cost the bare loop?      | 0.83–0.93× with 64 in flight and 0.70–0.73× with 256, in three runs. `realloc` was under 12.8% of the samples and `free` 7.5%, glibc's lock of an arena 18.7%, futex calls 19% where they were 4%. | `2633261` reserves a message's bytes before writing it.           |
| Did reserving them close it?           | The bare loop reads 1.07–1.12× of before at one to 256 in flight; a key's answer is written in 0.65% of the samples, where it took 5.5% before and 13.8% written out.                              | Built.                                                            |
| Where is the SDK now?                  | 0.78× of the bare loop one read at a time, 0.60× with 8, 0.37× with 64, 0.34× with 256, as before. Writing a call is 10% of its thread where the table took 12%; the answer's table walk is gone.  | Nothing for now: the rest is the value's JSON, a promise, a call. |
| What does the core do under the loop?  | SQLite: `sqlite3_step` under 54% of the samples with 256 in flight, `pread` of pages the connection's cache does not hold under 24%.                                                               | `mmap_size`, measured against the memory it maps.                 |
| Does anything else move?               | A Rust program in process, which runs no codec, 0.92–0.99× on keys and 1.01–1.03× on rows; its passes of one tree span 1.47 to 1.74 million.                                                       | Read a tenth either way at 64 threads in process as the host's.   |

## Environment and reproduction

Everything ran in one evening on one AMD Ryzen 7 7700 (8 cores, 16 logical
processors) through Docker Desktop on WSL2: Linux 6.18.33.2, glibc 2.41, the
`golang:1.27` image, the stores on a Docker volume on WSL2's ext4. This is not
bare Linux: read the ratios of one run, and no number here as a promise for a
deployment. Nothing else built, tested or measured while a run was timed.

| What         | Commit or version                                                                                                                               |
|--------------|-------------------------------------------------------------------------------------------------------------------------------------------------|
| before       | `9cb490f9eb838fe85337b0bf21370508d31ef4f7`, branch `rust`                                                                                       |
| written out  | `71d6dde7c0304cc5f03d31b0148354d4dff271bf` on `rust`, on the one before                                                                         |
| and reserved | `263326109f108093f2ef6fcc47676e2f329ac38c` on `rust`, on the one before                                                                         |
| the harness  | research `27cec3fb3ac5cb0f02d96b2bdea5558164aafb71`; its profiles as `dddb4f051aedc39421230f70e253625b7310db47` has them                        |
| Rust         | rustc 1.99.0, the release profile, rusqlite 0.40.2 with its bundled SQLite, `SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT -DHAVE_FDATASYNC=1` |
| Bun          | 1.4.2                                                                                                                                           |

The harness is [slice-bench](../slice-bench/README.md), committed before the
first run: its bare loop opens a store through the C function that takes the
store's options, which `9cb490f` added. The profiles of the first two trees
ran from a copy of `profiles.sh` committed after them, unchanged but for where
it finds its files; the third ran from the commit. Each environment file names
its trees' commits and flags under `also`; its own `tinystore_rust_commit` is
the harness's default tree, which no run here measured.

| Run                              | UTC, 10 October   | Rows | File                 |
|----------------------------------|-------------------|------|----------------------|
| reads, before and written out    | 17:10:08–17:15:19 | 48   | `gen-reads.jsonl`    |
| the other cases inside Bun       | 17:15:20–17:16:54 | 30   | `gen-others.jsonl`   |
| the server beside Bun            | 17:16:55–17:19:18 | 24   | `gen-sidecar.jsonl`  |
| in process                       | 17:19:19–17:20:17 | 12   | `gen-native.jsonl`   |
| the bare loop again, five passes | 17:20:47–17:25:05 | 40   | `gen-raw.jsonl`      |
| reads, the three trees           | 17:31:47–17:44:45 | 120  | `size-reads.jsonl`   |
| the other cases inside Bun       | 17:44:46–17:47:07 | 45   | `size-others.jsonl`  |
| the server beside Bun            | 17:47:08–17:50:44 | 36   | `size-sidecar.jsonl` |
| in process, run again            | 17:57:48–17:59:15 | 18   | `size-native.jsonl`  |

The rows, each run's environment and the profiles are in
[data/rust-codecs-2026-10-10](data/rust-codecs-2026-10-10). The tree with the
bytes reserved was built between the first runs and the second. A first run of
the last, in process, ran while another process read a large file on the host,
and the Rust program read 0.89× of before where it runs no codec: it was run
again with nothing else running, and only the second is kept.

A case is a process of its own and a fresh store, timed for three seconds. A
pass runs every case, the programs in turn case by case, and every other pass
turns their order round. Reads ran five passes and the rest three; every table
gives the passes as they ran, and a ratio is the ratio of their medians.

```sh
# from Git Bash, MSYS_NO_PATHCONV=1; <tinystore> has the three commits
# a tree a commit: dur at 9cb490f, gen at 71d6dde, size at 2633261
docker run --rm -v <tinystore>:/tiny:ro -v <research>:/src -v tinystore-perf:/perf \
  -e WORK=/perf/slice-<tree> -e RUST_COMMIT=<commit> golang:1.27 sh /src/tinystore/slice-bench/build.sh sources
docker run --rm -v <research>:/src -v tinystore-perf:/perf -v tinystore-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/perf/slice-<tree>/target -e WORK=/perf/slice-<tree> \
  -e 'SQLITE_FLAGS=SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT -DHAVE_FDATASYNC=1' \
  rust:1.99 sh /src/tinystore/slice-bench/build.sh rust

run() {
  docker run --rm -v <research>:/src -v tinystore-perf:/perf -w /src/tinystore/slice-bench \
    -e OUT=/src/tinystore/reports/data/rust-codecs-2026-10-10 "$@" golang:1.27 python3 run.py
}
E=bun-rust-embedded S=bun-rust-sidecar
two=ALSO=dur=/perf/slice-dur,gen=/perf/slice-gen
run -e $two -e RUNS=gen-reads -e ONLY=$E-dur,$E-gen,bun-raw-dur,bun-raw-gen -e CASES=kv-get:1,kv-get:8,kv-get:64,kv-get:256
run -e $two -e RUNS=gen-others -e ONLY=$E-dur,$E-gen -e CASES=sql-point:1,sql-point:64,sql-rows:1,kv-set:64,jobs-add:64
run -e $two -e RUNS=gen-sidecar -e ONLY=$S-dur,$S-gen -e CASES=kv-get:1,kv-get:64,sql-point:64,sql-rows:1
run -e $two -e RUNS=gen-native -e ONLY=rust-dur,rust-gen -e CASES=kv-get:64,sql-rows:1
run -e $two -e RUNS=gen-raw -e PASSES=5 -e ONLY=bun-raw-dur,bun-raw-gen,$E-dur,$E-gen -e CASES=kv-get:64,kv-get:256
three=ALSO=dur=/perf/slice-dur,gen=/perf/slice-gen,size=/perf/slice-size
run -e $three -e RUNS=size-reads -e PASSES=5 -e ONLY=bun-raw-dur,bun-raw-gen,bun-raw-size,$E-dur,$E-gen,$E-size \
  -e CASES=kv-get:1,kv-get:8,kv-get:64,kv-get:256
run -e $three -e RUNS=size-others -e ONLY=$E-dur,$E-gen,$E-size -e CASES=sql-point:1,sql-point:64,sql-rows:1,kv-set:64,jobs-add:64
run -e $three -e RUNS=size-sidecar -e ONLY=$S-dur,$S-gen,$S-size -e CASES=kv-get:1,kv-get:64,sql-point:64,sql-rows:1
run -e $three -e RUNS=size-native -e ONLY=rust-dur,rust-gen,rust-size -e CASES=kv-get:64,sql-rows:1

# the profiles: a privileged container of golang:1.27 with perf, <research> at /src and the volume at /perf
TREES="dur gen size" OUT=/src/tinystore/reports/data/rust-codecs-2026-10-10 sh /src/tinystore/slice-bench/profiles.sh
```

The cases are the slice round's, as [its load](rust-slice-2026-10-10.md#the-load)
has them: a key of 100,000 with a value of 128 bytes, a row of 100,000 by its
id, 25,000 rows as a list, a set and a job's add returned once synced. In
flight is the calls at once: promises awaited in Bun, threads in Rust. Inside
Bun is the SDK with the core in its process; beside Bun, the SDK through
`tinystore serve`; the bare loop is `bun/raw.ts`, kv gets over the C functions
with no SDK, each value decoded as JSON as the SDK decodes it, and with none
decoded under perf; in process, the Rust program, which runs no codec and is
the runs' control.

## What changed

Before, the core decoded a body into a tree of `Value`s, the whole message at
once, and each message's `Fields` took its numbers out of the tree; a message
was written by building its tree and writing that. The SDK's `message()` kept a
table of fields, and one writer and one reader walked it for every message.

Written out, the generator gives each message a reader and a writer of its own.
The core's reader takes tokens one at a time from the one parser,
`msgpack::Reader`, into the message's fields, and refuses what the profile
refuses where it meets it; its writer writes the fields it has in turn and
patches the map's count at the end, one byte, or three spliced in past fifteen
fields. The SDK's `write` and `read` are a message's own: the reader counts the
map's fields down into a local each and returns an object of one shape.

## Written out

Calls a second, each pass, before and written out:

| Where      | Case      | In flight | before                          | written out                     | written out to before |
|------------|-----------|-----------|---------------------------------|---------------------------------|-----------------------|
| inside Bun | kv-get    | 1         | 131,489; 135,861; 135,554       | 135,369; 141,042; 140,914       | 1.04×                 |
| inside Bun | kv-get    | 8         | 210,391; 231,455; 229,464       | 222,652; 236,021; 245,005       | 1.03×                 |
| inside Bun | kv-get    | 64        | 344,538; 347,951; 352,978       | 376,827; 368,046; 372,183       | 1.07×                 |
| inside Bun | kv-get    | 256       | 402,313; 395,153; 391,714       | 393,729; 377,633; 391,970       | 0.99×                 |
| inside Bun | sql-point | 1         | 89,170; 89,331; 94,547          | 92,268; 92,277; 98,624          | 1.03×                 |
| inside Bun | sql-point | 64        | 182,750; 184,816; 195,596       | 186,993; 182,788; 200,172       | 1.01×                 |
| inside Bun | sql-rows  | 1         | 43.5; 44.2; 45.4                | 54.3; 55.4; 57.6                | 1.25×                 |
| inside Bun | kv-set    | 64        | 18,907; 19,018; 19,055          | 18,607; 18,775; 18,708          | 0.98×                 |
| inside Bun | jobs-add  | 64        | 22,549; 23,073; 23,258          | 22,535; 23,448; 22,828          | 0.99×                 |
| bare loop  | kv-get    | 1         | 161,707; 168,391; 167,033       | 168,855; 173,662; 172,813       | 1.03×                 |
| bare loop  | kv-get    | 8         | 339,239; 371,931; 348,889       | 373,405; 359,608; 393,508       | 1.07×                 |
| bare loop  | kv-get    | 64        | 949,929; 865,158; 966,397       | 850,375; 970,869; 882,073       | 0.93×                 |
| bare loop  | kv-get    | 256       | 1,197,203; 1,160,839; 1,110,145 | 846,232; 839,676; 1,046,021     | 0.73×                 |
| beside Bun | kv-get    | 1         | 11,486; 11,775; 11,617          | 11,965; 11,580; 11,508          | 1.00×                 |
| beside Bun | kv-get    | 64        | 211,354; 209,341; 216,998       | 216,149; 218,959; 218,446       | 1.03×                 |
| beside Bun | sql-point | 64        | 145,323; 142,869; 140,720       | 144,448; 148,668; 147,069       | 1.03×                 |
| beside Bun | sql-rows  | 1         | 32.2; 33.8; 34.6                | 40.5; 40.0; 39.9                | 1.18×                 |
| in process | kv-get    | 64        | 1,740,704; 1,732,647; 1,736,227 | 1,764,107; 1,667,693; 1,722,802 | 0.99×                 |
| in process | sql-rows  | 1         | 157; 155; 156                   | 160; 152; 154                   | 0.99×                 |

Inside Bun, a key is read 1.03–1.07× as fast with one to 64 in flight and as
fast with 256; 25,000 rows 1.25×, where the answer is every row's fields, and
1.18× through the server beside Bun. Writes wait for the disk and do not move;
nor does the program in process. The bare loop fell where it reads most: 0.93×
with 64 in flight, 0.73× with 256. The bare loop and the SDK alone, five
passes:

| Where      | Case   | In flight | before                                                | written out                                 | written out to before |
|------------|--------|-----------|-------------------------------------------------------|---------------------------------------------|-----------------------|
| bare loop  | kv-get | 64        | 943,247; 963,437; 982,496; 985,714; 969,932           | 990,967; 602,894; 802,319; 873,295; 785,895 | 0.83×                 |
| bare loop  | kv-get | 256       | 1,083,276; 1,074,976; 1,103,628; 1,197,777; 1,099,750 | 847,949; 721,028; 796,997; 768,555; 761,477 | 0.70×                 |
| inside Bun | kv-get | 64        | 348,025; 357,885; 361,947; 367,095; 372,081           | 373,859; 366,755; 379,209; 378,470; 386,534 | 1.05×                 |
| inside Bun | kv-get | 256       | 381,813; 375,650; 401,007; 391,085; 398,869           | 385,256; 408,148; 411,049; 376,958; 386,211 | 0.99×                 |

## Why the bare loop fell

The bare loop at 256 in flight under `perf record -e cpu-clock --call-graph
dwarf`, each tree once (`profile-core-<tree>.txt`), every thread's samples,
each share with what it calls; a dash is below the last of the 120 lines a
profile keeps, 0.63–0.65%:

| Of the samples                            | before    | written out | and reserved |
|-------------------------------------------|-----------|-------------|--------------|
| reads a second under perf                 | 1,541,732 | 987,608     | 1,466,631    |
| `KvEntry::encode`, a key's answer written | 5.53%     | 13.83%      | 0.65%        |
| `realloc`                                 | 2.29%     | 12.80%      | —            |
| `free`                                    | 2.85%     | 7.54%       | 0.79%        |
| `__lll_lock_wake_private`                 | —         | 12.15%      | —            |
| `__lll_lock_wait_private`                 | —         | 6.53%       | —            |
| futex system calls                        | 4.13%     | 19.21%      | 5.26%        |
| `KvCall::decode`, a call read             | 4.40%     | 1.91%       | 1.98%        |

Written out, nearly all of a key's answer was `realloc`. The answer's buffer
started at 64 bytes and grew twice for a value of 128, and glibc's `realloc`
takes the lock of the arena its block came from, which its `malloc` and `free`
of a small block mostly do not: sixteen workers woke one another and waited
for those locks. The tree before grew its buffer too, from nothing, and paid
2.3% in `realloc` with no wait for the lock in its profile; the profiles do not
say why the same growth cost it less. That the growth was the bound is what the
next commit tested. Reading a call, the other half of the codecs, went from
4.4% of the samples to 1.9%.

## A message's bytes reserved

`2633261` gives each message a `size`, which the generator writes beside its
writer: the bytes the message writes at most, from its fields' lengths, and
`encode` reserves that many once. A test writes values of tens of thousands of
bytes, 300 names and 2,000 rows, and checks that no message writes more than
it reserved. The three trees side by side:

| Where      | Case      | In flight | before                                            | written out                                 | and reserved                                          | written out to before | reserved to before |
|------------|-----------|-----------|---------------------------------------------------|---------------------------------------------|-------------------------------------------------------|-----------------------|--------------------|
| inside Bun | kv-get    | 1         | 130,830; 135,106; 128,542; 129,242; 130,584       | 139,123; 136,910; 137,803; 136,877; 129,774 | 145,002; 140,611; 135,070; 136,594; 136,922           | 1.05×                 | 1.05×              |
| inside Bun | kv-get    | 8         | 231,208; 234,524; 211,967; 214,634; 219,593       | 225,086; 234,794; 226,169; 213,538; 217,450 | 238,265; 242,856; 223,759; 189,537; 236,279           | 1.03×                 | 1.08×              |
| inside Bun | kv-get    | 64        | 354,577; 353,627; 348,906; 359,301; 338,877       | 373,258; 366,527; 372,557; 372,763; 340,568 | 385,880; 379,925; 364,400; 390,283; 329,537           | 1.05×                 | 1.07×              |
| inside Bun | kv-get    | 256       | 378,870; 354,555; 371,736; 356,753; 367,117       | 400,337; 370,106; 368,217; 387,840; 374,595 | 426,419; 390,454; 367,427; 403,485; 392,575           | 1.02×                 | 1.07×              |
| inside Bun | sql-point | 1         | 81,209; 84,135; 82,385                            | 86,799; 87,507; 86,738                      | 85,296; 92,880; 90,557                                | 1.05×                 | 1.10×              |
| inside Bun | sql-point | 64        | 177,652; 189,205; 179,107                         | 177,989; 187,985; 177,392                   | 177,443; 189,769; 177,340                             | 0.99×                 | 0.99×              |
| inside Bun | sql-rows  | 1         | 43.8; 44.0; 43.2                                  | 54.7; 52.5; 53.9                            | 55.7; 54.5; 54.1                                      | 1.23×                 | 1.24×              |
| inside Bun | kv-set    | 64        | 18,351; 18,560; 18,422                            | 18,774; 18,418; 18,159                      | 18,592; 18,914; 18,705                                | 1.00×                 | 1.02×              |
| inside Bun | jobs-add  | 64        | 22,399; 22,053; 22,837                            | 22,903; 23,225; 22,274                      | 22,952; 22,376; 22,493                                | 1.02×                 | 1.00×              |
| bare loop  | kv-get    | 1         | 167,674; 167,247; 163,086; 163,240; 162,488       | 170,750; 171,316; 162,853; 170,779; 160,489 | 175,320; 179,161; 174,521; 173,629; 169,858           | 1.05×                 | 1.07×              |
| bare loop  | kv-get    | 8         | 352,718; 358,637; 355,687; 306,853; 348,799       | 366,967; 378,758; 362,185; 372,062; 352,438 | 403,480; 402,597; 381,237; 395,241; 388,948           | 1.04×                 | 1.12×              |
| bare loop  | kv-get    | 64        | 961,549; 977,566; 913,603; 956,832; 947,474       | 688,888; 853,837; 884,983; 819,175; 921,012 | 1,046,848; 1,032,536; 1,036,096; 1,029,421; 1,017,154 | 0.89×                 | 1.08×              |
| bare loop  | kv-get    | 256       | 1,205,363; 1,060,989; 840,077; 1,145,099; 988,932 | 836,865; 778,481; 757,413; 795,543; 761,331 | 1,229,612; 1,106,116; 1,145,519; 1,187,592; 1,150,796 | 0.73×                 | 1.08×              |
| beside Bun | kv-get    | 1         | 11,291; 11,552; 10,762                            | 11,484; 11,752; 11,457                      | 11,254; 11,813; 11,139                                | 1.02×                 | 1.00×              |
| beside Bun | kv-get    | 64        | 206,578; 194,571; 210,968                         | 208,316; 209,967; 215,317                   | 200,483; 220,435; 214,108                             | 1.02×                 | 1.04×              |
| beside Bun | sql-point | 64        | 137,293; 141,590; 140,805                         | 138,983; 126,164; 144,659                   | 144,615; 151,251; 141,066                             | 0.99×                 | 1.03×              |
| beside Bun | sql-rows  | 1         | 31.8; 33.4; 31.6                                  | 39.4; 39.2; 37.6                            | 39.5; 37.3; 38.6                                      | 1.23×                 | 1.21×              |
| in process | kv-get    | 64        | 1,470,769; 1,744,126; 1,720,874                   | 1,488,191; 1,642,386; 1,584,641             | 1,733,939; 1,673,173; 1,708,706                       | 0.92×                 | 0.99×              |
| in process | sql-rows  | 1         | 154; 162; 148                                     | 158; 158; 156                               | 156; 146; 155                                         | 1.03×                 | 1.01×              |

The bare loop reads 1.07–1.12× of before at every count in flight, the SDK
1.05–1.08×, 25,000 rows 1.21–1.24×; writes and a point read with 64 in flight
are as before. Under perf a key's answer is written in 0.65% of the samples,
`realloc` is gone below the profile's kept lines, and futex calls are 5.3%,
where they were 4.1% before. The program in process ran its passes of one tree
1.47 to 1.74 million apart: its 0.92× and 0.99× are this host's, and so is
anything within a tenth at 64 threads in process.

## The SDK beside the bare loop

The SDK's reads as a share of the bare loop's, each tree from its own medians:

| Case   | In flight | before | written out | and reserved |
|--------|-----------|--------|-------------|--------------|
| kv-get | 1         | 0.80×  | 0.80×       | 0.78×        |
| kv-get | 8         | 0.62×  | 0.61×       | 0.60×        |
| kv-get | 64        | 0.37×  | 0.44×       | 0.37×        |
| kv-get | 256       | 0.35×  | 0.48×       | 0.34×        |

The SDK is where it was, 37% of the bare loop with 64 in flight and 34% with
256, since the core gave both the same; written out but not reserved it read
44–48% of a bare loop that was held back. Bun's profiler on the SDK with the core inside, 64 in
flight (`profile-bun-<tree>.txt`), says what the codecs took from its one
thread: writing a call, 12.0% of the samples through the table, is 9.3–10.0%
written out, and the table's walk of an answer's fields, 5.8%, is gone. What
is left is spread thin: the value's text and `JSON.parse` 13.5%, the
harness's own loop and choice of key 20%, which every tree pays alike, and
the C call, the promise and the session's books the rest.

## The core under the bare loop

With the answers reserved, the core's threads under the bare loop at 256 in
flight are SQLite's. The bucket's read is under 78% of the samples,
`sqlite3_step` under 54%, and `readDbPage` under 24%: a read of a key fetches
the pages its connection's cache of 1 MiB does not hold from the operating
system's cache by `pread`, a system call and a copy each, 18% of the samples
in the call itself. The session, the codecs and the workers' waits are the
other 12% of the samples the workers took. SQLite can map the file instead,
`mmap_size`, which spares the call and the copy; a mapped page counts in the
process's resident memory, so it is a question for a measurement of memory.

## What follows

Nothing below is built.

- **The SDK's own work a call.** It stays at a third of the bare loop for now:
  what is left is no one thing. The value's JSON is the bucket's codec, which
  the program chooses; a bucket of bytes would show what a call alone costs.
- **Pages by `pread`.** `mmap_size` against the memory it maps, beside the
  memory a store holds at rest.
- **The codecs for Python and Go** are written out from the start, with their
  sizes reserved, as the generator writes Rust's and TypeScript's.

