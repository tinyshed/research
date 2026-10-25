# 5. Running a query

**Status: decided, 25 October 2026**, except its numbers, which a load run
of the hub measures, and its cancellation, whose every row is a test before
it is a promise. The first draft's seven forks went through two reviews; what
they changed is listed under *Forks*.

What happens between "a panel wants data" and "the frames are in the
browser". A collector, an alert, a button's flow and the query editor go the
same way: Go Dashbin's rule of **one path** stays, because every second path
grew its own idea of limits, time and errors. The hub connects what exists —
plugins, agents, TinyStore — and is not a database of its own: no
cross-instance joins, no scheduler of its own beside TinyStore's jobs.

## The picture

```
browser ──► hub   run: dashboard D, its panels, range "now-7d .. now", variables {site: kazan}, widths
hub       1 resolve   each panel's query, its reads, the instances their selectors match, access, allowed values
          2 bind      from, to, step, zone and the query's declared parameters, typed and checked
          3 limit     rows, bytes and time for this receiver; calls at once; the budget for frames in flight
          4 run       every (read, instance) at once: in the binary, in QuickJS, a process, an agent
          5 join      a call identical to one still running waits for that one instead
          6 answer    each frame as soon as it is complete; a failure is that frame's error
browser ◄── one stream of messages (chapter 3's wire), a frame per (panel, read, instance)
```

## Who asks

| Caller | What it sends | What it receives as (chapter 3) |
|---|---|---|
| a dashboard | the dashboard, the panels to draw, range, variables, each panel's width | chart, table, single number |
| one panel again | the same for one panel: a zoom, a changed variable | the same |
| the query editor | an unsaved query, the instances to try, parameter values | preview: a cut frame is shown as cut |
| a collector | its query and the window since its last run | dataset: a cut frame is refused |
| an alert | its query and its window | condition: a cut frame is refused |

All of them call one function of the kernel, which takes the scope (who
asks, in which workspace), the request and the receiver, and returns a stream
of frames.

## Which instances

A read names a plugin's function and an **instance selector**: which
instances it runs on. This settles chapter 2's first open question with the
smallest syntax that covers the three scenarios:

- `site=kazan` — instances with that label; `site=kazan, env=prod` — both;
- `site=$site` — a dashboard variable; a variable holding several values
  (`kazan`, `spb`) matches any of them;
- `name=pg-msk` — one instance by its name;
- `all` — every instance of the read's plugin, written out on purpose.

**An empty selector is refused.** A new production instance must not widen
an old query because nobody wrote anything; whoever wants "every site, the
eleventh too" writes `all` or a label and gets exactly that.

The plugin is never part of a selector: a `postgres.sql` read runs only on
instances of `postgres`, so a PostgreSQL text cannot reach MySQL. That is Go
Dashbin's dialect check, now the type of the read. Chapter 2's scenarios
wrote `plugin=stripe`; here that is `all` on a `stripe` read. No negation and
no patterns in the first version.

**A selector that matches nothing says so.** The panel shows "no instance
matches `site=kazn`", never an empty chart.

## Parameters

A call gets one typed list of parameters from two places, and nothing else.

**The kernel's, always present:**

| Name | Type | What it is |
|---|---|---|
| `from`, `to` | `Time` | the range, resolved from `now-7d` once per dashboard refresh, so every panel of one refresh sees the same instant; the kernel takes `now` to the whole second |
| `step` | microseconds | the width of one point: the range divided by the panel's width in pixels, rounded up to a round value (1 s, 5 s, 15 s, 1 min, 5 min, 1 h, 1 d…), never below the dataset's collection interval |
| `zone` | text | the dashboard's time zone, so "today", "this month" and a bucket of one day mean Moscow's day and not UTC's |

**The query's own, declared with the saved query:** a name, a type (one of
chapter 3's: text, int, float, decimal, time, date, bool, or a list of one of
them), whether it may be null, and a default. The value comes from the
dashboard variable of the same name, else from the panel, else the default.
A value the query did not declare is refused by name.

Why declared, when Go Dashbin read the type off the JSON spelling:

- **PostgreSQL has to know the type of a parameter it cannot infer.**
  `where (@site is null or site = @site)` failed in Go Dashbin for every value.
  With the type declared, the plugin prepares the statement with explicit
  types, and the pattern works, null included.
- **A value is converted against its declaration, never guessed.** `"42"`
  for an `int` becomes 42; `"abc"` is refused, naming the parameter.
- **A list is a declared type**, so `site in (@sites)` is expanded by the
  plugin for its engine: `= any($1)` in PostgreSQL, `?, ?, ?` in MySQL.

**A right type is not enough: a viewer picks only what the dashboard
allows.** A dashboard variable says where its values come from — a fixed
list, the instances a selector matches, the values a query returns — and a
value outside that set is refused before anything runs. `site=secret-prod`
is a perfectly typed text.

What Go Dashbin settled stays: a parameter is spelled `@name` and the plugin
binds it through its driver, never as text in the query; `@@version` is
untouched; a value never outlives its call (MySQL session variables leaked the
last query's parameters to the next one).

**In a JS plugin**, parameters arrive as the values of chapter 3's table read
backwards: `bigint` past 2⁵³, a decimal as a string. Time comes twice:

| Name | What it is |
|---|---|
| `fromUs`, `toUs` | **the range**: microseconds as a `number`, exact until 2255 |
| `from`, `to` | the same range as a `Date`, for an SDK that takes dates |

**A read declares the precision its source takes**: microseconds for a SQL
database, seconds for Stripe's `created[gte]`, milliseconds by default. When
the range is finer than that, the kernel widens it outward to the precision
before the call, passes the widened range in both forms, and the frame
carries a note that it covers more than was asked; the panel shows the note.
So a plugin cannot round a boundary on its own and hand back a sum over a
range nobody sees: what it was given is what it answered for, and a range is
never changed without saying so. The ranges the kernel makes itself fall on a
whole second, so a dashboard, an alert or a collector on Stripe is never
widened; a zoom or a range typed to the microsecond is, and says so.

The call's own values come in the second argument, as chapter 6 will write
it: `{ config, id, signal }`.

## Limits

Every call has three: **time, rows, bytes**. Their values depend on the
receiver, as chapter 3's table says, and an agent applies the smaller of what
the hub asked and what its operator allows, as in Go Dashbin.

- **The plugin stops reading at the cap plus one row**, so it knows the frame
  was cut; the kernel counts again and does not trust the plugin's count.
- **A SQL instance also sets a statement timeout on its session**, so a
  query whose cancellation was lost still ends on the database.

Beside the caps of one call, the hub bounds all of them together, at three
levels, because any one of them alone leaves a hole:

| Level | What it stops |
|---|---|
| **calls at once, for the whole hub** | a hundred instances at four calls each being four hundred live calls, each holding a connection, a response or a JS heap |
| **calls at once, per instance**, by what runs it | thirty panels becoming thirty queries on someone's production database |
| **a budget for frames in flight**, with backpressure | thirty calls whose frames each fit their cap and together do not fit the process |

**Calls at once per instance depend on what runs them**, because the
executors are not alike:

| Executor | Calls at once | Why |
|---|---|---|
| a built-in SQL plugin | four, to start the load run with | its driver's pool keeps calls apart |
| a QuickJS runtime | **one** | the host ends a call by sweeping the runtime's timers (`plugin-host/src/main.rs`, `run`), which would stop a neighbour's; a second call waits its turn |
| a process plugin, an agent | four, to start with | their protocol carries several calls by id, and each ends its own |

Two calls in one QuickJS runtime need a scope per call — its timers, its
fetches, its cancellation — and that is not in the first version. A few
runtimes per instance, at about 0.2 MiB each, are the other way, also later.

**The budget counts a frame's batches** from the moment they arrive until
they are written to the last socket that wants them. Three rules keep it from
waiting for nothing:

- **A call's byte cap is never larger than the budget**, which the
  configuration checks. A frame that could not fit an empty budget cannot
  exist; one that reaches its cap is cut, and refused or shown as cut by its
  receiver, at once.
- **Only starting waits.** A call is admitted when the budget has room for
  its first batch, and waits for that within its deadline; a call already
  running that cannot get room for its next batch fails at once as
  `Unavailable: the hub is busy`. Calls holding half-built frames therefore
  never all wait for each other.
- **A slow browser holds only its own stream.** The response is a bounded
  stream: when a browser reads slowly, writing to it waits and the frames it
  has not taken stay counted. A stream that has not accepted a write for a
  few seconds — a hidden tab, a dead connection — is closed with an error,
  and what it held is released; a frame shared by three tabs is held as long
  as the slowest of them, and never longer than that.

What a plugin allocates inside its own runtime is not a frame, and is bounded
where the runtime is: a QuickJS runtime's memory limit and a process's own
memory (chapter 6), an agent's own ceiling (chapter 8).

The numbers are measured, not chosen here: chapter 3's open caps, the
hub-wide count, the budget, the per-instance starting values and how long a
stream may stall all wait for a load run of the hub.

## Cancellation

**A call ends when the last one waiting for it is gone**: the tab closed,
the panel asked again with another range, the dashboard left, a flow's step
timed out, the hub stopping. Nothing waits for a cancellation to be
confirmed, since a hub that did would leak an entry every time a message was
lost. A dropped future and a query stopped on the database are two different
things, and the table says what joins them:

| Where the call runs | What stops it |
|---|---|
| TinyStore, for a dataset | dropping the future: `tinystore::aio` stops the read, measured on 0.1.1 |
| PostgreSQL | a cancel request on a new connection, which the server may or may not act on in time; the statement timeout behind it |
| MySQL | `KILL QUERY` naming the connection that runs it, sent on a second one; the killed query's connection goes back to the pool only once that query has ended, or is closed |
| a JS plugin in QuickJS | the `signal` of its call fires, and every `fetch` it started is aborted |
| a process plugin or an agent | `$cancel` with the call's id, as Go Dashbin's agent already does |

**The cancel is sent by whatever drops the call**, from a guard that runs
when the call's task is destroyed, so it cannot be lost with the task. Each
row of the table is a test before it is a promise; none of them has been run
in Rust, and an aborted `fetch` in our QuickJS host has not been tried either
— the host's tests unloaded instances, which is not the same.

## Calls still running

**A call identical to one still running joins it.** Identical means the
same instance (which also fixes the workspace), the same function, the same
read — its text and its options, so an edited query is another call — the
same parameters after binding, and the same limits. Who asked is not in the
key: the plugin is not told who asked, and access, variables included, is
checked in step 1, before a call can join anything. The day a plugin learns
who asked, the account joins the key.

**One call is one (read, instance), never a dashboard refresh.** A second
tab opening the same dashboard joins the calls still running and runs anew
the ones that have finished, so it never misses the sites that answered
before it came. A joiner gets the call's whole frame: the kernel keeps a
call's batches until its frame is complete — they are in the budget anyway —
and hands the complete frame to every waiter at once; then the entry is
gone, and a later identical call runs again. Sending batches before their
frame is complete would need a replay for joiners, and is not in the first
version.

The shared call is cancelled only when its **last** waiter leaves; Go
Dashbin's warning about `singleflight` was that the naive form lets one
closed tab cancel another tab's panel.

**No cache of results in the first version.** Every refresh reaches the
source, as in Go Dashbin. A cache can be added later without changing
anything above, because the key it needs is the one joining already uses.

## Answers and failures

**Each (read, instance) answers on its own.** Frames go to the browser as
they complete, in the stream of chapter 3's messages, so nine sites draw
while the tenth is still running. A failure is an `error` message for that one
frame, carrying an application error's kind (`Invalid`, `Unavailable`,
`Timeout`, `Forbidden`, `NotFound`) and a message safe to show: a refused
connection does not name the host that refused, as in Go Dashbin.

**An answer knows whether it is complete.** It ends saying which frames
failed or were cut, and a receiver decides by what it does with them:

| What the panel does with the frames | When one is missing |
|---|---|
| draws each instance apart: a line per site, a row per site | draws what came, and names what did not: "msk: unavailable, the agent has been away for 3 minutes" |
| adds instances together: a total, one number over all sites, a stack, shares of a whole | refuses, naming what is missing, as chapter 3 refuses a cut frame to a chart; a sum of nine is not the revenue of ten |
| an alert's condition, a dataset | already refused (chapter 3); chapter 7 says what the alert does then |

**The kernel does not retry a read.** The next refresh is the retry. An
agent's reconnect is its own business (chapter 8).

**A dashboard refresh is one request.** Every panel's frames come back in one
stream, numbered by the wire's frame field. Without TLS, on the LAN install
the first user runs, a browser opens six HTTP connections to a host, and
twenty panels as twenty requests would queue behind each other; one stream
also resolves `now` once for all of them. A panel asks alone only when it
alone changes.

## Reading Dashbin's own data

The built-in `datasets` plugin is an instance like any other, and its reads
take two different things that must not share a word:

- the **instance selector** picks the `datasets` instance, of which an
  installation usually has one;
- the **series match** picks series inside a dataset — `http_requests` with
  `site=kazan, status=500` — and is an ordinary typed parameter of
  `datasets.series`, with the syntax the dataset chapter gives it.

`datasets.series` reads TinyStore Metrics with an aggregation per `step`;
`datasets.events` reads Records. Limits, cancellation and errors are the
same: the kernel has no second path for its own data.

## Who may run what

A call carries the scope. A viewer runs the saved queries of the dashboards
they can see, with the values those dashboards allow, and never sends a
query's text. An editor uses the query editor on instances they may use. A
flow runs as the account that owns it. The plugin is not told who asked; a
plugin filtering rows by person is not in the first version. Chapter 10 owns
the roles; this chapter only says where the check is: in step 1, before
anything runs or joins.

## Against three scenarios

### Ten sites, one dashboard

Revenue by day, ten instances behind ten agents, `site=$site` with "all"
selected, and a number at the top with the total of all ten. One request;
ten calls, at most four per instance, which is one each. `msk`'s agent is
away: nine lines draw and the panel names the tenth as `Unavailable`; the
total refuses to show a sum of nine. Dmitry switches the range from seven
days to thirty: the browser closes the stream, the kernel cancels ten calls,
each agent gets `$cancel`, each PostgreSQL its cancel request, and a new
request starts. A colleague who opened the same dashboard a second later
joined the calls still running and got all ten.

### An alert over three Stripe accounts

Every minute, the alert's query: one read, `stripe.charges` with
`status=failed` and the selector `all`. Three calls, one per QuickJS
runtime, with `from = now-1h` on a whole second, which is what Stripe takes,
so nothing is widened. One
is cut by the row cap, and since an alert's condition refuses a cut frame,
that subject goes to whatever chapter 7 says a failed read means, rather than
counting four of five pages.

### A collector from closed servers

Every 15 s, `scrape.metrics` on `all` ten instances; the receiver is a
dataset, so a cut frame is refused and the collector writes nothing for that
instance. `step` is the interval itself. Two collectors scraping the same
exporter on the same tick join into one call.

## Forks

After two reviews:

1. **Parameters declared on a saved query, with types.** Kept.
2. **One request per dashboard refresh.** Kept.
3. **Partial answers** — changed: per-instance panels draw what came; a panel
   that adds instances together refuses an incomplete answer.
4. **No result cache; identical running calls join** — sharpened: one call
   is one (read, instance); the key holds the instance, the function, the
   read's text and options, the bound parameters and the limits; a joiner
   gets the whole frame.
5. **Bounds** — changed: three levels, calls hub-wide, calls per instance by
   executor (one per QuickJS runtime), a budget for frames in flight; only
   starting waits, a running call that cannot grow fails at once, a stalled
   browser stream is closed; all numbers from a load run.
6. **Selectors** — changed: an empty selector is refused and `all` is
   written out; an instance selector and a series match are two words.
7. **Time in a JS plugin** — changed: `fromUs`/`toUs` are the range and
   `from`/`to` its `Date`; a read declares its source's precision, and the
   kernel widens a finer range itself and says so; its own ranges fall on a
   whole second.

Added: a viewer picks only the values a dashboard variable allows.

## What it asks of other chapters

- Chapter 6: a JS function receives its parameters typed as above and
  `{ config, id, signal }`; a read declares its precision; a QuickJS runtime
  runs one call at a time and has a memory limit; a process plugin honours
  `$cancel`.
- Chapter 7: what an alert's subject does when its read fails or is cut.
- Chapter 8: `$cancel` and the ceiling cross the agent's protocol unchanged.
- Chapter 9: a panel that draws as frames arrive, names what failed, and
  refuses a combined number over an incomplete answer; variables with their
  allowed values.
- Chapter 11: the run endpoint, one bounded stream per refresh.

## Not measured yet

- The hub-wide count, the budget for frames in flight, the per-instance
  starting values, how long a stream may stall, and chapter 3's caps: a load
  run of the hub against the 64 MiB promise.
- Cancellation, row by row of its table: that PostgreSQL's cancel request and
  MySQL's `KILL QUERY` are sent when a task is destroyed, that the killed
  connection is not reused too early, and that `signal` aborts a `fetch` in
  our QuickJS host.
- The cost of `prepare_typed` per call against a cached statement.
