# 3. The frame

**Status: decided, 25 October 2026**, except the wire encoding and the caps'
defaults, which wait for measurements. Dmitry approved the third draft. The
first draft's seven forks went through review: a decimal never falls back to a
float, time is microseconds rather than milliseconds, a cut result is accepted
only where a person sees it is cut, and the wire format waits for a
measurement that includes Arrow IPC; a frame is any typed table. A second
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

Hub to browser, agent to hub, process plugin to kernel: one encoding, not
chosen yet. The candidates are JSON, Arrow IPC and a layout of our own; the
first draft's claim that a layout of our own could later "just" become Arrow is
withdrawn, since buffer alignment, metadata and type spellings differ even
where the model is the same. The measurement goes first, in `research`:

- 50 000 rows holding every type, nulls, `i64` bounds, decimals at both ends of
  38 digits, dates before 1970 and long text;
- size on the wire, raw and gzip;
- decoding in the browser, time and allocations, to the point where a chart
  and a table can read the values;
- what each adds to the Rust binary and to the browser bundle.

Whether time crosses as `i64` or as `f64` is measured with it.

The rule is set before the run so the run cannot bend it: **Arrow IPC wins a
tie.** A layout of our own is taken only if it beats Arrow clearly on browser
decoding or on what it adds to the binary and the bundle, by margins the
slice's report writes down before measuring. One more component of our own is
a cost for as long as Dashbin exists.

`?format=json` stays for a person and a `curl`, whatever wins.

## Test vectors

One set of frames, as bytes and as the values they mean, that the Rust codec,
the TypeScript codec, the agent and every SDK must read and write the same,
byte for byte: every type, null in every position, empty frames, `i64` and
`i128` bounds, decimals with negative scales, dates before 1970 and past 2038,
time at both ends of its range, text with every UTF-8 width, and malformed
bytes each consumer must refuse the same way. `Wall` and `Time` holding the
same digits are two different frames in the vectors, so no codec can confuse
them.

## Open

1. The wire encoding, and whether time crosses as `i64` or `f64`, after the
   measurement above.
2. The cap's defaults: rows and bytes, per receiver, from measurements.
