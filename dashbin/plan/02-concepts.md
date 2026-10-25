# 2. Concepts and words

**Status: decided, 25 October 2026.** Dmitry approved the words after the
four questions of the first draft were answered in review: the repeatable
kind is `read`; a dataset is data Dashbin owns; a query may hold several reads;
the workspace is one implicit `default`. Three scenarios then tested the words
and added two (`label`, `subject`) and one rule (a flow runs queries). The
three questions under *Open* are small and do not move a word.

Every plugin design that read as unnatural in October did so through its
words: "connection", "handle", "collector", "notifier" and "source" each meant
two things in two messages. This chapter gives every word one meaning, so the
later chapters can be written in them. A word not here is not a concept of
Dashbin.

## The picture

```
plugin ── provides ──► functions: read · action · webhook · stream
   │
   └─ configured once with settings ──► instance (stripe-prod, labels) ── placed on ──► hub | agent site-3
                                            │
                    query = one or more reads of instances, with parameters, run by the kernel
                                            │
      ┌─────────────────────────────────────┼──────────────────────────────────────┐
  panel on a dashboard                  flow (runs alone)                       button
  draws the frames                      collector · alert · webhook             runs an action
                                            │
                              dataset: data Dashbin owns (series, events)
```

## Who

| Word | Meaning |
|---|---|
| **installation** | one hub and its data directory |
| **workspace** | the boundary of access inside an installation, not a folder. The first version has one, `default`, which nobody sees; every request still carries it |
| **account** | a person who signs in; has one **role**: owner, admin, editor or viewer |
| **operator** | whoever runs the server an agent sits on; may not be the installation's admin, and decides what runs there |

## What runs

| Word | Meaning |
|---|---|
| **plugin** | a package that provides functions and visualizations: a JS bundle, a process in any language, or built into the binary. Its id is its package name |
| **function** | one export of a plugin, named by the export (`charges`), which is its id forever; renamed only with an alias. Its title and translations are for the interface |
| **read** | a function that may be called again and again with no effect outside: a dashboard, a schedule and an alert may call it |
| **action** | a function that changes something outside (sends a message, refunds a charge); called only on purpose, by a click or a step of a flow |
| **webhook** | a function called by a request from outside to an address the hub gives it |
| **stream** | a function that keeps producing while its instance is enabled (MQTT, a log tail); process plugins first |
| **settings schema** | what a plugin declares it needs (`secret`, `text`, `url`, `number`); read when it is installed |
| **settings** | the values an instance was given; they reach a function with the call, never at load |
| **instance** | a plugin configured once, under a name the user picks (`stripe-prod`, `pg-site3`). Two accounts of one service are two instances. Nothing forbids two instances to share a credential later, through a secret kept by the kernel |
| **label** | a `key=value` on an instance (`site=kazan`, `env=prod`) by which queries, panels and flows select instances; there is no "site" entity |
| **placement** | where an instance runs: the hub, or an agent |
| **agent** | the same binary in agent mode on another server, running the instances placed on it under its operator's configuration |
| **call** | one execution of a function of an instance; carries **parameters** (what the caller asks: `from`, `to`, `step` and the function's own) and gets from the kernel the settings, a call id and a signal to stop |
| **frame** | the columnar table every call returns; a plugin may return plain rows and the kernel builds the frame |

## What people build

| Word | Meaning |
|---|---|
| **query** | a saved request: one or more reads and their parameters. Each read names a plugin's function and the instances it runs on, by name, by label or by a dashboard variable. The kernel runs the reads in parallel and returns their frames, each marked with its instance; a panel only draws. Joining frames across instances is not in the first version |
| **visualization** | a way to draw frames: a line, a table, a plugin's own element |
| **panel** | a visualization of one query, placed on a dashboard with its options |
| **dashboard** | panels on a grid under one time range and its variables |
| **variable** | a dashboard's own parameter: which instances, which currency; feeds the queries of its panels |
| **repeat** | a panel drawn once for every instance a selector matches, so an eleventh site appears without editing the dashboard |

## What runs alone

| Word | Meaning |
|---|---|
| **flow** | a trigger and steps the kernel runs, each step a step of a TinyStore job. A step runs a query or calls an action |
| **trigger** | what starts a flow: a schedule, a request to a webhook, a click |
| **collector** | the flow "on a schedule, run a query, keep its frames in a dataset"; a form in the interface, a flow inside |
| **alert** | the flow "on a schedule, run a query, test a condition for each subject, call actions when a subject's state changes"; states are ok, pending, firing and resolved, and a silence mutes it |
| **subject** | what one state of an alert is about: the instance and labels of one frame (`pg-site3`, `site=kazan`). An alert over ten sites has ten subjects and fires for each on its own |
| **button** | the flow "on a click, call an action" |
| **dataset** | a named set of data that Dashbin owns and keeps itself: **series** (numbers over time, in TinyStore Metrics) or **events** (rows with a time, in TinyStore Records). A table in someone's PostgreSQL is not a dataset; the history Dashbin collected from it is. Collectors and webhooks write datasets; the built-in `datasets` plugin reads them like any other instance. The interface says "Metrics" and "Events" |

## What keeps it safe

| Word | Meaning |
|---|---|
| **network policy** | where an instance's `fetch` may go; private addresses only for hosts an operator typed into a `url` setting, link-local never |
| **trust** | what an admin accepts when installing a plugin: its code sees its own settings. A process plugin needs a separate "native code" consent |
| **pin** | an agent runs a JS plugin only if its operator pinned the bundle's hash; built-in plugins ship inside the agent's binary |

## The words against three scenarios

### A SQL dashboard over ten sites

Ten shops, five on PostgreSQL and five on MySQL, each server closed off and
reached through an agent.

- Ten **instances**: `pg-msk`, `pg-spb`, …, `my-kazan`, … of the built-in
  `postgres` and `mysql` plugins, each **placed** on its site's agent, each
  with a **label** `site=…`.
- One **query**, "Revenue by day", with two **reads**: `postgres.sql` with a
  PostgreSQL text on the instances labelled `engine=postgres`, and
  `mysql.sql` with a MySQL text on `engine=mysql`. A SQL text written for one
  engine cannot be run on the other, because a read names its plugin's
  function: Go Dashbin's dialect check becomes a type of the read.
- One **panel**, a line per site; or the same panel with a **repeat** over
  `site`, one chart per shop.
- A **variable** `site` narrows every panel of the **dashboard** to one shop.

The words held. What it asks of later chapters: parameters of a SQL text are
typed (`@from`, `@to`), and a frame carries its instance so the panel can name
the line.

### Metrics from closed servers

Every server runs node_exporter; the hub must keep CPU and memory history.

- Ten instances of a built-in `scrape` plugin, `node-msk`, …, each placed on
  its agent with `url=http://127.0.0.1:9100/metrics` and a label `site=…`.
- One **query**, "node exporters": one read, `scrape.metrics`, on the instances
  matching `plugin=scrape`.
- One **collector**: every 15 s, run that query, keep its frames in the
  **dataset** `node` (series).
- Panels read `node` through the `datasets` plugin; an agent that was away
  leaves a gap in the series, and the panel shows it.

What it changed: a collector over ten servers must be one flow, not ten, so a
**flow runs a query, not a bare read**, and the query's selector picks the
instances. What it asks of later chapters: the rule that turns a frame into
series (time, numbers, and text columns as dimensions) belongs to the frame.

### An alert to Telegram

Failed payments above five in an hour, for each of three Stripe accounts.

- Instances `stripe-a`, `stripe-b`, `stripe-c` of the `stripe` plugin, and
  `telegram-ops` of `telegram`.
- One **query**, "Failed payments, last hour": one read, `stripe.charges` with
  `status=failed`, `from=-1h`, on `plugin=stripe`.
- One **alert**: every minute run the query, `count > 5` for 10 minutes; three
  **subjects**, one an account; on firing and on resolving, the **action**
  `telegram-ops.send` with a text that names the subject.

What it added: an alert over several instances keeps one state each, which
needed the word **subject** ("alert instance" would have reused **instance**).

## Words retired

| Word | Why it goes |
|---|---|
| source | meant both "a database we read" and "anything that produces frames"; it is an instance whose plugin has reads |
| connection | meant both a credential and a plugin's handle; it is an instance's settings |
| collector as a plugin kind | a collector is a flow around a query, not a kind of plugin |
| notifier | an action an alert calls |
| `query` as a function kind | collided with the saved query; the kind is `read`, paired with `action` |
| alert instance | collided with instance; it is a subject |
| `ctx`, `env`, handles | parameters, settings and the call's own values replace them |

## From Go Dashbin

| Go Dashbin | Rust Dashbin |
|---|---|
| a source (`catalog.Source` row with sealed credentials) | an instance of the built-in Postgres or MySQL plugin |
| a source behind an agent | an instance placed on an agent |
| a saved query, `written_against` a source | a query whose read names a plugin's function; the instances are chosen by name, label or variable |
| a dialect check | the type of a read: a `postgres.sql` text runs on instances of `postgres` |
| `run-many` across sources | a query with reads on several instances |
| `panels.source_id` | the panel's query and the dashboard's variables |
| the "Source, collector, store" table | instance, flow, dataset |

## Open

1. How a selector is written: `plugin=scrape`, `site=kazan`, a variable — one small syntax for queries, repeats and flows alike.
2. Whether an instance's labels are typed by the admin only, or a plugin may suggest some (`engine=postgres` from the built-in plugin itself).
3. Shared credentials: when two instances need one secret, which chapter introduces the kernel's secret that settings refer to.
