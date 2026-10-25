# 3. The frame

**Status: decided, 25 October 2026**, the wire included, except the caps'
defaults, which wait for measurements. Dmitry approved the third draft, and
then our own wire over Arrow as a recorded exception to the margins. The
first draft's seven forks went through review: a decimal never falls back to a
float, time is microseconds rather than milliseconds, a cut result is accepted
only where a person sees it is cut, and the wire format waited for a
measurement that included Arrow IPC; a frame is any typed table. A second
review added a type for a wall clock, withdrew a promise of zero-copy time in
the browser, and asked that a column say why it became text.

Every call returns frames, and everything Dashbin does with data — draw it,
keep it, test it in an alert, send it across a socket — reads frames. A panel,
a collector and an alert must not be able to tell which plugin produced one.
The worst outcome this chapter guards against is not a slow chart but a
handsome chart with wrong numbers.

## What Go Dashbin already settled

These held against two production databases and stay:

- **Columnar.** A column is a typed array and a mask of nulls, never a row of
  boxed values.
- **A date is a time axis, not its rendering.** `group by created_at::date` as
  a string broke every time rule of the panel.
- **A type that cannot be charted arrives as the database prints it.**
  `uuid`, `inet`, `interval`, `jsonb`, arrays: text, verbatim.
- **A whole number keeps all its digits.** `9007199254740993` must not arrive
  as `…992`, in a parameter or in a column.
- **A result is read-only once returned**, so one answer can be shared by two
  panels, a cache, or a collapsed request.
- **MySQL's surprises are the adapter's**: no boolean in the result set,
  `0000-00-00` meaning absent, `UNSIGNED` past 2⁶³, time pinned twice.

## A frame is any typed table

A frame is columns of one length, each with a name, a type, an optional unit
and its nulls. Nothing forces a shape on it: `day, revenue, orders, customers`
is a frame exactly as SQL returned it, and so is
`time, site, status, count`. What Go Dashbin had and this drops is the third
way of writing the same data, a column per series with the series' labels on
the column (`count{site=kazan}`), and the `Kind` that told a panel which of the
three it was looking at. A label is always a column of its own, so every
consumer reads one kind of table.

```rust
pub struct Frame {
    pub columns: Vec<Column>,   // names unique within the frame
    pub origin: Origin,         // the instance and its labels, set by the kernel
    pub truncated: bool,        // a row cap cut it
}

pub struct Column {
    pub name: String,
    pub unit: Option<Unit>,
    pub data: Data,
    pub nulls: Option<Bitmap>,       // absent when nothing is null
    pub source_type: Option<String>, // the source's own name for it: `numeric(40,2)`, `timestamp`
    pub note: Option<String>,        // why it is not what its source type suggests
}

pub enum Data {
    Time(Vec<i64>),                                // an instant: microseconds since 1970, UTC
    Wall(Vec<i64>),                                // a wall clock with no zone: microseconds since 1970-01-01 00:00 on it
    Date(Vec<i32>),                                // days since 1970: a calendar day, no time zone
    Int(Vec<i64>),
    Float(Vec<f64>),
    Decimal { mantissa: Vec<i128>, scale: i8 },    // 12345 at scale 2 is 123.45, exactly
    Text(Texts),                                   // offsets into one UTF-8 buffer
    Bool(Bitmap),
}
```

How a consumer reads a table it was not written for — which column is the
time, which are numbers, which are labels — is a rule on top of the frame:
the series rule below, the pivot a chart asks the kernel for, the profile the
suggestions read. None of them needs the frame to be in one shape.

## Time

**One unit for every instant: microseconds.** It is what PostgreSQL and MySQL
store, so a timestamp from either arrives with every digit; milliseconds, the
first draft's choice, would have cut them. A JS `number` holds microseconds
since 1970 exactly from 1685 to 2255, so a chart can work in plain numbers;
nanoseconds stop being exact in a JS number 104 days after 1970.

Whether the browser gets those numbers without a copy depends on the wire, not
on the unit: an `i64` on the wire is read through a `BigInt64Array` and
converted once per column, while an `f64` on the wire — exact for microseconds
in that range — is a `Float64Array` over the bytes as they came. The
measurement below decides which the wire carries.

A unit per column (`ms` here, `ns` there) is not offered: every join, pivot,
comparison and axis would have to normalise first, and a forgotten
normalisation is a chart shifted by a factor of a thousand. Sources finer than
a microsecond — TinyStore's events keep nanoseconds, traces do too — are cut to
the microsecond in a frame; the event keeps its own time in its dataset.

**`Wall`** is a wall clock with no zone: PostgreSQL's
`timestamp without time zone`, MySQL's `DATETIME`. `2026-10-25 10:00:00` of
that kind is neither 10:00 UTC nor 10:00 in Moscow; it is what a clock
somewhere showed. Making it an instant would invent the zone, which is what Go
Dashbin did to MySQL's `DATETIME` by pinning the session to UTC, and that goes.
A `Wall` column is drawn as the clock it is, never moved into the viewer's
zone, and is compared with a `Time` only once a zone is given. A SQL instance
may declare the zone its database writes wall clocks in, a `timezone` setting;
then its plugin turns them into `Time` and says so in `note`.

| Source | Column |
|---|---|
| `timestamptz`, MySQL `TIMESTAMP` | `Time` |
| `timestamp`, MySQL `DATETIME` | `Wall`, or `Time` when the instance declares its database's zone |
| `date`, MySQL `DATE` | `Date` |

**`Date`** is a calendar day. Stored as an instant at midnight UTC, the 1st of
October shows as the 30th of September to anyone west of Greenwich. A date of
birth before 1970 is a negative day, and a panel treats a `Date` column as a
time axis all the same.

## Exact numbers

**`Decimal` is exact or it is not a decimal.** An `i128` mantissa holds 38
digits; the scale is the column's, signed, so PostgreSQL's negative scales fit.
An unconstrained `numeric` has a scale per value, so the column takes the
largest and rescales the rest; a value that then does not fit 38 digits makes
the whole column `Text`, exact digits as printed, never a `Float`, with
`source_type` naming the `numeric` and `note` saying it passed 38 digits. A
chart cannot draw a text column and shows the note, which is the honest
failure; a rounded float that draws a wrong line is the one this type exists to
prevent.

**`Float` is what the source calls a float** (`float8`, `DOUBLE`, a JS
`number` with a fraction), not where a decimal goes when it is large.

## Units

A unit is what a number means, not how it is stored:

```rust
pub enum Unit {
    Bytes, Seconds, Milliseconds, Percent /* 0–100 */, Ratio /* 0–1 */,
    Currency(Currency),              // one currency for the whole column
    CurrencyIn(String),              // the currency is another column's value, row by row
    PerSecond(Box<Unit>),
    Other(String),                   // "°C", "rpm"
}
```

Money is a number with a currency unit: an `Int` in minor units with
`CurrencyIn("currency")` for Stripe, a `Decimal` with `Currency(EUR)` for a
shop's SQL. The yen's zero decimals are known to the kernel's table of ISO 4217
codes, never to a plugin dividing by 100.

An aggregate never adds two currencies: a sum over a `CurrencyIn` column
groups by the currency column, and one that would mix dollars and yen into a
single number is refused. Minor units of one currency are summed as integers,
and the currency's exponent is applied only when the sum is shown.

## From a plugin's rows to a frame

A JS plugin returns plain rows; the kernel builds the columns. Nothing is
guessed, and nothing is converted where a digit could be lost.

| JS value | Column |
|---|---|
| `Date` | `Time` |
| `number`, every value whole | `Int` |
| `number` otherwise | `Float` |
| `bigint` | `Int`, or `Decimal` at scale 0 past `i64`, or refused past 38 digits |
| `string` | `Text`, **never guessed as a time**, even `"2026-10-25T10:00:00Z"` |
| `boolean` | `Bool` |
| `null`, `undefined`, a missing key | null |
| an object or an array | `Text`, as JSON |

- **A mix is joined only where no digit can move.** Whole JS numbers with
  fractional ones make a `Float`, which loses nothing: a JS `number` is already
  an `f64`. A `bigint` with a fraction is refused, since the `bigint` would
  lose digits as a float. An `Int` with a `Decimal` makes a `Decimal` when it
  fits. Anything else is refused with the column and the row:
  `column total: row 37 is text, the rows before were numbers`.
- **A time that arrives as a number** (`1761386400000`) is a time only when the
  plugin says which unit, `columns: { time: 'ms' }`, or when the column also
  holds `Date` values.
- **A column with no value to read its type from** — all null, or no rows at
  all — takes the type the plugin declared, and is `Text` otherwise. A SQL
  plugin always declares: the driver knows every column's type before the
  first row.
- **Columns keep the author's order**: the first row's keys, then any key a
  later row adds.

Process plugins in other languages send frames or rows through their SDK,
which applies the same table.

## A frame kept as a dataset

A collector writes its frames into a dataset. The rule for **series**:

- exactly one `Time` column;
- each number column (`Int`, `Float`, `Decimal`) is a series name;
- every other column is a dimension, and the frame's origin adds the
  instance's labels;
- each distinct set of dimensions is one series, a null skipped.

```
time   site   status  count          series dataset "http", instance labels {env=prod}
10:00  kazan  200     120      →     count{site=kazan, status=200, env=prod}  10:00 → 120
10:00  kazan  500     3        →     count{site=kazan, status=500, env=prod}  10:00 → 3
```

**Precision on the way into TinyStore.** TinyStore Metrics keeps `f64` and
sums exactly, rounded once. An `Int` or a `Decimal` mantissa below 2⁵³ is an
exact `f64`, so a `Decimal` is kept as its mantissa with the scale and the
currency in the series' unit, and a month of payments sums to the cent. A
mantissa past 2⁵³ is refused for a series, with the reason; an **events**
dataset keeps it, since an event's fields keep the decimal's digits as written.

An **events** dataset keeps any frame with one `Time` column, each row an
event, the other columns its fields. A frame that fits neither rule is refused
with the reason, never kept in part.

## A result cut by its cap

A read has a row cap and a byte cap, as Go Dashbin's agent already measured.
Who receives a cut frame decides whether it may:

| Receiver | A cut frame |
|---|---|
| a table, the query editor's preview | shown, with "first 10 000 of more" |
| a chart, a single number, a gauge | refused: a revenue line that stops in the middle of the month is a false picture of the business |
| an alert's condition | refused |
| a dataset | refused |

The refusal says what to do: aggregate in the query, or raise the cap.

## Across a process, a socket and into the browser

**Decided, 25 October 2026: Dashbin's own binary frame wire**, from the hub to
a browser, from an agent to the hub and from a process plugin to the kernel.
JSON stays for the public API, a person, `curl` and simple integrations,
as `?format=json`. Arrow IPC is not a transport; it may come later as an
import and export format, converted at the edge, for the analytical tools
that read it.

### What the measurement said, and what the rules did

[The wire, 25 October](../reports/wire-format-2026-10-25.md) compared JSON,
Arrow IPC and a layout of our own on 50 000 rows in V8, Chromium and
JavaScriptCore, under margins committed before any timing.

- **Round 1, text as text: JSON, by the rule.** Arrow and ours were 2.6–8×
  faster to decode and draw, and 1.31× larger in gzip on the chart frame, past
  the 1.25× allowed. All of that was the offsets of one label column.
- **Round 2, labels as a dictionary in all three, JSON included.** Arrow beat
  JSON 8–31× at 1.09× the gzip, inside the margins. Ours beat Arrow 1.74–2.11×
  at equal gzip.

**Ours over Arrow is an exception to the rules, not their outcome.** The 2×
margin was missed in two cells of four, 1.74× in Node and 1.98× in Chromium;
this chapter had given a tie to Arrow; no margin had been written for the
binary or the bundle. Dmitry took our layout on the cost of the dependency,
which the rules did not weigh: 41 KB against 2.27 MB in the binary, 0.75 MB of
it in an agent that only writes; no crates against 30; 1.9 KB against 53 KB of
gzip in the page, which also spends 7–8 ms and 1 MB of heap loading Arrow and
parses its schema again on every frame. Round 1's verdict stays in the report
as it was measured.

Not measured, and to be: the network, where the break-even of about 30 Mbit/s,
below which JSON arrives sooner, is computed; and the hub under load decoding
agents' frames. Arrow JS's getters print a decimal without its scale, so any
reader is ours whatever the transport.

### The contract

What is fixed now, so that it stops being reinvented. The Rust codec and the
TypeScript one are written against this section and the test vectors, and
nothing else. The probe's layout (`wire-probe/rust/src/own.rs`) is the
measured body of a batch; the preamble, the frame and end messages and the
checks below are new and cost a few dozen bytes a frame.

```
stream    = message*                                  one HTTP body, one WebSocket message each, or a pipe
message   = preamble body
preamble  = "DBFW" | version u16 = 1 | kind u16 | body bytes u32 | 0 u32      16 bytes

kind 1  frame   frame u32 | origin | column count u16 | column*        starts a frame and names its columns
kind 2  batch   frame u32 | rows u32 | encoding* | buffers             rows of that frame, independent of other batches
kind 3  end     frame u32 | rows u64 | truncated u8                    the frame is complete; rows is the batches' sum
kind 4  error   frame u32 | kind u8 | message                          the read failed after it began

column    = type u8 | scale i8 | name | unit | source type | note        strings: u16 length, then UTF-8
encoding  = plain | dictionary | time as f64,  and whether a validity bitmap follows
buffers   = per column, each starting on an 8-byte boundary of the message:
            validity bitmap, then the data of its encoding, lengths derived from rows,
            except text bytes and a dictionary's size, which the encoding declares
```

`frame` is the frame's place in the response, since a query answers with a
frame per instance; `origin` is that instance and its labels. Everything is
little-endian. The content type is `application/vnd.dashbin.frames`;
compression is HTTP's business, never the format's.

1. **Batches are independent given their frame's columns.** A batch carries
   its own dictionaries and leaves no state for the next one, so an agent sends
   a million rows without holding the million, and a reader can draw or drop a
   batch without the rest. The first version may send one batch a frame; the
   format never requires it.
2. **The encoder picks a column's encoding batch by batch.** A text column
   goes as a dictionary when its distinct values are at most one per two rows,
   and the encoder stops counting at a cap, so 50 000 UUIDs cost it a bounded
   look and travel plain. Time goes as `f64` when every value of the column
   reads back exactly, as `i64` otherwise. A reader takes any encoding; a
   writer is never required to use the optional ones.
3. **A version a reader does not know is refused whole.** Version 1 has no
   optional parts to skip: a change of the format is a new version.
4. **A malformed message is an error, never a panic and never a guess.**
   Before it reads or allocates, a decoder checks: magic, version and kind;
   the body length against its limit and against the bytes present; frame
   numbers, so a batch for a frame never started, a frame started twice and an
   end with no start are refused; the columns of a batch against its frame;
   rows against the limit; every buffer length, computed with checked
   arithmetic, inside the body; text offsets starting at 0, never decreasing,
   ending at the byte length and each on a UTF-8 character boundary, the bytes
   valid UTF-8; dictionary codes below the dictionary's size, under nulls too;
   a decimal's scale within ±38 and its mantissa within 38 digits; a time sent
   as `f64` whole and within `i64`; the end's row count equal to the batches'.
   It never allocates more than the message it already holds.
5. **The limits are the reader's**: bytes a message, rows a frame, columns,
   the length of a name. Frames from an agent or a plugin meet the same limits
   as the caps of this chapter.
6. **The decoders are fuzzed as a gate.** One fuzz target for the Rust
   decoder and one for the TypeScript one, seeded with the test vectors, run
   in CI; a crash or a hang fails the build. TinyStore's own binary protocol is
   why this is a gate and not a habit.

`?format=json` mirrors the same model: a whole number past 2⁵³ and every
decimal as a string, a non-finite float by name, a label column optionally as
`"dictionary"` and codes.

## Test vectors

One set of frames, as bytes and as the values they mean, that the Rust codec,
the TypeScript codec, the agent and every SDK must read and write the same,
byte for byte: every type, null in every position, empty frames, `i64` and
`i128` bounds, decimals with negative scales, dates before 1970 and past 2038,
time at both ends of its range, text with every UTF-8 width, labels plain and
as a dictionary, time as `i64` and as `f64`, a frame in one batch and in
several, and malformed bytes each consumer must refuse the same way: a
truncated message, an unknown version, a batch before its frame, an offset
past its buffer, an offset inside a character, a code past its dictionary, a
time with a fraction, a row count whose buffers overflow `usize`. `Wall` and `Time` holding the
same digits are two different frames in the vectors, so no codec can confuse
them.

## Open

1. The cap's defaults: rows and bytes, per receiver, from measurements; and
   the wire's own limits beside them.
2. Whether a frame in memory keeps a label column as a dictionary too, or
   only the wire does.
