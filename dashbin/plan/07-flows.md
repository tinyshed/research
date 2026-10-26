# 7. Flows

**Status: draft, second round, 25 October 2026; TinyStore 0.2.0 folded in on
26 October.** The first draft went through one review; what it changed is
listed under *Forks*. Nothing is agreed until Dmitry says so.

Everything that runs without a person looking at it: collectors, alerts,
webhooks, buttons. Chapter 2 named them flows of one shape, a trigger and
steps; this chapter says what the shape is, how it runs on TinyStore's jobs,
and what makes running a step twice harmless, since TinyStore runs a job at
least once and says so.

The interface offers a form for each kind — a collector, an alert, a
webhook, a button — and every form writes the same flow underneath. Dmitry's
canvas, plugins' functions as blocks the way Scratch has them, is a later
editor of that same flow, not a second engine.

## The shape

```
flow    = trigger, then steps in order
trigger = schedule (every 15 s | cron "0 9 * * 1-5" in a zone)
        | webhook  (an instance's webhook function)
        | click    (a button on a dashboard)
step    = query    run a saved query (chapter 5); the step is its receiver
        | keep     write the frames before it into a dataset: series or events
        | check    test a condition for each subject, keep each subject's state
        | action   call an action of an instance, once a run or once a subject
        each step may carry `when`: a condition on what the steps before it found
```

Four kinds of step, a list, no loops, no branches but `when`, no code: what a
flow does that a plugin cannot is order and memory, and anything cleverer is
a plugin's function. An alert, written out:

```json
{
  "name": "Failed payments",
  "trigger": { "every": "1m" },
  "steps": [
    { "query": "failed-payments", "params": { "from": "-1h" } },
    { "check": "sum(count) > 5", "for": "10m" },
    { "action": "telegram-ops.send", "when": "state changed",
      "params": { "text": "{{subject.instance}}: {{value}} failed payments in an hour, {{state}}" } }
  ]
}
```

A collector:

```json
{
  "name": "node exporters",
  "trigger": { "every": "15s" },
  "steps": [
    { "query": "node-exporters" },
    { "keep": "node", "as": "series" }
  ]
}
```

A button, and a webhook:

```json
{ "name": "Refund", "trigger": { "click": "dashboard:payments" },
  "steps": [ { "action": "stripe-eu.refund", "params": { "charge": "{{variables.charge}}" } } ] }

{ "name": "GitHub pushes", "trigger": { "webhook": "github.push" },
  "steps": [ { "keep": "deploys", "as": "events" } ] }
```

The JSON is a sketch of what is stored; chapter 11 says how it travels.

## How a flow runs on TinyStore

| Flow | TinyStore |
|---|---|
| a flow with a schedule | one repeating job in a `flows` queue, its id the flow's: `every` gives each flow its own phase within the interval, so a hundred flows every minute do not all start at second zero; `cron` runs on a named zone's wall clock |
| a webhook's delivery | a job whose id is the delivery's, as the webhook function read it from the provider, in a queue with `dedupe`, so a delivery the provider sends twice runs once |
| a click | a job whose id is the operation's (below), so a second click, a retried request or a second tab asking for the same thing runs once |
| a run | the job's run: `at` is when it was due, and attempts, backoff and timeout are the flow's |
| a unit of a run | `run.step(name)`: its answer is kept, and the attempt after a crash or a retry runs only the units left |
| one run at a time | the flow is the job's group, one at a time a group; a run that overruns delays the next instead of overlapping it |
| how far a run got | `set_progress` and `watch`, which the interface shows |
| a provider that asks to wait | `snooze`: the run comes back later without counting an attempt |

**A run is kept in units, not in steps.** A query's frames are never a kept
answer: they may be megabytes, a kept answer is at most 1 MiB, and frames
held in a process die with it. So a query is never a unit of its own; it
belongs to the step that consumes its frames, and the two are one
`run.step`:

| Unit | What it runs | What is kept |
|---|---|---|
| collect | the query and its `keep`, instance by instance | how many rows each instance wrote, and each instance's new window end |
| evaluate | the query and its `check` | the transitions: which subjects moved from which state to which |
| commit | the new states and the notifications they cause, in one transaction | that it committed |
| act | one action, for one subject or for the run | the action's result |

A crash after a unit is kept resumes at the next one; a crash inside a unit
runs the whole unit again, query included. A query is cheap to repeat and
what follows it is made to bear repeating:

| Step | What makes twice harmless |
|---|---|
| query | it only reads |
| check | its answer is the transitions, computed from the frames and the states before the run, and kept as the unit's answer |
| keep, as series | TinyStore Metrics takes the same batch again as nothing, in the head or after packing, measured on 0.2.0. A sample that differs behind what its series has packed — the source changed between the attempts — refuses the whole batch as `too old`, naming the time the series is packed to; the collector drops that series' samples before it and writes the rest |
| keep, as events | each event carries an id, and an append of an id its stream holds writes nothing, measured on 0.2.0: a webhook's event is named by its delivery; a collector's by its instance, its time, a hash of its fields and its place among identical rows of the same read, so the same window read again names the same events and two identical rows stay two |
| action | the operation's id (chapter 6): `flow/run/step/subject`, the same on every attempt; inside the provider's window, a repeat is the same operation |

**A state and the notifications it causes are written together.** An alert
that kept `firing` and died before queueing its message would never send
it, since the next run finds `firing` already. So the commit unit writes, in
one transaction, the subjects' new states to a KV bucket and one notification
job a transition to a queue kept in the same database file — TinyStore keeps
a queue in a database's file when asked, and a job added in a transaction
exists only if it commits. The notification jobs then run on their own, at
least once, each with its operation id. A notification may be sent twice; it
is never lost.

**Missed runs collapse into one**, measured on 0.1.1 and 0.2.0: a job
repeating every second, its store closed for five seconds, ran once on
reopening — its `at` the first time it missed, five seconds before — and then
on its schedule. A collector catches up through its windows, below, and an
alert evaluates now.

## The window a scheduled read covers

**A run reads up to when it was due, not up to now.** A run due at 10:00 that
starts at 10:00:07 reads `[…, 10:00)`, so a late run reads what an on-time
one would have, and two runs never read the same stretch twice.

**A collector reads each instance from where that instance last kept.** The
end of the last window kept is the flow's state **for each read and each
instance**, in TinyStore KV, never one cursor for the whole query: with ten
sites, nine that answered move on, and the tenth, whose agent was away,
keeps its old end and reads the missed hour when it is back. Each
(read, instance) call of chapter 5 gets its own `from`. A window is capped —
a day by default — past which the flow records a gap rather than one
enormous query. A scrape cannot read the past, so for a scrape a missed hour
is a gap, and the panel shows it.

The window end moves only after the instance's rows are written: a crash
between the two reads and writes them again, which a series bears.

**An alert reads a fixed window ending when it runs**: the last hour,
whatever happened before. An alert is about now; a run that comes late after
a restart, due hours ago, would otherwise fire for the past, and backfilling
it would fire for yesterday.

`step` (chapter 5) is the collector's interval.

## Alerts

**A subject is one frame's instance and its dimensions**: for a frame of
`site, count` from `pg-msk`, each site of `pg-msk` is a subject, as chapter 2
said and the series rule of chapter 3 decides what is a dimension. Each
subject has a state of its own.

**The condition is small**: a column, an aggregate over the window — `last`,
`sum`, `avg`, `min`, `max`, `count` — a comparison and a number:
`sum(count) > 5`, `last(ratio) < 0.95`. A condition more involved than that
belongs in the query.

| State | When |
|---|---|
| ok | the condition is false |
| pending | true, for less than `for` |
| firing | true for `for` |
| resolved | was firing, false now; becomes ok after the next run |
| no data | the subject's read failed or was cut, for `for`; or the subject is gone |

The evaluate unit computes transitions from the states before the run; the
commit unit writes the new ones (above), so a transition is reported once
and a crash between the two re-evaluates it rather than losing it.

**A failed read is `no data`, never a silent ok.** Chapter 5 left this here:
a subject whose read fails, or whose frame was cut and therefore refused,
keeps its state until the failure lasts `for`, then goes to `no data`, which
notifies like `firing` unless the alert says it should not. A subject that
stops appearing — an instance removed, a site gone from the frame — goes to
`no data` the same way, and its state is dropped once it is resolved.

**Actions follow transitions**: a step with `when: "state changed"` runs once
for each subject whose state moved, with the subject, its value and its state
to fill the text. `repeat: "4h"` notifies again while a subject stays firing.
The template knows `{{subject.*}}`, `{{value}}`, `{{state}}`, `{{flow}}` and
nothing else.

**A silence** is a time range and a subject match: a silenced subject's state
still moves and is recorded, and its actions are not called.

Each transition is an event in a built-in `alerts` dataset, so the history of
an alert is read like any other data, by the `datasets` plugin.

## Collectors

A query and a keep. The series rule and the events rule are chapter 3's; a
frame neither fits is refused with the reason, and a cut frame is refused
(chapter 5). **One instance failing fails nothing else**: the frames that
came are kept, the one that did not is a gap in its series, and the run's
report names it. A dataset's retention is the dataset's, kept by TinyStore.

**Collectors share one writer.** A scrape of up to 100 000 samples is written
whole, in one write that holds the metrics writer: 2.6 s the first time, when
every series is new, 0.5 s after, measured on 0.2.0. Every other collector's
write waits behind it meanwhile, and reads do not.

**A number its source corrects later** — a day's revenue after a refund —
cannot be corrected in a series once packed
([tinyshed/tinystore#29](https://github.com/tinyshed/tinystore/issues/29),
open), and a collector never reads a window twice anyway. Such a number is
read from its source by the panel; where Dashbin keeps its history is not
decided here.

## Webhooks

**The address is unguessable**: `/hooks/<token>`, a token the hub mints for
the pair of the instance's webhook function and the flow, since a webhook is
the one door that opens without a session.

The function runs on the request, inside the hub's limits: it verifies the
request with the instance's settings, which only it knows how to do, and
returns a response, rows, and **the delivery's id as the provider gives
it** — GitHub's `X-GitHub-Delivery`, a Stripe event's `id`. The hub adds the
run's job under that id, with the rows as its value, and answers the provider
**once the job is on disk** — TinyStore's add returns then — so a provider
that got a 2xx can trust the delivery is kept, and one that did not sends
again and is deduplicated.

**Without an id from the provider there is no deduplication**, and the hub
says so when the webhook is set up: two deliveries with the same body are
not proof of one event sent twice, so a hash of the body is never the id. A
delivery whose rows pass 1 MiB is refused with 413; a larger one is a blob,
later.

## Buttons

A button is a flow with a click trigger, placed on a dashboard. Its params
come from the dashboard's variables, within the values they allow (chapter
5); it may ask for a confirmation first. Clicking needs the dashboard's
`interact` permission, Go Dashbin's third permission, which exists for
exactly this.

**What makes two clicks one operation is decided on the server.** An id the
browser makes for each click event would make a double click two refunds,
and a second tab would anyway. So:

- **An action declares what makes two calls the same operation**:
  `action({ once: ['charge'], idempotency: '24h' }, …)` for a refund — two
  refunds of one charge are one operation, whoever asks and from where. The
  run's job id is the flow, the action and those params; TinyStore's
  `dedupe` makes a second request inside the window add nothing, and the
  same key goes to the provider as its idempotency key.
- **An action that declares nothing** gets an id per request, made when the
  confirmation opens and kept for that request's retries, so a lost answer
  retried by the browser is one run; two tabs are then two runs, and that is
  what declaring nothing means.
- **The browser disables the button** from the first click until the run is
  accepted, and watches it until it is done or failed. That is a courtesy;
  the server's id is the guarantee.

## An action whose outcome is unknown

Chapter 6 handed this here. A call that timed out or lost its connection
after sending may or may not have refunded the charge.

- **An action declares how long its provider keeps an idempotency key**:
  `action({ idempotency: '24h' }, …)` for Stripe. Inside that window the step
  is retried with the same operation id, by the flow's attempts and backoff.
- **Past the window, or for an action that declares none, the step is not
  retried by itself.** The run fails as `outcome unknown`, the interface shows
  it with what was sent, and a person marks it done or runs it again. An
  action without a declared window is assumed to have none.
- A provider's "slow down" is a `snooze`, not an attempt.

## Bounds

- **One run of a flow at a time**, as above.
- **Runs at once across all flows** are bounded, and their plugin calls share
  chapter 6's slots with the dashboards; a flow waits for a slot like a panel
  does.
- **A run has a timeout**, the flow's; when it passes, the run is told to
  stop, and its query is cancelled as chapter 5 cancels one.
- **An instance's own limit is its group's rate.** The notification queue
  groups jobs by instance, and an instance whose provider limits it gets a
  group rate beside the queue's, so two Telegram bots do not share one limit
  — measured on 0.2.0: each of two bots allowed two a second sent two at once
  and two a second later. The rate is counted in the process, which is one hub.

## Who a flow runs as

A flow runs as the account that owns it (chapter 5). An owner who loses
access to an instance a flow reads, or whose account is deleted, pauses the
flow, marked, rather than leaving it to fail every minute or to run as nobody.

## What is stored where

| What | Where |
|---|---|
| a flow's definition | TinyStore SQL: columns for what it references — queries, instances, datasets — so that deleting one a flow uses is refused, as Go Dashbin refused deleting a query a panel draws; its steps as JSON |
| its schedule and its runs | TinyStore Jobs |
| subjects' states, a collector's last window | TinyStore KV, under the flow |
| alert transitions, runs' reports | events in Records, read through `datasets` |

## Against three scenarios

### Revenue of ten sites, collected

Every hour, `revenue-by-day` on `site=$all` into the series dataset
`revenue`. At 13:00 the hub has been down since 10:40: the run due at 13:00
reads each site from where that site's window ended, 10:00, to 13:00;
`pg-msk`'s agent is away, so `msk` keeps its window end at 10:00 while the
other nine move to 13:00, and at 14:00, its agent back, `msk` reads
10:00–14:00 and has no gap. The hub dies after writing the series and before
keeping the unit's answer: the next attempt reads and writes the same samples
again, which change nothing, packed or not. A refund booked tomorrow against
today is not seen: that is #29's case, above.

### Failed payments over three Stripe accounts

Every minute, `stripe.charges` with `status=failed` on `all`, the last hour.
`stripe-us` passes five for ten minutes: `firing`, one Telegram message for
that subject only. Its read then times out three runs in a row, ten minutes:
`no data`, one message saying the alert cannot see `stripe-us`. A silence for
`stripe-us` during a planned migration keeps recording states and sends
nothing.

### A refund button

`stripe-eu.refund` on the payments dashboard, `charge` from a variable;
the action declares `once: ['charge']`. The admin double-clicks, and a
colleague presses it in another tab: one run, because both ask to refund
the same charge. Stripe times out after the request was sent;
inside its 24 hours the step is retried with the same id and Stripe answers
with the refund it already made. Had the hub been down for two days, the run
would stop as `outcome unknown` and wait for a person.

## What TinyStore is asked

The rule is that what is inconvenient in TinyStore is reported, not worked
around. Two things a flow needed were not there in 0.1.1, each measured by a
program (`tinystore-repro/0.1.1`); 0.2.0 closed both, rerun by the same
programs (`tinystore-repro/0.2.0`), so no workaround is built for either:

- **Records: an id on append**
  ([tinyshed/tinystore#23](https://github.com/tinyshed/tinystore/issues/23)).
  A second append of an id the stream holds writes nothing and says so.
- **Metrics: an identical sample behind the packed frontier taken as
  nothing** ([tinyshed/tinystore#24](https://github.com/tinyshed/tinystore/issues/24)).
  NOT-08 still refuses a different one, and the refusal now names where the
  series is packed to.

Two more, asked with five others the same day, bear on flows: a group's own
rate ([#30](https://github.com/tinyshed/tinystore/issues/30), closed in
0.2.0, above under *Bounds*) and a gauge corrected behind what is
packed ([#29](https://github.com/tinyshed/tinystore/issues/29), open: a
rewrite of a packed block is a design change for TinyStore, which suggests a
SQL table upserted by day meanwhile and asks how far back and how often
corrections land).

## Forks

After the first review:

1. **Four kinds of step in a list**, `when` on each, no loops, no code. Kept;
   a run is now kept in units — a query together with what consumes it —
   since frames cannot be kept and a crash between a query and its `keep`
   would have nothing to resume with.
2. **Scheduled reads end when the run was due** — changed: a collector's
   window end is kept for each read and each instance, not one for the query,
   so a site that was away reads its missed hour when it is back.
3. **A failed or cut read is `no data`.** Kept; and a state and the
   notifications it causes are written in one transaction, so a crash never
   loses a notification and may send one twice.
4. **Webhooks** — changed: deduplicated only by the id the provider gives,
   never by a hash of the body; without one, no deduplication, said at setup.
5. **An action retries by itself only inside the window it declares** — and,
   added, declares what makes two calls one operation (`once`), so two tabs or
   two clicks asking for the same refund are one run.
6. **One run of a flow at a time**; runs share chapter 6's slots. Kept, with
   missed runs collapsing into one.
7. **Events written twice**: check TinyStore Records with a repro, then ask
   TinyStore for an id on append. Kept: asked as #23, closed in 0.2.0, and a
   kept event now carries an id.

## What it asks of other chapters

- Chapter 4: the tables and buckets above.
- Chapter 6: `idempotency` and `once` on an action, and a webhook function
  that returns the provider's delivery id.
- Chapter 9: the forms, a run's progress, an alert's subjects and history, a
  button and its confirmation.
- Chapter 10: `interact`, webhook tokens, who may edit a flow.
- Chapter 11: the flow, run and hook endpoints.

## Not checked yet

- A thousand flows on one schedule queue, and a jobs worker handing its runs
  to the plugin thread.
- The commit unit: KV states and notification jobs in one transaction of one
  database file, with the queue kept in that file.
