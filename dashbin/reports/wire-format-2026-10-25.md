# A frame on the wire, 25 October 2026

Chapter 3 left one question open: which encoding a frame crosses a socket
in, from the hub to a browser, from an agent to the hub, from a process
plugin to the kernel. Three candidates, the same two frames of 50 000 rows,
decoded to the point where a chart and a table can read them, in V8,
Chromium and JavaScriptCore; then what each adds to the binary and the
bundle. The margins were committed before any timing ran
([`wire-probe/README.md`](../wire-probe/README.md)).

| Question | Answer | What follows |
|---|---|---|
| Is any encoding lossless? | All three, after work: serde_json needs `float_roundtrip`, JSON carries big integers and decimals as strings, and Arrow JS's getters must not be used | a reader of our own in every case |
| Round 1, text as text | JSON stays: the binary encodings are 2.6–8× faster, and 1.31× larger in gzip on the chart frame, past the 1.25× the rule allows | the whole gap is one buffer |
| Where the gap is | the offsets of the `site` column: 200 KB of `u32` that gzip takes to 133 KB, against 1.8 KB for the same strings in JSON | labels need a dictionary |
| Round 2, labels as a dictionary everywhere | Arrow over JSON: 8–31× faster, 1.09× larger in gzip on one frame and smaller on the other | binary on the wire |
| Our layout over Arrow? | 1.74–2.11× faster, equal in gzip: short of the 2× margin in two of four cells; +41 KB to the binary against +2.27 MB, 1.9 KB to the bundle against 53 KB | our layout, chosen by Dmitry as an exception to the margins, on the cost of the dependency |
| What does Arrow cost to carry? | +2.27 MB to the hub (+0.75 to an agent, which only writes), 30 crates; 53 KB gzip in the page, 7–8 ms and 1 MB of heap to load, 15–60 µs a frame for its schema | weight is the whole case against it |
| Time as `i64` or `f64`? | `f64` saves a copy: up to 0.19 ms and 391 KiB per 50 000 rows, gzip within 0.5% | `f64` per column when every value is exact, `i64` otherwise |

## Environment and reproduction

- One machine: Ryzen 7 7700, 16 threads, Windows 11. JavaScript ran on the
  host: Node 26.4.0 (V8 14.6), Bun 1.4.2 (JavaScriptCore), and Chromium 152 as
  the browser pane of the Claude desktop app. The pane was hidden while its
  page ran, so the tab ran as a background page; its ratios agree with Node's.
- Rust in Docker Desktop 29.6.2 on WSL2, the `rssprobe-musl` image of
  `memory-probe/docker` (`rust:1.99-bookworm@sha256:e12566d5…`). Crates as
  locked in `wire-probe/rust/Cargo.lock`: arrow-ipc 60.0.0 with no default
  features, serde_json 1.0.154 with `float_roundtrip`, flate2 1.1.10; zstd
  0.13 and brotli 8 only to report sizes. apache-arrow 21.2.0 in the browser.
- Size binaries are static musl, fat LTO, one codegen unit, stripped; each
  holds the frames and writes and reads them one way, against one that only
  builds the frames.
- Timing is `performance.now()` around each step, twelve rounds after three of
  warm-up, the order of the encodings rotating every round; Chromium was
  cross-origin isolated, so its clock has 5 µs resolution. Retained memory is
  `heapUsed + arrayBuffers` after two collections, Node only; Bun's figures are
  in the raw files and are noisier.
- The harness was committed before timing: round 1 as 77fbcd3, round 2 as
  741d2b2. The generator had run before both commits, since it has to prove
  the round trips, so the sizes were known when the margins were written; the
  timings were not. The retained-memory loop was fixed between the two (a
  previous format's garbage was collected inside the next one's measurement,
  giving negative numbers); round 1's memory figures come from the fixed loop.
- The probes of Arrow's carrying cost (split binaries, `load`, `small`) were
  written after round 2 and run before they were committed; no margin rests on
  them.

```sh
MSYS_NO_PATHCONV=1 docker run --rm -v <research>/dashbin/wire-probe:/w -v rssprobe-cargo:/usr/local/cargo/registry \
  -v wireprobe-target:/target -e CARGO_TARGET_DIR=/target -w /w/rust rssprobe-musl \
  bash -c 'cargo build -q --release --features all --bin gen && /target/release/gen /w/out'
MSYS_NO_PATHCONV=1 docker run --rm -v <research>/dashbin/wire-probe:/w -v rssprobe-cargo:/usr/local/cargo/registry \
  -v wireprobe-target:/target -e CARGO_TARGET_DIR=/target -e CC_x86_64_unknown_linux_musl=musl-gcc \
  -w /w/rust rssprobe-musl bash size.sh
cd <research>/dashbin/wire-probe/js && bun install && node check.mjs
node --expose-gc node.mjs && bun node.mjs && bun bundles.mjs
bun build browser.mjs --outdir dist --target browser && node serve.mjs   # open the printed address
```

Raw output: [data/wire-format-2026-10-25/](data/wire-format-2026-10-25/),
`round-1/` and `round-2/`, one file a tool.

## What was compared

Two frames, from a fixed seed. `mixed` holds every type of chapter 3 with
its bounds: `i64::MIN` and `MAX`, 2⁵³ ± 1, decimals of ±(10³⁸ − 1), NaN and
infinity, dates from 1950, text in Cyrillic and emoji up to 4 KB, 5–10% nulls,
values at random. `series` is twenty sites scraped every 15 s for a month:
time, site, a count on a daily wave, a ratio to four places, an amount in
cents, 2% nulls.

Each encoding has one reader in `wire-probe/js/formats/`. A round is decode
(from an `ArrayBuffer`), chart (a `Float64Array` each for time, count, ratio
and amount, null as NaN, and a code per row for the site with its list of
names) and table (the first 1 000 rows of every column as exact text). Before
any timing, `run.mjs` checks that all eight files of a frame draw the same
sums and print the same cells; they do, digests `f52093b4` and `b880fc8f` in
every engine.

**Arrow JS cannot be read through its own getters.** A `Decimal128(38, 2)`
holding 2293617.22 prints as `229361722`, its scale dropped; a
`Timestamp(µs)` comes back as fractional milliseconds,
`1790812861440.271`. So the Arrow reader parses with `tableFromIPC` and then
reads the buffers with the same code our layout uses: what Arrow brings to
the browser is the envelope's parser.

## Round 1: text as text

Decode + chart, median ms:

| | Node series | Node mixed | Chromium series | Chromium mixed | Bun series | Bun mixed |
|---|---|---|---|---|---|---|
| json | 9.30 | 19.45 | 11.30 | 36.35 | 10.27 | 20.18 |
| arrow | 2.75 | 3.45 | 4.38 | 4.97 | 2.57 | 2.84 |
| own-i64 | 2.56 | 3.17 | 3.96 | 4.68 | 2.35 | 2.45 |
| own-f64 | 2.40 | 3.09 | 3.77 | 4.51 | 2.35 | 2.38 |

Bytes:

| | series raw | series gzip | mixed raw | mixed gzip |
|---|---|---|---|---|
| json | 2 267 100 | 323 174 | 8 116 846 | 2 392 518 |
| arrow | 2 514 888 | 424 535 | 6 452 936 | 2 338 189 |
| own-i64 | 2 501 392 | 424 000 | 6 438 840 | 2 337 649 |
| own-f64 | 2 501 392 | 422 076 | 6 438 840 | 2 337 934 |

Against JSON, Arrow is 3.4× and 5.6× faster in Node and 2.6× and 7.3× in
Chromium, past the 2× margin; but 1.31× larger in gzip on `series`, past the
1.25× the margin allows. Our layout is the same. **By the rule, JSON stays.**

Each column compressed on its own shows where the 101 KB are:

| `series` column | json → gzip | own → gzip |
|---|---|---|
| time | 850 001 → 10 391 | 400 000 → 12 190 |
| site | 432 501 → 1 831 | offsets 200 004 → 132 941, bytes 282 500 → 1 079 |
| count | 201 023 → 56 645 | 406 250 → 64 508 |
| ratio | 342 660 → 95 538 | 406 250 → 105 842 |
| amount | 440 644 → 152 767 | 806 250 → 157 443 |

Everything but `site` is within a fifth of JSON. The site's strings compress to a
kilobyte either way; its offsets, growing by an irregular step every row, do
not compress at all. That is a property of how the binary encodings were
asked to carry a label, not of being binary, and both columnar formats have
the standard answer for it.

## Round 2: labels as a dictionary

A text column with at most one distinct value per two rows goes as a
dictionary in **all three** encodings, JSON included, so it gains from it as
much as the others: `"dictionary": [names], "values": [codes]` in JSON,
`Dictionary(Int32, Utf8)` in Arrow, type 9 in our layout. `mixed.message`,
nearly all distinct, stays text. The margins did not move.

Decode + chart, median ms:

| | Node series | Node mixed | Chromium series | Chromium mixed | Bun series | Bun mixed |
|---|---|---|---|---|---|---|
| json-dict | 8.39 | 19.48 | 10.69 | 34.61 | 9.48 | 18.51 |
| arrow-dict | 1.01 | 1.29 | 1.13 | 1.13 | 0.84 | 0.90 |
| own-i64-dict | 0.78 | 0.81 | 0.69 | 0.61 | 0.51 | 0.53 |
| own-f64-dict | 0.58 | 0.61 | 0.54 | 0.57 | 0.41 | 0.36 |

Bytes:

| | series gzip | series zstd 3 | series brotli 5 | mixed gzip |
|---|---|---|---|---|
| json-dict | 321 510 | 325 492 | 311 093 | 2 377 936 |
| arrow-dict | 350 836 | 320 119 | 329 450 | 2 253 745 |
| own-i64-dict | 350 420 | 319 913 | 329 217 | 2 252 942 |
| own-f64-dict | 348 990 | 318 108 | 327 409 | 2 253 530 |

What each adds, with the dictionary code in:

| | Rust binary, static musl | browser bundle, minified | gzip |
|---|---|---|---|
| json | +122 912 B | 1 555 B | 810 B |
| arrow | +2 269 184 B | 226 658 B | 53 200 B |
| own | +40 960 B | 4 305 B | 1 883 B |

Retained once decoded and drawn, Node, KiB: json-dict 6 168 on `series` and
15 317 on `mixed`; arrow-dict and own-i64-dict about 1 570 on both;
own-f64-dict 1 177 and 1 179. Every binary decode is views over the received bytes, 4–7
KiB of objects; what a chart then holds is its own typed arrays.

In Rust, `series`: JSON decodes in 7.5 ms, Arrow in 0.8, ours in 1.6 (its
decoder rebuilds the text column by allocating a string a row, which nothing
forces). Encoding `mixed` with dictionaries costs 8.5–11 ms against 1.1–3.4
without in Arrow and ours, because the encoder counts `message`'s distinct values until it
gives up at 25 000; a cap of a few thousand would stop it early.

**Arrow over JSON:** 8.3× and 15× faster in Node, 9.5× and 31× in Chromium;
1.09× larger in gzip on `series` and 0.95× on `mixed`. Taken.

**Our layout over Arrow:** `own-f64-dict` is 1.74× and 2.11× faster in Node,
2.09× and 1.98× in Chromium — under the 2× margin in two of four cells — and
equal in gzip. On the two criteria that can take it, it does not pass, and
1.74× is not 2×. The README's tie-break toward the smaller binary and bundle
was written for a near tie, which this is not clearly; chapter 3 gave a tie to
Arrow, and allowed a win on the binary and the bundle only by margins written
first, and none were.

**So the margins do not choose our layout, and it was chosen as an exception
to them.** Dmitry took it on the cost of the dependency, which the margins did
not weigh: 55× in the binary (2.2 MB, a third of the 6.7 MiB hub measured on
24 October), 28× in the bundle (51 KB gzip), and the weight measured below.

## Time as `i64` or `f64`

`f64` is read as a `Float64Array` over the bytes; `i64` is two `i32` words a
value, combined into a new array. The copy is 0.04–0.19 ms in Node and
Chromium per 50 000 rows, and 391 KiB that a chart keeps. Gzip differs by
0.5% either way.

`f64` is exact for every microsecond from 1685 to 2255, and for every whole
second until about the year 20 000, since a whole second carries six factors
of two; `9999-12-31 23:59:59` survives, a microsecond in 2300 does not. So the
choice is the encoder's, per column: `f64` when every value reads back
exactly, `i64` otherwise, a flag in the column's header. Arrow has no place
for that; `Timestamp` is an `i64`.

## What Arrow costs to carry

Measured after round 2, at Dmitry's question, with the same binaries and
bundles: what Dashbin pays for Arrow before a single frame arrives, and per
frame when frames are small.

| | Arrow | ours | JSON |
|---|---|---|---|
| Rust binary, static musl | +2.27 MB: reading +1.93, writing +0.75 | +41 KB | +123 KB, serde_json, which the API links anyway |
| crates added to the graph | 30: eight `arrow-*`, flatbuffers, chrono, num-*, half, ahash, zerocopy, syn | 0 | 5 |
| browser bundle, minified / gzip | 227 KB / 53 KB, whatever is imported: `tableFromIPC` alone pulls it all | 4.3 KB / 1.9 KB | 1.6 KB / 0.8 KB |
| `node_modules` | 8.1 MB, plus flatbuffers, tslib, json-with-bigint and 2.5 MB of `@types/node` as a runtime dependency | — | — |
| loading the module, Node | 7.2 ms | 1.1 ms | 1.1 ms |
| loading the module, Chromium, fetch included | 8.4 ms | 1.6 ms | 2.0 ms |
| heap once loaded, Node | 1 063 KiB | 122 KiB | 122 KiB |

`cargo bloat` puts the Rust half in `arrow_select` (765 KiB of `.text`),
`arrow_array` (343), `arrow_data` (184), `arrow_ipc` (125), flatbuffers (65)
and `arrow_schema` (61): the reader brings generic kernels for every type
whether a frame uses them or not. The agent only writes, so it would carry
0.75 MB on a 4.6 MiB binary, +16%; the hub reads and writes, 2.2 MB on
6.7 MiB, +32%.

Small frames, decode + chart + every row as text, µs a frame:

| rows | | Node | Chromium | envelope raw / gzip, `series` |
|---|---|---|---|---|
| 1 | JSON | 2.6 | 2.7 | 317 / 175 B |
| | Arrow | 17.5 | 44.8 | 1 608 / 415 B |
| | ours | 2.4 | 4.6 | 168 / 140 B |
| 30 | JSON | 25.1 | 24.7 | 1 609 / 546 B |
| | Arrow | 48.6 | 74.6 | 2 824 / 1 027 B |
| | ours | 30.7 | 37.2 | 1 608 / 726 B |
| 1 000 | JSON | 754 | 786 | 45.6 / 7.6 KB |
| | Arrow, labels as a dictionary | 806 | 838 | 46.2 / 10.3 KB |
| | ours, labels as a dictionary | 736 | 725 | 44.7 / 9.8 KB |

Arrow's fixed cost is its schema, a flatbuffer read and turned into objects
on every frame: 15–25 µs in Node and 40–60 µs in Chromium, and 1.4 KB raw
before the first value. On a dashboard of twenty small panels that is about a
millisecond and 28 KB, which no one would notice; it is the price of the
envelope, and the column it buys is the same one ours carries. At thirty
rows and at a thousand, printing the table costs more than any envelope, and
JSON, whose strings are already strings, is as fast as anything.

## What the margins do not say

- **The network.** At 50 000 rows round 2's binary encodings carry 27–29 KB
  more gzip than JSON and save 7.4–7.8 ms in Node; below about 30 Mbit/s the
  JSON arrives sooner in total. Over HTTPS the binary is the smaller with zstd,
  and JSON still is by 5% with brotli.
  A LAN install without TLS gets gzip, since browsers offer brotli and zstd
  only over HTTPS.
- **Small frames.** A revenue by day is thirty rows; no encoding costs
  anything there. The 50 000-row frames are where a dashboard stalls.
- **A worker.** `JSON.parse` in a worker keeps the page responsive and moves
  the cost rather than removing it; a structured clone back is a copy again.
- **The hub's memory.** A JSON frame from an agent becomes a `serde_json::Value`
  tree before it becomes columns; that peak was not measured, and under a
  64 MiB promise it is the number that would matter most.
- **Interchange.** Arrow is what DuckDB, Polars and pyarrow read. Nothing in
  Dashbin's first version hands a frame to them; an export could convert at
  the edge, and would bring the 2.2 MB with it.

## What follows

- **Labels go as a dictionary in whatever wins.** The rule needs a cap on
  distinct values and an early stop in the encoder.
- **No reader trusts a library's getters.** Arrow's print a decimal wrong;
  ours must be tested against the vectors of chapter 3 whatever carries them.
- **Decided in chapter 3, 25 October:** our layout, as `own-f64-dict`
  measured it, under a contract written there: versioned messages, batches
  independent of each other, the encoding picked per column, strict decoding
  that refuses rather than panics, and decoders fuzzed as a gate.
  `?format=json` stays for the public API; Arrow may come later as an export.
  Round 1's verdict, JSON, stands here as measured, and round 2 decided only
  with the exception above recorded.
