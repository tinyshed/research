# 10. Security and access

**Status: draft, second round, 27 October 2026.** The first round went
through two reviews, and the second showed that its holes were one hole; the
chapter is rebuilt on one invariant and one context of execution. What
changed is listed under *Forks*. Nothing is agreed until Dmitry says so.

Who may do what, from one person running ten sites to a company with teams,
without a permission matrix, without custom roles and without an identity
service inside a small binary. The first round patched eight holes one by
one: a flow edited into reading what its editor could not, a dashboard that
became public when its last grant went, a group that had to widen everyone's
data, a bot answering a whole group chat. They were all one mistake — an
object acting with one person's authority while others shaped or used it —
and they are closed here by one rule and one check, not by eight.

## The invariant

```
no one receives data or causes an effect beyond their own authority,
except through an explicit, bounded grant from someone who has that authority
```

Every rule below is a way of keeping it, and the model test at the end is the
gate that says whether it holds.

## The context of execution

Every execution — a panel's refresh, a flow's run, a click, a chat's command,
a call over an agent — carries one small context, and the query engine, the
flow engine and the plugin host read it; nothing else decides access.

```rust
struct Context {
    workspace: WorkspaceId,
    actor: Actor,               // who set this off: an account, through a session, a token or a chat link;
                                //   or a flow's trigger — its schedule, its webhook, its button
    credential: Limits,         // what the way in allows: read-only, a scope, an expiry; a session's is none
    authority: Principal,       // whose access the execution uses: an account or a service account
    via: Option<TriggerGrant>,  // when the actor is not the authority: the grant that lets it act for it
    request: RequestId,
}
```

**What an execution may do is an intersection, computed when it acts:**

```
effective = what the authority may now  ∩  what the credential allows  ∩  what the grant bounds
```

and, apart from it, **whether the actor may use the grant**: a click needs
`interact`, and a read-only token is not allowed to press a button that acts.

| Entrance | actor | authority | via |
|---|---|---|---|
| a panel in the browser | the viewer's session | the viewer | — |
| `dashbin show` | the viewer's personal token | the viewer, narrowed by the token | — |
| a chat's command | the linked account | that account | — |
| a scheduled flow | the flow's schedule | the flow's `runs_as` | the schedule, as the flow declares it |
| a webhook | the delivery, checked by the instance's function | the flow's `runs_as` | the webhook trigger |
| a button | the clicker | the flow's `runs_as` | the button's grant |

The audit records all three for every change, so a year later it is still
clear who pressed, under whose authority, and by which grant.

## Where it is checked

**At saving, and before every real operation.** A save is refused when it
would let an object do more than whoever saves it may (below). Then the
plugin host checks the context **before each `read` and each `action` it
starts** — not once when a long flow began, and again on every attempt of a
job's unit after a crash. Selectors are resolved again under the authority's
current scope, the grant's state is read again, the parameters are checked
again against the grant's bounds.

So access taken away at 12:01 stops the next call after 12:01. A call already
running is not undone by the check; it ends by chapter 5's cancellation, if
anything cancels it.

## Triggers are grants

A schedule, a webhook and a button are one thing: **a trigger a flow
declares, which lets something that is not its authority start it**, within
bounds the trigger states. Each is activated its own way and then runs under
`runs_as` with the trigger's bounds:

| Trigger | Activated by | Its bounds |
|---|---|---|
| a schedule | the flow's own time | no parameters from outside; the flow's own |
| a webhook | a request to the hook's address, then the instance's function verifying it (chapter 7) | only the rows that function returns; knowing the address starts this flow and nothing else |
| a button | a click by someone with `interact` on its dashboard, through a credential that may act | its one function of its one instance; parameters only from the values the dashboard's variables allow, checked on the hub whatever the browser sent; a rate and an expiry if it has them |

A grant is bound to what it grants: the function, the instance and the
bounds. Changing any of them is an edit of what the flow does, under the rule
below, and a plugin upgrade that changes a function a grant names is shown
with the flows and buttons that call it, which the admin confirms with the
upgrade (chapter 6).

## Who a flow runs as

- **`runs_as` is set when the flow is made, and nothing but an explicit
  transfer changes it.** The new principal must cover everything the flow
  does, and the transfer is audited. Editing does not move it, so a flow's
  authority never depends on who last pressed save.
- **An edit of what a flow does needs the editor to cover it.** Reads,
  selectors, actions, parameters' templates and triggers are what it does; a
  name and a description are not. Changing what it does is refused unless the
  editor may themselves read every read and call every action of the flow as
  it will be — and `runs_as` must cover it too. Kazan's editor can rename the
  admin's flow over all sites and cannot change what it reads; the refusal
  names what is not covered and who could make the change.
- **A shared query is part of every flow that uses it**: saving one that
  flows use needs the right to edit each of them.
- **A service account** is a principal that is not a person, held by admins,
  with a role and a scope of its own. A company's automations run as one, so a
  person leaving stops nothing. A flow whose `runs_as` is a person who loses
  access pauses, marked, rather than running as nobody (chapter 7).
- **No queue of approvals in the first version.** A refusal that says who
  could make the change is enough; asking that person through Dashbin is
  later, if anyone needs it.

## Absence never grants

- **Every scope is written out**, `all` included; an account and its groups
  with no scope between them read nothing.
- **A group only adds.** It may give a role and no scope at all — a group
  `managers` that makes its members viewers, each with a scope of their own —
  and it never widens anyone's data by having no scope.
- **An object has an explicit mode**: `workspace`, open to the workspace by
  role, or `restricted`, open to its grants. A restricted object with no
  grants is seen by its author, the admins and the owner, not by everyone.

## One place decides, and answers go to the actor

The query engine decides data at resolve, the policy decides objects, and the
plugin host checks each call; the browser, the api, the terminal, a chat, a
flow and an agent are thin ways into those, with no access logic of their
own. **An answer goes to the actor**, or where an action the authority may
call sends it: `/show revenue` asked in a group chat is answered privately,
not to everyone in the group.

Two functions hold the objects' side, as Go Dashbin's `access.Policy` began:

```
allowed(context, act, object)  → yes or no
visible(context, kind)         → a filter the repository applies in SQL
```

`visible` is the answer to Go Dashbin's open question of how a listing is
filtered once permissions exist.

## Trust, boundary by boundary

| Boundary | Not trusted | Held by |
|---|---|---|
| browser → hub | the browser | the session, CSRF, the one place that decides |
| a plugin's code in the browser | the plugin | a sandboxed frame with no same origin and no network (chapter 9) |
| a plugin's code on the hub | the plugin | the admin's trust at install, its declared hosts only; a process plugin by the owner's consent, which is not a sandbox |
| hub → agent | the hub | the operator's file, pinned hashes, `allow`, ceilings (chapter 8) |
| hub → someone's database | everyone | the database user's grants — the only boundary inside a database |

Two limits are said plainly. **A process plugin runs native code with the
operating system's rights**, outside this model; the owner's consent is what
admits it. **A scope of labels chooses instances and series; it does not
isolate rows inside one database.** When one PostgreSQL holds every site,
the database isolates them — a role per site with row-level security — and
Dashbin makes an instance of each role, which its labels then really bound.

## What carries over from Go Dashbin, and what is tightened

- **An installation starts empty and its first account is the owner**; only
  the first setup wins a race.
- **The owner is the database's promise**: a partial unique index makes a
  second one impossible; nobody but the owner unseats the owner, and
  ownership is transferred against the owner's own password.
- **A credential is a row**: sessions, personal tokens and agents' keys in one
  table, a token stored as SHA-256, so a revocation stops the next request.
- **A password is hashed with Argon2id, one hash at a time** for the process.
  Tightened: sign-in attempts are limited per account and per address with
  TinyStore's own rate limiter, the wait for the one slot is a short bounded
  queue, and a full queue answers 429 at once without hashing anything.
- **Changing a password ends every other session in the same write**; a
  password an admin hands out answers nothing but its own replacement.
- **CSRF is `SameSite=Lax` and a check of where a write came from.**
  Tightened: a write carrying the cookie with neither `Sec-Fetch-Site` nor
  `Origin` is refused; a bearer token skips the check.
- **`Secure` follows the scheme.** Tightened: `X-Forwarded-Proto` is believed
  only from the proxies the configuration names; HSTS only over TLS.
- **TLS is the binary's own**, its certificate reloaded when it changes.
- **The scope is a parameter of a handler, never read from the context.**
- **The way back is the machine's**: `dashbin passwd` and `dashbin owner`.
- **Every route is public on purpose**, a test over the registered operations.

## The ladder

Every step is designed now; the first version builds step 0, the audit and
service accounts' place in the tables, and chapter 14 says when the rest
comes.

| Step | What it adds |
|---|---|
| 0 | four roles, held per workspace in a membership row: one row per account for `default` from the first version |
| 1 | scopes of labels on accounts and groups, applied at resolve with the authority's scope |
| 2 | groups, which add a role, scopes, or both |
| 3 | sharing a restricted object with an account or a group: view, interact, edit |
| 4 | workspaces: a workspace column on every table from the first version, filled with `default` |

| Role | May |
|---|---|
| viewer | see the dashboards open to them, run them with the values their variables allow, `dashbin show`, a chat's commands |
| editor | and edit dashboards, queries and flows within the rules above, use the query editor on the instances in their scope, press buttons, make an alert from a panel |
| admin | and manage accounts other than the owner, service accounts, groups, scopes, grants, instances and their secrets, JavaScript plugins, agents, themes; read the audit |
| owner | and consent to process plugins; make a backup over the api and take its key; transfer ownership; make workspaces |

**With step 1, a scope narrows everything the authority reads**: a read's
selector, a variable's values, a dataset's series match. `all` is every site
the authority may read, a total over a scope says "within your access:
kazan", and groups outside the scope are absent by access, never shown as the
whole. **A grant on a restricted object may give more than the role on that
one object** — a viewer who edits the finance dashboard — and never what a
role alone has: accounts, instances, plugins, agents. Sharing never shares
data: a scope still narrows every read on a shared dashboard.

**Until step 1 is built, a manager cannot be held to one site by Dashbin.**
The first version says so; if the first user needs it, step 1 joins the first
version, which chapter 14 decides.

## Credentials

| Credential | What it adds to the context |
|---|---|
| a session | nothing: its account's access |
| a personal token | limits: read-only, a scope, an expiry; it never mints a token or changes anyone's access, and a read-only one never activates a trigger that acts |
| an agent's token | its agent's door, nothing else (chapter 8) |
| a linked chat user | its account's access through the chat's commands and buttons; answers to them alone |
| a Mini App's sign-in | a session for the linked account, after the kernel checks Telegram's signature with the bot's token and refuses one older than a few minutes |

A personal token is shown once, named, listed with its last use and revoked
like a session; `dashbin show` signs in with one. A chat is linked by a code
used once and valid for minutes, from the account's own page.

## Secrets

Sealed at rest by TinyStore (chapter 4), masked rather than hidden from the
plugin that uses them (chapter 6), never in a log — Go Dashbin's redacting
handler is the gate — and never sent back to a browser once saved: a secret
field is written, replaced or cleared, never read.

## The audit

**Every change of access, trust or secrets is an event in an `audit`
stream**, read by admins through `datasets`:

- signing in, failed or not, and from where; signing out;
- a password changed or handed out; a role, a scope, a group, a grant, a
  service account;
- a token made or revoked; a chat linked or unlinked;
- a plugin installed, upgraded or rolled back, with the grants it confirmed; a
  process plugin consented to;
- an agent added, rotated, revoked, disabled or deleted; an announced instance
  approved;
- an instance or its secret changed — that it changed, never the value;
- a flow's `runs_as` transferred; a flow paused for lost access;
- a button pressed, with the clicker, the authority and the grant;
- a backup made, with its key or without; a restore; ownership transferred; a
  workspace made.

Each carries the context's actor, authority and grant, the object, the values
before and after for anything that is not a secret, the `request_id` and an
`event_id` of its own — the request's id and the event's place in it, so a
request that changes three grants writes three events.

**An audit event is written in the transaction of the change it records**,
into a queue inside `dashbin.db`, and a job appends it to Records under its
`event_id` — the outbox of chapter 7's notifications. The id is the same on
every attempt, and an append of an id Records holds writes nothing, measured
on 0.2.0, so a retried job does not write an event twice.

Reads are not audited, or the audit would be a log of every refresh. A chain
of hashes that makes it tamper-evident is later.

## The model test

**The invariant is a gate, not prose.** A model-based test builds worlds —
accounts, service accounts, scopes, groups, grants, flows, shared queries,
buttons, tokens — and walks them through **sequences of changes**: make a
flow, change a role, add an instance, rename a label, edit a shared query,
retry a job, revoke a token, take back a grant, upgrade a plugin, transfer
`runs_as`. After every step it goes through every entrance — the api, the
terminal, a chat, a schedule, a webhook, a button, an agent — and checks that
no actor received a frame from an instance outside its effective authority
and no effect was caused beyond it, except through a grant still in force.

**Its reference is its own**: a small model of the rules written as plain
sets inside the test, never a call to `allowed()` or `visible()`, so a wrong
implementation cannot vouch for itself. A failing sequence is shrunk to the
fewest changes that break it, as TinyStore's own model tests do.

## Against six scenarios

### Alone

The first user is the owner of `default`, with `all`. Nothing on any screen
asks about groups, grants, service accounts or workspaces: complexity appears
on demand.

### Ten site managers

The owner makes a group `managers` that gives the role viewer and no scope,
and gives each manager a scope of their site. `Revenue` is open to the
workspace: each manager opens it, the switch lists their site, the total
reads "within your access: kazan", and `/show revenue` in a group chat
answers the manager privately. This needs step 1.

### A button that restarts a service

An admin makes a button flow running as the service account `ops`: restart,
on `kazan-api`, no parameters. Kazan's viewer with `interact` on the
dashboard presses it; the audit says who pressed, as whom and by which
button. A request forged to restart `msk-api` through the same button is
refused: its instance is the grant's, not the request's. A read-only token
cannot press it at all.

### An admin's flow, edited

Kazan's editor opens the admin's flow over all sites and renames it: saved,
`runs_as` unchanged. They add a read of Moscow's database and an action
sending it to Telegram: refused, naming the read they cannot cover and the
admins who could make the change.

### Access taken back mid-run

At 12:00 a flow starts reading five sites. At 12:01 its `runs_as` loses one
of them. At 12:02 the flow reaches its Telegram action: the plugin host
checks the context before the call and refuses it for what is no longer
covered; the run fails with the reason, and the flow pauses, marked.

### A token that leaked

A personal token made read-only, scoped to Kazan and expiring in thirty days
was pasted into a chat. An admin revokes it; the next request with it is
refused. The audit shows when it was made and by whom; its scope bounded what
it could read, and being read-only it could press nothing.

## Forks

After two reviews and a rethink:

1. **A ladder of tables, no custom roles and no permission matrix.** Kept.
2. **Data scope by labels, applied by the kernel** — kept, with its limit
   said: it does not isolate rows inside one database.
3. **An object open until it is shared** — changed: an explicit mode,
   `workspace` or `restricted`, and a restricted object with no grants is
   nobody's but its author's and the admins'.
4. **Roles per workspace in a membership row from the first version.** Kept.
5. **Every scope written out** — kept, and groups only add: a group may give
   a role and no scope.
6. **The audit through an outbox** — kept, with an `event_id` per event beside
   the request's id.
7. **Process plugins and a backup with its key are the owner's alone.** Kept.
8. **A personal token only narrows** — kept, now as the credential's limits in
   the context.

Added: one invariant; a context of actor, credential, authority and grant,
read by the query engine, the flow engine and the plugin host; checks at
saving and before every call; schedules, webhooks and buttons as one kind of
grant; `runs_as` fixed until an explicit transfer, and an edit of what a flow
does only by someone who covers it; service accounts; answers to the actor;
trusted proxies, refused writes with no origin, a limited queue for signing
in; a model test with its own reference, over sequences of changes.

## What it asks of other chapters

- Chapter 4: memberships, service accounts, scopes, groups, grants, objects'
  modes, tokens and chat links as tables; a workspace column on every table
  from the first version; the audit's queue inside `dashbin.db`.
- Chapter 5: the context instead of a bare scope; the authority's scope
  narrows selectors, variables and series matches at resolve.
- Chapter 6: the plugin host checks the context before each call; an
  upgrade lists the grants that call what it changes.
- Chapter 7: `runs_as`, triggers as grants, a check before each call and on
  every attempt, a pause for lost access, the audit beside notifications.
- Chapter 8: an agent's call carries no person; the hub has checked the
  context before it asks.
- Chapter 9: the sharing screen, an object's mode, "within your access", a
  refusal that names who could make a change, answers to the actor in a chat.
- Chapter 11: every operation public on purpose, `visible` behind every
  listing, the context built at the edge.
- Chapter 12: `passwd`, `owner`, `backup` and `restore`, and the trusted
  proxies in the configuration.
- Chapter 14: which steps the first version builds; this chapter proposes
  step 0, the audit and service accounts' tables.

## Not checked yet

- The model test itself: its reference, its sequences and how long a run
  takes to find a broken one.
- A scope applied to a dataset's series match over many series, and its cost.
- The check before each call: what it adds to a call, and that a revoked
  grant stops the next one.
- The audit's outbox under a sign-in storm, and how much it writes a day.
- Argon2's one slot behind the rate limiter and its queue, inside 64 MiB.
