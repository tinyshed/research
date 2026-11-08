# 8. Agents

**Built in slice 7, 8 November 2026,** as Dashbin's `75d4a73`: the agent, its
link, the hub's door and approval through the api, and the command line
through a hub. **Status: agreed in its third round, 6 November 2026.** The
first round went through one review and the second through another; the third
is Dmitry's, after slices 4 to 6 built dashboards as files, the `Plugin`
trait, and MySQL. What each round changed is listed under *Forks*. What
slice 7 leaves for later says so where it stands. The spike before its code,
[an agent's socket](../reports/agent-2026-11-06.md), is in this text.

The first user runs ten sites on ten servers that cannot see each other, and
none of them may open a port. An agent is how their databases reach a hub
they can see: it sits beside a database, dials out to the hub's own port, and
answers for the instances its operator configured and nothing else.

```
a site's server, closed to the outside        the hub: dashbin.example.com, one open port
  agent ───── outbound wss:// ─────────────►  /api/v1/agent
        ◄──── on the same connection: "run sql on orders"
        ───── the answer's frame ──────────►
```

**The agent dials, the hub never does.** A closed server may still connect
out, so an agent passes any NAT and any firewall that lets a browser out;
only the hub has to be reachable, and it already is for its browsers.

**One host, one port, two doors.** The hub's address serves its api, its
browser and its agents: `https://dashbin.example.com` for people,
`wss://dashbin.example.com/api/v1/agent` for agents, behind the same proxy.
What crosses each door shares one envelope — JSON-RPC, a call's id,
`$cancel`, an error's kind — and the token's kind decides which door it
opens: a person's token is refused at the agent's door, and an agent's at
every other.

**One system of reads, not two.** A read on an agent is the same call of the
same plugin as a read on the hub: the hub routes it, the agent runs it
through the plugin built into the same binary, and nothing above a call
knows a database is a thousand kilometres away.

Go Dashbin built this and proved it on two servers; most of it carries over,
and this chapter says what changes now that an instance is a plugin's and the
agent is the same binary as the hub.

## What the hub knows and what the agent knows

| Who | Knows |
|---|---|
| the hub | an agent: its name, forever, given by the admin who added it; its tokens' hashes; and for each instance on it: the id the agent gives it, its plugin, the plugin's version and functions, what the operator allows it, whether the admin approved it |
| the agent | the hub's address, its own name and token, and for each instance: its plugin, its settings and secrets — a database's host, user and password, a Stripe key |

The hub never sends "connect to 10.0.0.8 with this password". It sends "run
`sql` on the instance you call `orders`", and the agent resolves that against
its own file.

**An instance on an agent is named `agent.id` on the hub**: the agent
`kazan`'s `orders` is `kazan.orders`, which a read reaches as
`on: "name=kazan.orders"` and a variable lists among the instances. A name
on the hub is lower-case letters, digits, `_` and `-`, and never a dot, so an
agent's instance cannot meet the hub's own, and ten agents may each call
their database `orders`.

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
hub = "wss://dashbin.example.com/api/v1/agent"
agent = "kazan"                        # the name the hub gave it; a token of another agent is refused
token = "dba_7Q…"                      # the hub gave it once, on adding the agent
# tls.ca = "/etc/dashbin/hub-ca.pem"   # or tls.pin = "sha256:…", for a self-signed hub

[limits]                               # the agent's ceilings; the hub may ask for less
rows = 10_000
bytes = "4MiB"
duration = "30s"
calls = 8

[instance.orders]                      # "orders" is the instance's id, forever; the hub's kazan.orders
plugin = "postgres"
title = "Orders, Kazan"
[instance.orders.settings]
host = "127.0.0.1"
database = "shop"
user = "dashbin_ro"
password = "…"                         # or DASHBIN_INSTANCE_ORDERS_SETTINGS_PASSWORD

[instance.legacy]
plugin = "mysql"
[instance.legacy.settings]
database = "legacy"
user = "dashbin_ro"
password_file = "/etc/dashbin/legacy.password"
```

- **It is read with TinyStore's config loader, without a store**, as
  `dashbin.toml` is: the file, then the environment over it, each instance's
  settings as the group its plugin declares, the same errors that name the
  file and the variable. Nothing is written beside it.
- **This file may hold secrets, where `dashbin.toml` may not.** `dashbin.toml`
  is committed with a project's code, so TinyStore refuses a secret in it
  (CLI-03); this one lives in `/etc` on one server and is nobody's code, so a
  password and the token may be written in it, or come from the environment
  and its `_FILE` as on the hub. **The agent refuses to start when the file is
  readable or writable by anyone but its owner**, on Unix, as Go Dashbin's
  did: a secret that never leaves the server is still a secret sitting in
  `/etc`.
- **A setting a plugin declares secret is masked** wherever the agent prints
  it, as on the hub (chapter 6). The plugin's own `settings()` says which.
- **A database is opened on its first call**, not at start, so one that is
  down is a panel that says so and not an agent that will not dial.
- **The instance's key is its id.** The hub binds what reaches it to
  `(agent, "orders")`. Renaming the key is a new instance that waits for
  approval, and the old one reads as removed; `title` is what changes freely.
  Go Dashbin generated an id and kept it in a state file beside the
  configuration; a key in the operator's own file is one fact fewer.
- **Labels are not in slice 7.** An operator's `labels = { site = "kazan" }`
  will be a suggestion the admin sees when approving; until labels are built,
  `name` is the one label every instance has (chapter 9's bridge), and a
  variable over the instances' names is how one dashboard shows ten sites.

## Placement and approval

**An instance on an agent is still an instance of its plugin.** What changes
is where it runs: chapter 2's placement is `hub` or `agent`, and nothing above
a call knows a database is a thousand kilometres away. The hub's row says
`{ "kind": "agent", "agent": "kazan", "id": "orders" }` or `{ "kind": "hub" }`,
as Go Dashbin's api already answered.

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

**In slice 7 the admin approves through the api**, as the first owner is made
(`POST /api/v1/agents/kazan/instances/orders/approve`); the agents page in the
browser comes with the browser.

**A new plugin version is checked against what uses it, not trusted again.**
The greeting carries each instance's functions with their args. When they no
longer fit a saved query or a flow that reaches the instance — a function
gone, an argument's type moved — those say so, by name, before a call fails
on them. A version whose functions still fit is shown and nothing else. In
slice 7 every plugin on an agent is built in, and its version is the binary's.

**Labels are the hub's.** When labels are built, the operator's are a
suggestion the admin sees when approving; what an instance is labelled — and
therefore which selector reaches it — is decided on the hub, so an agent
cannot announce itself into `site=@site`.

Whether an agent is connected is the live registry's, held in memory and in
no column, so no row says "unavailable" about a process that came back an
hour ago.

## Identity

**An agent is who its token proved; its name in the greeting is a check.** An
admin adds an agent on the hub by its name, and the hub answers its first
token, once. The token is of its own kind (`dba_`), a row of its own table,
since a person's token row is its account's, stored as SHA-256 and bound to
that agent. The operator writes the name beside the token, and the hub
compares the two at every connection:

| The greeting says | The token is | The hub |
|---|---|---|
| `kazan` | `kazan`'s | accepts it |
| `kazan` | `msk`'s | refuses it, naming both, and nothing of either agent is reached |
| `kazan` | unknown or revoked | refuses it as a token, naming no agent |

**The token decides, the name only refuses.** A name never chooses an agent:
it catches the token pasted onto the wrong server, which would otherwise make
Kazan's database answer for `msk.orders` without a word. Nothing on the agent
can take a name the hub gave another, since a name opens nothing without its
agent's token.

**A name is forever, on the hub as in the file.** An agent's name is part of
every instance's id on the hub, `kazan.orders`, which documents and variables
write down; renaming it would move every one of them. A different name is a
new agent, with its own token and its instances approved again; what an
agent is called for people is its `title`, which changes freely.

**A reconnect is the same agent.** Its identity and its approvals are its
name's, so an agent that comes back after a minute or a month finds its
instances as they were approved. Two connections with one token are one
agent twice: the hub keeps the newer, closes the older, whose calls fail as
unavailable, and says in its log that the agent connected from two
addresses. A connection that ends removes itself from the live registry only
while it is still the one registered, so a reconnect is not undone by the
connection it replaced noticing later.

**The hub cannot write the agent's file**, so a new token reaches the agent
only through its operator, and replacing one comes in two kinds:

| Act | What it does |
|---|---|
| add | an agent by its name, and its first token, shown once |
| rotate | a new token, shown once, valid beside the old; the first connection that proves the new one revokes the old in the same write; until then the hub says the agent is still on its old token |
| revoke now | for a token that leaked: every token the agent had is revoked and the connection hung up in one write; the agent stays away until its operator pastes a new one |
| disable | revokes and hangs up; its instances stay and read as unavailable |
| delete | the agent, its tokens and its instances; refused while a flow names one of them (chapter 7), and a document that names one is refused when it is checked |

The hub hangs up after the write rather than inside it, so a transaction
never holds the writer while a socket closes. Slice 7 builds add, revoke and
delete; rotate and disable come when a person can see the agents page.

## The connection

**One outbound connection, to the hub's own port.** The agent dials
`wss://…/api/v1/agent` with its token as a bearer, and the hub's listener
upgrades it to one WebSocket; a person's token does not open that door, and
the api a browser uses is not on it. Nothing listens on the agent's server.

- **TLS is the proxy's, in front of the hub.** Caddy, nginx or a Cloudflare
  Tunnel holds the certificate and passes the WebSocket through; the hub
  speaks plain HTTP behind it and listens on `127.0.0.1` unless its file says
  otherwise, so it is never open to the internet by accident. The hub that
  terminates TLS itself waits for someone who needs it.
- **The agent's TLS is rustls on ring, trusting the system's roots, or a CA
  file the operator names** (`tls.ca`): +1.6 MB in the binary with the
  WebSocket's both sides, where aws-lc-rs would be 2.1 MB more; a pinned
  certificate hash (`tls.pin`) waits for a hub whose certificate no CA file
  can name. Plain `ws://` is allowed for a hub on `127.0.0.1`, which is how a
  tunnel looks, and refused otherwise. With rustls in the binary for the
  agent, TLS for the SQL plugins' sources is a slice of its own, and a small
  one.
- **Liveness is bytes, not messages**: each side pings every 15 s, and a
  connection from which no byte has arrived for 45 s is closed. A WebSocket
  library hands over a message only when it is whole, so a side that counted
  messages would let a peer go while its answer still arrives: in the spike a
  hub counting messages dropped a 10 MB answer over 1 Mbit/s at 42 s, and one
  counting bytes had it whole at 85 s. The hub's door therefore does its own
  upgrade over hyper, with the socket under the WebSocket watched. 15 s is
  also under every idle close between the two: Cloudflare closes a quiet
  WebSocket after about 100 s, nginx's default after 60.
- **An agent that cannot reach the hub waits, it does not stop**: one second,
  doubling to thirty, with a fifth of that as jitter so ten agents that saw
  the same outage do not dial in one volley; a connection the hub welcomed
  starts the wait over, so that a reload of the proxy costs a second and not
  the thirty that three short connections in a row grew the wait to.
- **A proxy that reloads closes the sockets through it**: Caddy does, with the
  same configuration or a changed one, unless its `reverse_proxy` has
  `stream_close_delay`, with which the spike's ten agents stayed connected
  through every reload. The recipe sets it.

**Why a WebSocket, when newer transports exist.** What matters between an
agent and a hub is passing whatever stands between them:

| Transport | Through Caddy, nginx, Cloudflare Tunnel, a corporate proxy | Streams with their own flow control | Over a process's stdin and stdout |
|---|---|---|---|
| WebSocket | everywhere | ours, below | the same protocol |
| HTTP/2, gRPC | gRPC not on Cloudflare Tunnel's public hostnames; a proxy that buffers a request body hangs the agent, which is why Go Dashbin left it | yes | no |
| QUIC, HTTP/3 | needs UDP and a second open port; not passed to the hub by Caddy or the tunnel | yes | no |
| WebTransport | made for browsers; not something a proxy or a tunnel can be counted on to pass | yes | no |

What the newer ones give for real is a stream per call with its own flow
control, and the call protocol can carry that itself when frames stream
(*Later*), so it works the same over the socket and over a process plugin's
pipes.

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

**What crosses is a call, the same call a process plugin will get**
(chapter 6): the request, its frame or its error, and `$cancel`. Every
message is a JSON-RPC document in a text message.

| Direction | Message | Carries |
|---|---|---|
| agent → hub | `agent.hello` | its name, the call protocol's versions it speaks, the binary's version, and the instances: id, title, plugin and version, its functions with their args, what is allowed |
| hub → agent | `agent.welcome` | the call protocol's version the hub chose from those, and the instances it approved |
| hub → agent | `call` | the call's id, the instance's id, the function, its args and the typed parameters they name (chapter 5), and what the hub asks for — rows, bytes, duration |
| agent → hub | answer | the call's id and its frame, as the api writes one (`FrameJson`) |
| agent → hub | error | the call's id, its kind and its safe message |
| hub → agent | `$cancel` | a call's id; nothing waits for it to be confirmed |

- **The versions are agreed before any call.** The hub speaks its own call
  protocol and the one before. An agent with nothing in common connects, says
  what it speaks, and its instances read as "upgrade the agent", or "upgrade
  the hub" the other way; nothing connects and then sends what the other side
  cannot read. The binary's version is shown, never compared.
- **A frame is one message, in JSON.** Every frame is already whole in memory
  before anyone sends it — the row cap is 10 000 and a read returns its frame
  at once (chapter 5) — so a frame in pieces would only move the same bytes in
  more messages. The binary wire of chapter 3 and the batches with credit
  below come together when frames stream, for a local source and an agent's
  alike; the versions in the greeting let the wire change then without an
  agent that breaks.
- **An error crosses as the kind it left as** — invalid, forbidden,
  unavailable, timeout, cancelled — with its safe message; the cause stays on
  the agent and in its log. A refused connection does not name the host that
  refused it.
- **The session owns what is in flight.** An answer that comes late, twice or
  for a call nobody made is dropped; a call whose id is in flight is refused;
  a connection that ends fails every call waiting on it.
- **The envelope knows nothing of what it carries**, as in Go Dashbin:
  JSON-RPC's id, method and raw params, and the vocabulary beside it. Dispatch
  — who runs a call, what is in flight — is the hub's and the agent's, not
  the protocol's.

## Stopping a call

**Every stop ends the statement where it runs**, along the whole path:

```
dashbin show ── HTTP ──► hub ── wss ──► agent ── plugin ──► database
```

| What stops | What happens |
|---|---|
| the person presses Ctrl-C, or their client goes away | the hub's request ends and drops its read; dropping it sends `$cancel`; the agent drops the call, and the plugin cancels the statement on the server (SRC-02) |
| the call passes its time | the plugin's own bound ends it on the server (SRC-03), and the agent's own timer, a moment later, drops the call for a plugin whose bound did not |
| the hub loses the agent | every call waiting on that connection fails as unavailable at once |
| the agent loses the hub | the agent drops every call it was running, and each plugin stops its statement; nothing waits for a hub that may never come back |
| the agent comes back | nothing that was in flight is sent again: whether a statement ran is not known, and the caller asks again if it still wants the answer |

## Limits

**The ceiling is the agent's; the hub may only ask for less.** For each call:

```
effective = min(what the hub asked, what the instance allows, what the agent allows)
```

| Ceiling | Why |
|---|---|
| rows | a chart needs a count, and the cap is the plugin's (FRAME-04) |
| bytes | one row holding one long string is as large as it likes; the agent counts the bytes of the answer it encoded and refuses its own answer past the ceiling, saying both; 4 MiB by default |
| duration | the statement's bound a SQL plugin sets, and the call's end for any other |
| calls | at once, across the agent; the hub's four per instance (chapter 5) sits under it |

**The socket is bounded at each end, by numbers of ours.** A WebSocket
library's defaults are made for anyone: `tungstenite` reads a message of up
to 64 MiB, and queues what it is asked to send without an end. So:

- **The largest message either side reads** is the bytes ceiling and a little
  envelope; a larger one closes the connection, since only a broken or hostile
  peer sends it.
- **What is waiting to be sent is bounded**: the agent's answers by its calls
  at once, the hub's calls by the agent's ceiling, and nothing else queues.
- **Reading the socket never waits on whoever asked.** One task reads the
  socket and hands each answer to the call it is for, which holds it until its
  caller takes it; a slow browser or a stalled command line holds its own
  answer, never the socket, so pings and every other call go on.
- **An agent's answers wait behind its biggest one**, since a frame crosses
  as one message: in the spike, `site5`'s small reads timed out while its
  10 MB answer crossed 1 Mbit/s, and the other nine agents' did not slow.
  The bytes ceiling is how long that can be: 4 MiB is about 34 s at 1 Mbit/s
  and 3 s at 10, and still holds 10 000 rows of a dozen short columns. Frames
  that stream end the wait.

**What the hub holds for an agent is bounded by calls times bytes**: eight
calls of 4 MiB at the worst. The hub's own reads hold their frames the same
way; chapter 5's budget for frames in flight is the hub's, for every source,
and is set by the load run.

## Plugins on an agent

| Plugin | How it gets there | What the operator writes |
|---|---|---|
| a built-in — `postgres`, `mysql`, later `scrape`, `probe`, `notify` | inside the binary; its version is the binary's | `plugin = "postgres"` |
| a JS plugin, later | the hub offers the bundle's bytes; the agent runs it only if its hash is the one pinned | `plugin = "stripe"`, `bundle = "sha256:…"` |
| a process plugin, later | the operator installs it on the server | `command = ["bun", "/opt/slack/index.js"]` |

- **Slice 7 runs built-ins alone**: `postgres` and `mysql`, read-only.
- **A bundle that does not match its pin does not run**, and the instance says
  so. A new version on the hub reaches the agent only when its operator pins
  the new hash; the `bundle` message joins the protocol with JS plugins.
- **Kinds are reads unless `allow` says otherwise.** An `action` or a `stream`
  runs on an agent only when its operator wrote it.
- **The operator's line is the second consent** chapter 6 asks for a process
  plugin; it runs as the agent's user and reaches what that user reaches. It
  speaks the same call protocol.
- **A JS plugin on an agent costs what it costs on the hub**: a QuickJS
  runtime, unloaded when idle with its bytecode on disk. An agent that runs
  one needs a directory for that (`state = "/var/lib/dashbin"`); an agent that
  runs only built-ins writes nothing.

## The command line through a hub

**`show`, `query` and `mcp` read a hub's instances when `dashbin.toml` names
one**, so a person sees an agent's data the way they see their own:

```toml
hub = "https://dashbin.example.com"   # its token from DASHBIN_HUB_TOKEN, never the file
```

A read then goes to the hub's `/api/v1/run`, and the instances a document
names are the hub's: `kazan.orders` as much as the hub's own. The token is a
person's, and reads what that person's role may (chapter 10). Without `hub`
the commands read `dashbin.toml`'s instances as they do now.

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
| the hub restarts | every agent waits and dials again, spread by their jitter; nothing is lost but what was in flight |
| the proxy reloads and closes the sockets | the same as a restart of the hub, every agent back within a second or two; with `stream_close_delay`, nothing |
| the link to the hub is cut without a word | the hub lets the agent go after 45 s without a byte, and its calls fail as unavailable; a statement on the database ends by its own bound, 30 s, before either side notices |
| a database behind the agent is down | that instance's calls fail with the database's safe message; the agent's other instances answer |
| a person stops reading a big answer | their call holds its answer until it ends; the socket and the agent's other calls go on |
| a token is pasted onto the wrong server | the hub refuses it, naming the agent the file says and the one the token is |
| the agent's file names an instance the hub never approved | discovered, and nothing calls it |
| an operator widens `allow` | changed: the instance keeps what was approved until the admin looks |
| a token was rotated and the operator has not pasted it yet | the old one still works, and the hub says so |

## Memory

The agent is the hub's binary, with jemalloc's decay set to zero at start in
agent mode: about 6 MiB idle, measured against 5 for a musl build
([memory and allocators](../reports/memory-and-allocators-2026-10-24.md)).
What it holds for a call is the call's frame, and the agent's calls at once
bound that. The spike's agent, on musl's allocator, was 4.6 to 4.8 MiB idle
and 11.7 MiB at its peak with four reads of 10 000 rows at once beside
PostgreSQL, 12.4 beside MySQL; Dashbin's agent mode is not measured yet.

## Against three scenarios

### Ten sites, closed from each other

Each server runs `dashbin agent` with one `[instance.orders]` — `postgres` on
five, `mysql` on five — and a read-only user that sees the tables dashboards
need. The admin adds ten agents on the hub by their sites' names, pastes each
token and name into its server's file, and approves ten announced instances,
`kazan.orders` among them. A dashboard's variable `site` lists the hub's ten
instances, and its panels read `on: "name=@site"`; `dashbin show revenue --var
site=msk.orders` reads Moscow's through its agent. `msk`'s agent is away: the
panel says so and names how long. One panel over all ten at once waits for
`all` and labels.

### node_exporter on site 3, later

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

After the first review:

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

Added then: a hub taken over reads what the approved instances can read, said
plainly; rotating a token keeps the old one until the new one connects, and
revoking at once is its own act; a new plugin version is checked against the
queries and flows that use it; the WebSocket is kept, with the reason.

The third round, 6 November 2026, agreed with Dmitry:

1. **Slice 7 ends at what a person sees**: `show`, `query` and `mcp` read a
   hub's instances when `dashbin.toml` names the hub, so the chain
   command line → hub → agent → database is whole, and approval is the api's
   until the browser.
2. **An agent's instance is `agent.id` on the hub**, `kazan.orders`, unique
   by construction: a name on the hub holds no dot.
3. **The operator's file may hold secrets**, `dashbin.toml` may not, and the
   agent refuses a file others can read.
4. **TLS is the proxy's on the hub's side and rustls's on the agent's**;
   the hub listens on `127.0.0.1` by default; `ws://` only to `127.0.0.1`.
5. **Fork 7 waits, and fork 6 shrinks**: a frame is one JSON message, bounded
   by the rows and bytes ceilings, and the binary wire with credit comes when
   frames stream, for every source at once. Only the call protocol's version
   is agreed in the greeting.
6. **A spike before the code**: the weight of a WebSocket and of TLS in the
   binary, and ten agents end to end.

With them, from Dmitry and a review of the round:

- **Identity is the token's, and the name a check**: the hub refuses a token
  whose agent is not the one the file names; a name is forever; two
  connections with one token keep the newer.
- **Every stop reaches the database**, along the whole path, and nothing is
  sent again after a reconnect.
- **The socket is bounded by numbers of ours**: the largest message, what
  waits to be sent, and a read loop no caller can hold up.
- **One host and one port, two doors**: the agent's at `/api/v1/agent`, opened
  by an agent's token alone; the envelope shared with whatever a person's
  socket becomes.

After the spike, agreed with Dmitry:

- **rustls on ring with the system's roots**, and `tls.pin` later.
- **Liveness is bytes**, and the hub's door upgrades the request itself.
- **The wait starts over once the hub welcomed the agent.**
- **The recipe for Caddy sets `stream_close_delay`.**
- **The bytes ceiling is 4 MiB by default.**

## Later

- Labels, `all`, and one panel over every agent's instance.
- The agents page: approving, a changed line drawn with both sides, a token
  not yet rotated, queries a new version broke; rotate and disable with it.
- Frames that stream: chapter 3's binary wire, small batches, credit per call
  and per agent, for every source alike.
- JS and process plugins on an agent, and the `bundle` message.
- TLS for the SQL plugins' own sources, a hub that terminates TLS itself, and
  `tls.pin`.

## What it asks of other chapters

- Chapter 1: the rule on choosing technology this chapter applied.
- Chapter 4: agents, their tokens, and an instance's placement.
- Chapter 5: a call on an agent carries what the hub asks for; the budget for
  frames in flight is the hub's, for every source.
- Chapter 6: a plugin's functions and args are what `agent.hello` announces;
  later, a process plugin honours credit and a bundle has a hash an operator
  can pin.
- Chapter 9: the agents page, with the browser.
- Chapter 10: who may add, revoke and delete an agent; `X-Forwarded-Proto`
  behind a proxy.
- Chapter 11: the agent's door and the api that manages agents; the command
  line through a hub.
- Chapter 12: the agent mode of the binary, its file, its service unit, the
  Caddy and Cloudflare Tunnel recipes.

## Not checked yet

The spike before slice 7 checked the weight, ten agents end to end through
Caddy's TLS, a cancel, a lost hub, a slow link, restarts and the prototype
agent's memory ([an agent's socket](../reports/agent-2026-11-06.md)). Not
checked:

- Dashbin's agent mode with jemalloc, and hundreds of agents, whom a jitter of
  200 ms at a one-second wait does not spread.
- A real network rather than Docker's loopback.
- An agent behind a corporate HTTP proxy, and a hub behind Cloudflare Tunnel.
- The window, the batch size and each agent's share, once frames stream.
