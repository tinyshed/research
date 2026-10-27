# 11. The API and the command line

**Status: draft, first round, 27 October 2026.** Nothing is agreed until
Dmitry says so; the choices a review should look at are under *Forks*.

Dashbin has one public surface, its api, and every client uses it: the
browser, the command line, the terminal's `show`, a chat's bot. What the
interface can do, a script can do, because the interface is a client like any
other. The agent's door (chapter 8) is the one way in with a protocol of its
own.

And the command line is not an afterthought beside the web: a dashboard is a
text file, the terminal and the browser are two windows on it, and someone
who has a TinyStore and wants to see it types one command and sees it.

## One api, every client

- **There are no private endpoints.** The browser calls `/api/v1` like
  everyone else; a feature the interface has and the api lacks is a bug.
- **The command line, the bot and the terminal drawer are clients too**, so
  every one of them is a test of the api the others rely on.
- **Every operation is public on purpose**: Go Dashbin's test over the
  registered operations stays, and fails on one that answers without a
  session and was not meant to.

## Tokens

The credentials are chapter 10's; what crosses the wire is a bearer token, a
session's cookie only in a browser.

```
dbp_2QmFr8s1yXk9…_c41a      a person's token
dbs_…                       a service account's
dbat_…                      an agent's
```

**A token says what it is and checks itself**: a prefix by kind, random bytes,
and a short checksum at the end, as GitHub's tokens have. A token mistyped is
refused without a lookup, a log or a scanner can tell one when it sees one,
and joining a secret-scanning programme, so a token pushed to a public
repository is revoked by itself, is a step for later.

## The shape

- **Resources under `/api/v1`, named as the command line names them**:
  dashboards, queries, instances, flows, alerts, datasets, plugins, agents,
  accounts, groups, tokens, grants, the audit.
- **JSON in and out**, typed as chapter 5 types parameters: a number too large
  for an integer is refused rather than rounded, a time is never guessed out of
  text.
- **A document is saved against its version**: `If-Match` carries the
  version it was read at, and a save made after someone else's answers `412`,
  saying who saved (chapter 9).
- **A listing pages by `limit` and `after`**, as TinyStore's reads do, and
  answers with a shape that cannot be sent back: a dashboard listed carries a
  panel count, never its panels.
- **An error is a problem document** (RFC 9457, `application/problem+json`):
  its `type` is the kind — invalid, not found, forbidden, unavailable,
  timeout, conflict, limit — its `detail` is a message safe to show that says
  what to do, and its `instance` is the request's id. The status comes from
  the kind, never the other way round.
- **A write may carry an `Idempotency-Key`**: the hub keeps the key and its
  first answer for a day in a bucket that forgets, and a retry gets that
  answer instead of a second refund or a second dashboard. A button's run takes
  its operation id from it (chapters 6 and 7).
- **Requests are limited per token and per address** with TinyStore's own
  rate limiter, and the answer says how much is left in `RateLimit` headers;
  past it, `429` with `Retry-After`.
- **Within `v1` a change only adds.** Taking something away is `v2`, served
  beside `v1` for a time, and what is going says so in `Deprecation` and
  `Sunset` headers.

## Running a dashboard

`POST /api/v1/run` is chapter 5's one bounded stream per refresh:

| Client | What it asks for | What comes back |
|---|---|---|
| the browser | `application/vnd.dashbin.frames` | chapter 3's binary wire, message by message |
| anything else | `?format=json` | one JSON object a line — a frame's header, a batch of its rows, an end, an error — as each is ready |

`dashbin show --json` reads the second, and so can `curl`.

## The schema

- **OpenAPI 3.1, generated from the Rust handlers**, as Go Dashbin generated
  it from huma: the code is the one source. The browser's TypeScript types are
  generated from it, and CI fails when they drift from the handlers — Go
  Dashbin's gate, kept.
- **Read at `/api/openapi.json`, printed by `dashbin openapi`**, and drawn as
  documentation on the project's site from the same file. A documentation page
  inside the binary comes only if its weight is measured and found worth it
  (chapter 1).
- **No SDK of ours.** The schema generates a client in any language; the
  TypeScript one the browser uses and the Rust one the command line uses live
  in Dashbin's own repository.

## The command line

### A dashboard is a file

The document a dashboard is (chapter 9) has a spelling for people: TOML, the
same fields, read and written exactly, so a dashboard lives in a repository
beside the code it watches. The terminal draws it, the hub serves it, and the
same file goes both ways.

```
dashbin pull revenue > revenue.dash.toml       # the hub's document, as a file
dashbin show revenue.dash.toml --watch         # edit it, the terminal redraws on save
dashbin diff revenue.dash.toml                 # what the hub would change
dashbin push revenue.dash.toml                 # saved against the version it was pulled at
```

`push` names the version it was pulled at; when the hub's has moved, it is
refused and says to pull and merge, as a browser's save is.

### Seeing a store at once

**`dashbin see` draws any store it is pointed at, before anyone has made a
dashboard**:

```
$ dashbin see ./data
tinystore at ./data
  metrics   42 names    http_requests_total (counter), node_load1 (gauge), rpc_seconds (histogram) …
  records    3 streams  deploys, log, alerts
  sql        1 database app: 7 tables
6 panels suggested, last 24 h ─────────────────────────────────────────
┌ http_requests_total, rate ──────────────┐ ┌ rpc_seconds, p95 ─────────┐
│ ⣀⡠⠤⠒⠉⠉⠑⠢⢄⣀  200 ok  ⠤⠤⠤ 500          │ │ 120 ms ┤ ⢀⡠⠔⠒⠊⠉⠉⠑⠒⠤⣀  │
└─────────────────────────────────────────┘ └───────────────────────────┘
┌ deploys, per hour ──────┐ ┌ log, last 5 errors ──────────────────────┐
│ ▂▁▁▅▁▁▁▂▁▁▁▃            │ │ 11:02 api  slow request  ms=1200        │
└─────────────────────────┘ └──────────────────────────────────────────┘

$ dashbin see ./data --save overview.dash.toml
```

- **It reads what the store says about itself**: a metric's kind and labels,
  a stream's fields, a table's columns. A counter becomes its rate, a
  histogram its 95th percentile, a stream its count per hour and its last
  lines, a table with a time column its rows per day; chapter 9's suggestions
  choose how each is drawn.
- **A TinyStore is read by the built-in `tinystore` plugin**: its directory
  when nothing holds it, or through the server of the program that does, as
  TinyStore's own command line reaches a store another process holds.
  Dashbin's own data directory is a TinyStore, so `dashbin see` on it shows
  Dashbin.
- **It reads other stores the same way**: `postgres://…`, `mysql://…`, a
  Prometheus address — whatever a built-in plugin opens.
- **`--save` writes what it drew as a dashboard's file**, the start of one
  rather than a wizard.

### Without a hub

**In a directory holding a `dashbin.toml`, `see`, `show` and `query` run
without a hub**: the same binary, the same kernel and plugins, in the command
line's own process, on the instances the file names — their secrets from the
environment or asked for, never written in arguments. No server, no account.
A hub is what sharing needs, and what flows, agents and alerts need, since
they run while nobody is looking; the file says so when a command needs one.

### The commands

```
# every day
dashbin see <store | url>                          a store drawn at once
dashbin show <dashboard | file> [--watch]          a dashboard drawn
dashbin query "<sql | promql>" --on <instance>     one question, its answer drawn; --save a name
dashbin open [dashboard]                           the same place in the browser

# dashboards and flows as files
dashbin pull <dashboard> | push <file…> | diff <file>

# the hub, and which one
dashbin serve                                      a hub
dashbin login [hub] | logout | whoami | use <hub>  several hubs, one at a time

# one grammar:  <noun> <verb>
dashbin dashboard  list | new | edit | history | restore | rm
dashbin query      list | new | edit | run | rm
dashbin instance   list | add | test | rm
dashbin flow       list | new | edit | run | runs | pause | resume | rm
dashbin alert      list | silence
dashbin dataset    list | read
dashbin plugin     new | dev | run | test | build | submit | search | install | list | info | upgrade | rollback | rm | hash
dashbin agent      run | add | list | approve | rotate | revoke | disable | rm
dashbin account | group | token | grant            chapter 10's steps
dashbin audit

# the machine
dashbin backup | restore | passwd | owner | status | config explain | telemetry | openapi | completion | version
```

`dashbin agent run` is an agent on this machine (chapter 8); `dashbin agent
add` and the rest manage agents on the hub. One noun does not mean two
things, so running one is a verb of its own.

### What makes it pleasant

- **A person gets a picture, a script gets data**: tables and charts at a
  terminal, one JSON object a line when the output is a pipe, and `--json`,
  `--csv` to say so, as TinyStore's command line does.
- **Tab completes names** — dashboards, instances, flows, a variable's values —
  and a word that matches nothing is answered with what it is likely a typo of.
- **`edit` opens the document in `$EDITOR`** with the drawing beside it,
  redrawn on every save. There is no form to learn: there is a file.
- **`instance add` asks for what the plugin's settings declare**, a secret
  without echo, from standard input or from a variable, never as an argument
  that stays in the shell's history.
- **An error says what to do**, and its kind is the exit code (chapter 12).
- **Web and terminal point at each other**: every panel and dashboard in the
  browser has "copy as command" —
  `dashbin show revenue --var site=kazan --range now-7d` — and `dashbin open`
  opens the browser at the place the terminal is looking at.

## Later

- **`dashbin mcp`**: an MCP server over the same api, reads only, inside its
  token's limits, so an assistant can look at dashboards and data the way the
  person who made the token can. TinyStore's command line has `tinystore mcp`;
  Dashbin's would be the same shape.
- **Pictures in a terminal** through Sixel or kitty's protocol where one has
  them (chapter 9), and `--png` for a chat.

## Against four scenarios

### A TinyStore, in a terminal

A developer has an application on TinyStore and no Dashbin. They run
`dashbin see ./data` and see six panels in a second; `--save` makes a file of
them, `show --watch` lets them fix two titles and a range in their editor, and
`push` puts it on the team's hub, where the browser draws the same thing.

### Dashboards in a repository

A team keeps `dashboards/*.dash.toml` beside its code. CI runs `dashbin diff`
on a pull request, so a reviewer sees what the hub would change, and
`dashbin push` with a service account's token on merge; a push retried after a
dropped connection carries its `Idempotency-Key` and saves once.

### From the browser to SSH

A colleague looking at Kazan in the browser copies the panel as a command and
pastes it to Dmitry, who runs it over SSH on a server with no browser and sees
the same numbers, because both asked the same api with the same variables.

### A token in a public repository

A `dbp_` token is pushed to a public repository by mistake. Its prefix and
checksum make it recognisable to a scanner, and once Dashbin joins a scanning
programme the hub revokes it by itself; until then the owner revokes it with
`dashbin token rm`, and the audit shows when it was made and by whom.

## Forks

1. **No private api**: the browser is a client like any other — against
   endpoints only the interface uses, which are quicker and are where an api
   stops being complete.
2. **The schema generated from the Rust handlers** — against a hand-written
   specification, which drifts from the code the day after it is written.
3. **Tokens with a prefix by kind and a checksum.**
4. **Standard HTTP for what HTTP already has**: problem documents, `If-Match`
   for versions, `Idempotency-Key`, `RateLimit` headers, `Deprecation` and
   `Sunset`.
5. **The browser gets the binary wire, everyone else a JSON line per message.**
6. **Dashboards and flows as TOML files** with `pull`, `push` and `diff` —
   against an export format that is not what is stored.
7. **`see`, `show` and `query` without a hub**, from a project's file.
8. **`see` and a built-in `tinystore` plugin**: Dashbin as the way to look at
   any TinyStore.
9. **One grammar, `<noun> <verb>`**, and `agent run` for running an agent.

## What it asks of other chapters

- Chapter 3: the JSON line per message as the public spelling of a stream.
- Chapter 4: a bucket that forgets for idempotency keys; a dashboard's
  document spelled as TOML, read and written exactly.
- Chapter 5: the run endpoint as both streams.
- Chapter 6: the plugin's commands and its catalog; a built-in `tinystore`
  plugin.
- Chapter 8: `dashbin agent run`.
- Chapter 9: "copy as command", and the drawing `see` and `show` share.
- Chapter 10: `dashbin login`, and the tokens' kinds.
- Chapter 12: the verbs the binary takes are this chapter's map.
- Chapter 13: the crate that derives the schema, the tests of the command
  line, the gate on the browser's types.

## Not checked yet

- `see` on real stores: TinyStore's own research stores, a Dashbin's data
  directory, a PostgreSQL with dozens of tables — whether its six panels are
  the ones a person would have chosen.
- What the command line's drawing and the documentation page would add to the
  binary.
- The JSON line stream against the binary wire for a refresh of twenty panels.
- Reading a TinyStore through the server of the program that holds it.
