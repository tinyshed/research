# 8. Agents

**Status: draft, second round, 27 October 2026.** The first round went
through one review; what it changed is listed under *Forks*. Nothing is
agreed until Dmitry says so.

The first user runs ten sites on ten servers that cannot see each other, and
none of them may open a port. An agent is how their databases reach a hub
they can see: it sits beside a database, dials out to the hub's own port, and
answers for the instances its operator configured and nothing else.

```
a site's server, closed to the outside        the hub, one open port
  agent ───── outbound wss:// ─────────────►  /agent
        ◄──── on the same connection: "run sql on orders"
        ───── the answer's frames ─────────►
```

**The agent dials, the hub never does.** A closed server may still connect
out, so an agent passes any NAT and any firewall that lets a browser out;
only the hub has to be reachable, and it already is for its browsers.

Go Dashbin built this and proved it on two servers; most of it carries over,
and this chapter says what changes now that an instance is a plugin's and the
agent is the same binary as the hub.

## What the hub knows and what the agent knows

| Who | Knows |
|---|---|
| the hub | an agent, its tokens' hashes, and for each instance on it: the id the agent gives it, its plugin, the plugin's version and functions, what the operator allows it, the labels the admin gave it |
| the agent | the hub's address, its own token, and for each instance: its plugin, its settings and secrets — a database's host, user and password, a Stripe key |

The hub never sends "connect to 10.0.0.8 with this password". It sends "run
`sql` on the instance you call `orders`", and the agent resolves that against
its own file.

**What this protects, and what it does not.** A hub taken over cannot learn a
credential, point an agent at a new host, reach an instance an operator did
not configure, run an action an operator did not allow, or run JavaScript
whose hash an operator did not pin. **It can read everything the approved
instances can read**: `select * from customers` is a read, and the agent runs
it if the database user may. The agent guards secrets and authority, not the
data from the hub that asks for it. What bounds that is the database user's
grants — a user that sees only what dashboards need — and the agent's
ceilings. A list of allowed queries or tables kept by the agent is not in the
first version.

## The operator's file

The agent is `dashbin agent --config /etc/dashbin/agent.toml`: the same
binary as the hub, in another mode. Everything the agent does is in that one
file, which belongs to whoever runs the server, who may not be the hub's
admin:

```toml
hub = "wss://dashbin.example.com/agent"
token = "dbat_7Q…"                     # the hub gave it once, on adding the agent
# tls.ca = "/etc/dashbin/hub-ca.pem"   # or tls.pin = "sha256:…", for a self-signed hub

[limits]                               # the agent's ceilings; the hub may ask for less
rows = 100_000
bytes = "64MiB"
duration = "60s"
calls = 8

[instance.orders]                      # "orders" is the instance's id, forever
plugin = "postgres"
title = "Orders, Kazan"
labels = { site = "kazan" }            # a suggestion; the hub's admin decides
[instance.orders.settings]
host = "127.0.0.1"
database = "shop"
user = "dashbin_ro"
password = "…"                         # or AGENT__INSTANCE__ORDERS__SETTINGS__PASSWORD

[instance.node]
plugin = "scrape"
settings = { url = "http://127.0.0.1:9100/metrics" }

[instance.stripe]
plugin = "stripe"
bundle = "sha256:9f2c…"                # the JS bundle the operator pinned
allow = ["read", "action"]             # reads only, unless written here
settings = { key = "rk_live_…" }
```

- **It is read with TinyStore's config loader, without a store**
  (`tinystore::config::load`, measured on 0.2.0): the file, then the
  environment over it, the same errors that name the file and the variable.
  Nothing is written beside it.
- **The agent refuses to start when the file is readable by anyone else**, on
  Unix, as Go Dashbin's did: a secret that never leaves the server is still a
  secret sitting in `/etc`.
- **A setting a plugin declares secret is masked** wherever the agent prints
  it, as on the hub (chapter 6). The plugin's own `settings()` says which.
- **A database is opened on its first call**, not at start, so one that is
  down is a panel that says so and not an agent that will not dial.
- **The instance's key is its id.** The hub binds a flow to
  `(agent, "orders")`. Renaming the key is a new instance that waits for
  approval, and the old one reads as removed; `title` is what changes freely.
  Go Dashbin generated an id and kept it in a state file beside the
  configuration; a key in the operator's own file is one fact fewer.

## Placement and approval

**An instance on an agent is still an instance of its plugin.** What changes
is where it runs: chapter 2's placement is `hub` or `agent`, and nothing above
a call knows a database is a thousand kilometres away. The hub's row says
`{ "kind": "agent", "agent": …, "id": "orders" }` or `{ "kind": "hub" }`, as
Go Dashbin's api already answered.

**What an agent announces is compared, never stored.** On every connection the
agent lists its instances; the hub compares the list with what was approved:

| State | What it means |
|---|---|
| discovered | announced, approved by nobody, reachable by nothing |
| approved | bound to an instance row; queries, panels and flows may reach it |
| changed | announced differently in a way that bears on trust: another plugin, or more allowed than before (`action` where it was `read`); it keeps what was approved until the admin looks |
| removed | approved, and no longer announced |

A tampered agent could otherwise announce `production-orders` and collect
somebody's panels, which is why nothing runs before approval. A changed line
shows what was approved and what is announced now, side by side. A title or
a narrower `allow` is not a change; it is shown.

**A new plugin version is checked against what uses it, not trusted again.**
The greeting carries each instance's functions with their params and columns.
When they no longer fit a saved query or a flow that reaches the instance — a
function gone, a parameter's type moved — those say so, by name, before a
call fails on them. A version whose functions still fit is shown and nothing
else.

**Labels are the hub's.** The operator's `labels` are a suggestion the admin
sees when approving; what an instance is labelled — and therefore which
selector reaches it — is decided on the hub, so an agent cannot announce
itself into `site=$all`.

Whether an agent is connected is the live registry's, held in memory and in
no column, so no row says "unavailable" about a process that came back an
hour ago.

## Identity

**An agent is who its token proved.** The token is a row in the same table
as a person's session, stored as SHA-256, shown once when an admin adds the
agent. The name in the agent's greeting is not read. A connection that ends
removes itself from the live registry only while it is still the one
registered, so a reconnect is not undone by the connection it replaced
noticing later.

**The hub cannot write the agent's file**, so a new token reaches the agent
only through its operator, and replacing one comes in two kinds:

| Act | What it does |
|---|---|
| add | a token, shown once |
| rotate | a new token, shown once, valid beside the old; the first connection that proves the new one revokes the old in the same write; until then the agents page says the agent is still on its old token |
| revoke now | for a token that leaked: every token the agent had is revoked and the connection hung up in one write; the agent stays away until its operator pastes a new one |
| disable | revokes and hangs up; its instances stay and read as unavailable |
| delete | the agent, its tokens and its instances; refused while a flow names one of them (chapter 7), and a panel that selected them by label stops finding them |

The hub hangs up after the write rather than inside it, so a transaction
never holds the writer while a socket closes.

## The connection

**One outbound connection, to the hub's own port.** The agent dials
`wss://…/agent` with its token as a bearer, and the hub's listener upgrades
it to one WebSocket; a person's session does not open that door, and the api
a browser uses is not on it. Nothing listens on the agent's server.

- **TLS trusts the system's roots, or a CA file the operator names**
  (`tls.ca`), or a pinned certificate hash (`tls.pin`) for a hub with a
  self-signed certificate. Plain `ws://` is allowed for a hub on `127.0.0.1`,
  which is how a tunnel looks, and refused otherwise.
- **Liveness is the socket's own ping**: every 15 s, and a connection silent
  for 45 s is closed. 15 s is also under every idle close between the two:
  Cloudflare closes a quiet WebSocket after about 100 s, nginx's default
  after 60.
- **An agent that cannot reach the hub waits, it does not stop**: one second,
  doubling to thirty, with a fifth of that as jitter so ten agents that saw
  the same outage do not dial in one volley; a connection that lasted a minute
  starts the wait over.
- **An agent connecting twice** keeps the hub's newer connection; the older is
  closed, and its calls fail as unavailable.

**Why a WebSocket, when newer transports exist.** What matters between an
agent and a hub is passing whatever stands between them:

| Transport | Through Caddy, nginx, Cloudflare Tunnel, a corporate proxy | Streams with their own flow control | Over a process's stdin and stdout |
|---|---|---|---|
| WebSocket | everywhere | ours, below | the same protocol |
| HTTP/2, gRPC | gRPC not on Cloudflare Tunnel's public hostnames; a proxy that buffers a request body hangs the agent, which is why Go Dashbin left it | yes | no |
| QUIC, HTTP/3 | needs UDP and a second open port; not passed to the hub by Caddy or the tunnel | yes | no |
| WebTransport | made for browsers; not something a proxy or a tunnel can be counted on to pass | yes | no |

What the newer ones give for real is a stream per call with its own flow
control, and the call protocol carries that itself (*Limits*), so it works the
same over the socket and over a process plugin's pipes.

## A hub behind NAT

An agent needs nothing to pass NAT; a hub needs an address. When it has no
public one — at home, in an office:

- **Cloudflare Tunnel, free.** `cloudflared` beside the hub dials out to
  Cloudflare and gives the hub a public HTTPS name; it needs a domain on
  Cloudflare, on the free plan. It passes WebSockets whole, and the hub needs
  nothing of its own for it: the 15 s ping keeps the agents' connections open,
  and `X-Forwarded-Proto` tells the hub its cookie travels over TLS
  (chapter 10). Dashbin ships the recipe, not `cloudflared`, which is
  Cloudflare's program and account.
- **Not a quick tunnel.** `trycloudflare.com` gives a random name that changes
  every time `cloudflared` restarts, with no availability promised; every
  agent's file would point at a name that is gone.
- **Tailscale** works too, and asks every server to join a private network,
  which is a step more for every operator.

Punching through NAT the way WebRTC does is not needed: it needs a server to
meet at anyway, and an agent already reaches the hub by dialling out.

## The protocol

**What crosses is a call, the same call a process plugin gets** (chapter 6):
the request, its frames or its result or its error, credit, and `$cancel`.
Over a process's stdin and stdout it is the same messages behind a length;
over the socket a JSON-RPC document is a text message and a frame message is
a binary one.

| Direction | Message | Carries |
|---|---|---|
| agent → hub | `agent.hello` | the call protocol's versions it speaks, the frame wire's versions it writes, the binary's version, and the instances: id, title, plugin and version, its functions with their params and columns, what is allowed, suggested labels |
| hub → agent | `agent.welcome` | the call protocol's version and the frame wire's version the hub chose from those |
| hub → agent | `call` | the call's id, the instance's id, the function, its typed params (chapter 5), `from`/`to`/`step`, what the hub asks for — rows, bytes, duration — and the call's first credit |
| hub → agent | `credit` | a call's id and how many more bytes it may send |
| agent → hub | frames | binary messages: the call's id, then a message of chapter 3's wire — frame, batch, end or error |
| agent → hub | result | an action's answer, as JSON |
| either | `$cancel` | a call's id; nothing waits for it to be confirmed |
| hub → agent | `bundle` | a JS bundle's bytes, offered for an instance whose plugin is JS (below) |

- **Both versions are agreed before any call.** The hub speaks its own call
  protocol and the one before, and reads its own frame wire and the one
  before. An agent with nothing in common connects, says what it speaks, and
  its instances read as "upgrade the agent", or "upgrade the hub" the other
  way; nothing connects and then sends frames the other side cannot read. The
  binary's version is shown, never compared.
- **An error crosses as the kind it left as** — invalid, unavailable, timeout,
  limit and the rest — with its safe message; the cause stays on the agent and
  in its log. A refused connection does not name the host that refused it.
- **The session owns what is in flight.** An answer that comes late, twice or
  for a call nobody made is dropped; a call whose id is in flight is refused;
  a connection that ends fails every call waiting on it.
- **The envelope knows nothing of what it carries**, as in Go Dashbin:
  JSON-RPC's id, method and raw params, and the vocabulary beside it. Dispatch
  — who runs a call, what is in flight — is the hub's and the agent's, not
  the protocol's.

## Limits

**The ceiling is the agent's; the hub may only ask for less.** For each call:

```
effective = min(what the hub asked, what the instance allows, what the agent allows)
```

| Ceiling | Why |
|---|---|
| rows | a chart needs a count |
| bytes | one row holding one long string is as large as it likes; the agent counts the bytes it encodes and refuses its own answer past the ceiling |
| duration | the statement timeout a SQL plugin sets, and the call's end for any other |
| calls | at once, across the agent; the hub's four per instance (chapter 5) sits under it |

**A call sends only what the hub has room for.** Checking a budget after the
bytes arrived does not keep them out of memory: eight calls each sending a
16 MiB message are 128 MiB in the hub before anything can refuse them. So:

- **Batches are small**: about 256 KiB, the encoder's target. 16 MiB stays
  the reader's hard limit on one message, which no batch comes near.
- **Each call has credit**: the hub opens a call with a window — 1 MiB to
  start with — and sends more as it hands batches on, to the browser or to
  the step that keeps them. An agent out of a call's credit stops reading that
  call's source, the database cursor included, and holds at most the batch it
  was encoding; it never queues what it may not send.
- **An agent's calls together have credit too**: never more than that
  agent's share of chapter 5's budget for frames in flight, so ten agents
  cannot each fill the hub.
- **Batches of several calls interleave**, so a big answer holds a small one
  back by one batch at most, and a `$cancel` or a `credit` waits behind one
  batch, not behind an answer.

A slow browser therefore slows its own call down to the database it reads,
not the socket and not the other calls on it. The window, the batch size and
the shares are starting values, for the load run to set.

## Plugins on an agent

| Plugin | How it gets there | What the operator writes |
|---|---|---|
| a built-in — `postgres`, `mysql`, `scrape`, `probe`, `notify` | inside the binary; its version is the binary's | `plugin = "postgres"` |
| a JS plugin | the hub offers the bundle's bytes; the agent runs it only if its hash is the one pinned | `plugin = "stripe"`, `bundle = "sha256:…"` |
| a process plugin | the operator installs it on the server | `command = ["bun", "/opt/slack/index.js"]` |

- **A bundle that does not match its pin does not run**, and the instance says
  so. A new version on the hub reaches the agent only when its operator pins
  the new hash.
- **Kinds are reads unless `allow` says otherwise.** An `action` or a `stream`
  runs on an agent only when its operator wrote it.
- **The operator's line is the second consent** chapter 6 asks for a process
  plugin; it runs as the agent's user and reaches what that user reaches. It
  speaks the same call protocol, credit included.
- **A JS plugin on an agent costs what it costs on the hub**: a QuickJS
  runtime, unloaded when idle with its bytecode on disk. An agent that runs
  one needs a directory for that (`state = "/var/lib/dashbin"`); an agent that
  runs only built-ins writes nothing.

## What runs where

**Flows, schedules and webhooks run on the hub.** The agent has no scheduler
and no queue: a collector due at 13:00 is the hub calling the agent at 13:00.
A webhook needs an open port, which is what an agent exists not to have.

**What an agent cannot read later is a gap while it is away.** A SQL read can
be asked for the hour it missed, and chapter 7's windows do that for each
instance. A scrape, a probe and a stream cannot: their past is gone, so an
hour without the agent is an hour without their samples, and the panel shows
the gap. Buffering them on the agent until it is back waits for someone who
needs it.

## When it breaks

| What happens | What the hub sees |
|---|---|
| the agent dies mid-call | the call fails as unavailable; the panel names the instance and how long its agent has been away |
| the agent stops answering pings | the connection closes after 45 s; its instances read as unavailable |
| the hub restarts | every agent waits and dials again; nothing is lost but what was in flight |
| a database behind the agent is down | that instance's calls fail with the database's safe message; the agent's other instances answer |
| a browser stops reading a big answer | that call runs out of credit and its cursor stops; the agent's other calls go on; chapter 5 closes the stalled stream |
| the agent's file names an instance the hub never approved | discovered, and nothing calls it |
| an operator widens `allow` | changed: the instance keeps what was approved until the admin looks |
| a token was rotated and the operator has not pasted it yet | the old one still works, and the agents page says so |

## Memory

The agent is the hub's binary, with jemalloc's decay set to zero at start in
agent mode: about 6 MiB idle, measured against 5 for a musl build
([memory and allocators](../reports/memory-and-allocators-2026-10-24.md)).
Credit bounds what it holds for a call to about one batch. With its built-in
SQL plugins under load, with a JS plugin, and over a slow socket it is not
measured yet.

## Against three scenarios

### Ten sites, closed from each other

Each server runs `dashbin agent` with one `[instance.orders]` — `postgres` on
five, `mysql` on five — and a read-only user that sees the tables dashboards
need. The admin adds ten agents on the hub, pastes each token into its
server's file, approves ten announced instances and labels each `site=…`. A
panel reads `revenue-by-day` on `site=$site`; the hub calls ten agents, one
call each. `msk`'s agent is away: nine lines are drawn, the tenth is named,
and the collector reads `msk`'s missed hours when it comes back (chapter 7).

### node_exporter on site 3

The operator of site 3 adds `[instance.node]` with `scrape` and its local
URL; the admin approves it, and a collector every 15 s keeps it as `node`.
The agent is away for ten minutes: ten minutes of `node` are a gap, drawn as
one.

### A hub taken over

Whoever owns the hub can call every approved instance with any `select`, up
to each agent's ceilings, and read whatever each database user may: for the
ten sites, whatever the ten read-only users were granted. They cannot read a
password, reach a host the operators did not write, run an action an operator
did not allow, or run JavaScript whose hash an operator did not pin. Each
operator can cut the hub off alone: an agent whose token is gone from its file
does not dial.

## Forks

After the review:

1. **The instance's id is its key in the operator's file**, and a rename is a
   new instance. Kept.
2. **Labels are the hub admin's**; the operator only suggests them. Kept.
3. **The hub sends a JS bundle and the agent checks its pin.** Kept.
4. **No buffering while the hub is away**: a scrape, a probe and a stream
   have gaps. Kept.
5. **TLS by system roots, a CA file or a pinned hash.** Kept; and a hub
   behind NAT is reached through Cloudflare Tunnel, which Dashbin documents
   rather than ships.
6. **The hub speaks two versions** — changed: of the call protocol and of the
   frame wire, both agreed in the greeting before any call.
7. **No flow control on the socket** — changed: small batches and credit per
   call and per agent, so the hub never receives more than it has room for and
   a slow reader slows only its own call.

Added: a hub taken over reads what the approved instances can read, said
plainly; rotating a token keeps the old one until the new one connects, and
revoking at once is its own act; a new plugin version is checked against the
queries and flows that use it; the WebSocket is kept, with the reason.

## What it asks of other chapters

- Chapter 1: the rule on choosing technology this chapter applied.
- Chapter 4: agents, their tokens, and an instance's placement.
- Chapter 5: a call on an agent carries what the hub asks for and its credit;
  an agent's calls share the budget for frames in flight.
- Chapter 6: a process plugin honours credit; a plugin's functions, params
  and columns are what `agent.hello` announces; a bundle has a hash an
  operator can pin.
- Chapter 9: the agents page, approving what an agent announces, a changed
  line drawn with both sides, a token not yet rotated, queries a new version
  broke.
- Chapter 10: who may add, rotate, revoke, disable and delete an agent;
  `X-Forwarded-Proto` behind a tunnel.
- Chapter 11: the agent endpoint and the api that manages agents.
- Chapter 12: the agent mode of the binary, its file, its service unit, and
  the Cloudflare Tunnel recipe.

## Not checked yet

- Ten agents end to end: connections dropped and restored, `$cancel` reaching
  PostgreSQL and MySQL behind an agent, and one connection overloaded while
  the others answer.
- The window, the batch size and each agent's share, under the hub's load run.
- The agent's memory with `postgres` and `mysql` under four calls each, with a
  JS plugin loaded, and over a socket slower than its answers.
- The size of the binary with a WebSocket client, rustls and the agent mode.
- An agent behind a corporate HTTP proxy, and a hub behind Cloudflare Tunnel.
