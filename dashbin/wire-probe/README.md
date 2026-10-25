# Wire probe

Which encoding a frame crosses a socket in: columnar JSON, Arrow IPC, or a
layout of our own. Chapter 3 of the plan left it open until this measurement.

## What is compared

Two frames of 50 000 rows, built from a fixed seed in `rust/src/data.rs`:

| Frame | What it holds |
|---|---|
| `mixed` | every type of chapter 3: `Time`, `Wall`, `Date` from 1950, `Int` with both `i64` bounds and 2⁵³ ± 1, `Float` with NaN and infinity, `Decimal` at scale 2 with ±(10³⁸ − 1), short and 3–4 KB text in Cyrillic and emoji, `Bool`; 5–10% nulls in every column but two. Random values, so nothing compresses it well |
| `series` | twenty sites scraped every 15 s: time, site, a count on a daily wave, a ratio rounded to four places, an amount in cents; 2% nulls. The shape a chart over a month of metrics reads |

Four encodings of each, written by `rust/src/bin/gen.rs`:

| File | Encoding |
|---|---|
| `json` | `{"rows", "columns": [{"name", "type", "unit", "scale", "values": [...]}]}`; an `Int` column holding anything past 2⁵³ travels as strings, a decimal always does, a non-finite float as `"NaN"` or `"Infinity"` |
| `arrow` | an Arrow IPC stream from `arrow-ipc` 60: `Timestamp(µs, UTC)` for `Time`, `Timestamp(µs)` for `Wall`, `Date32`, `Int64`, `Float64`, `Decimal128(38, s)`, `Utf8`, `Boolean`, the unit in the field's metadata |
| `own-i64` | `rust/src/own.rs`: a header naming the columns, then each column's buffers 8-byte aligned, a validity bitmap where there are nulls |
| `own-f64` | the same with `Time` and `Wall` as `f64`, which a browser reads as a `Float64Array` over the bytes |

`gen` reads every file back and compares it with the frame it came from, so
none of them may lose a digit; JSON needs serde_json's `float_roundtrip` for
that.

In the browser, `js/formats/` has one reader per encoding, each doing three
things:

1. **decode**: from an `ArrayBuffer` to something the next two can read;
2. **chart**: a `Float64Array` each for time, count, ratio and amount, null as
   NaN, and the site as a code per row with the list of names;
3. **table**: the first 1 000 rows of every column as exact text.

`js/run.mjs` checks that the three readers draw and print exactly the same
before timing anything. Arrow's values are read from its buffers rather than
its getters, which print a `Decimal128` without its scale, so `arrow` and
`own` share the code that turns a buffer into a cell, and what differs between
them is the parsing of the envelope.

## How it is decided

Fixed before any timing ran. The sizes of both frames were already known: the
generator had to run to prove the round trips.

- **Preference: JSON, then Arrow IPC, then our own layout**, because each
  later one adds something to keep.
- A later one is taken over an earlier one only if it is **at least 2× faster
  on decode + chart** (median, on both frames, in Chromium and in Node), or
  **at least 1.5× smaller in gzip** (on both frames), and in either case
  **not worse on the other by more than 1.25×**.
- What it adds to the Rust binary and to the browser bundle is reported, and
  breaks a near tie toward the smaller.
- Flexibility is not a criterion: Arrow carries metadata and extension types,
  and a layout of our own is as flexible as its next version, which is a cost.
- `table` is reported and decides nothing: 1 000 rows of text are the same
  work in every encoding once the values are read.
- `own-i64` against `own-f64` answers whether time crosses as `i64` or `f64`,
  and does not take part in the comparison between encodings, where `own`
  counts as the faster of the two.

## Running it

```sh
# the frames and their sizes, in the build image of memory-probe
MSYS_NO_PATHCONV=1 docker run --rm -v <research>/dashbin/wire-probe:/w -v rssprobe-cargo:/usr/local/cargo/registry \
  -v wireprobe-target:/target -e CARGO_TARGET_DIR=/target -w /w/rust rssprobe-musl \
  bash -c 'cargo build -q --release --features all --bin gen && /target/release/gen /w/out'

# what each encoding adds to a static musl binary
MSYS_NO_PATHCONV=1 docker run --rm -v <research>/dashbin/wire-probe:/w -v rssprobe-cargo:/usr/local/cargo/registry \
  -v wireprobe-target:/target -e CARGO_TARGET_DIR=/target -e CC_x86_64_unknown_linux_musl=musl-gcc \
  -w /w/rust rssprobe-musl bash size.sh

cd js && bun install
node check.mjs                      # the readers agree, nothing timed
node --expose-gc node.mjs           # V8
bun node.mjs                        # JavaScriptCore
bun bundles.mjs                     # what each reader adds to a bundle
bun build browser.mjs --outdir dist --target browser && node serve.mjs   # then open the address it prints
```
