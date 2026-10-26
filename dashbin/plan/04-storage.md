# 4. Storage on TinyStore

**Status: draft, second round, 27 October 2026.** The first round went
through one review, which approved the files and what shares them and held
back the backup; what it changed is listed under *Forks*. Nothing is agreed
until Dmitry says so.

Everything Dashbin keeps of its own lives in one TinyStore directory, and
nothing else holds state: no second database, no cache server, no file beside
it that only the code knows about. Chapters 5 to 9 each said what they keep;
this chapter puts it in files, decides which things share a file because they
must commit together, and says what a restore promises across the files that
do not.

What it rests on, read in TinyStore 0.3.0's source and run against it: an
application database in `sql/<name>.db` with migrations checked at every
open; a KV bucket, a job queue and a search index opened **inside** that
database's file, which commit with its rows in one transaction; encrypted
buckets sealed with XChaCha20-Poly1305 under a key in a file; files by path in
named sets; a backup of the whole store as one checked zip; a memory budget
for every engine's work.

## The directory

```
data/
├── LOCK
├── sql/dashbin.db        the application: its tables, and inside the same file
│                           buckets  flow states, collectors' windows, sealed secrets
│                           queues   flows' runs, notifications
│                           search   names of dashboards, panels, queries and flows
├── metrics.db            series datasets, TinyStore's own measurements and Dashbin's
├── records.db            events datasets, alert transitions, runs' reports
├── blobs/                plugin packages, each version kept for a rollback
├── encryption.key        what seals the secrets, unless the operator names another file
└── cache/bytecode/       QuickJS bytecode per plugin version, rebuilt when missing
```

**The application database stays metadata-only**, as Go Dashbin's rule had
it: `dashbin.db` is neither a copy of anyone's data nor a time-series store.
What collectors keep goes to `metrics.db` and `records.db`, whose retention,
write load and damage cannot touch the file the hub needs to start.

## What commits together lives in one file

TinyStore does not make a write across two files atomic, so anything that
must hold together is in `dashbin.db`, as a table or as an engine opened
inside it:

| What | Where in `dashbin.db` | Why it must commit with something |
|---|---|---|
| accounts, roles, sessions and tokens — a person's and an agent's | tables | a password changed and every other session revoked in one write; a key rotated and the old one revoked in one write (chapter 8) |
| instances: plugin, settings, labels, placement | a table | — |
| an instance's secrets | an encrypted bucket, keyed by the instance | the row and its secret in one write, so a host is never saved with the old password |
| agents | a table, their tokens among the tokens | delete refused while a flow names one of its instances |
| plugins installed: version, SHA-256, source, who and when | a table; the package's bytes in `blobs/` (*Plugins*, below) | the previous version kept for a rollback |
| dashboards, flows, shared queries | one document a row, a table of references and a table of history (chapter 9) | a save checks the version, writes its references and its history in one write |
| what can be searched | a search index following those tables | changed in the commit that changes a row |
| flows' schedules and runs | a queue | — |
| subjects' states and collectors' windows | buckets under each flow | an alert's new states and the notifications they cause in one write (chapter 7) |
| notifications | a queue | the same write |

**One transaction makes a notification's job exist; it does not make the
message arrive once.** Delivering it outside is the job's, at least once, with
chapter 7's operation id and its rules for an outcome nobody knows.

**Datasets are apart, on purpose.** A series goes to `metrics.db` and an event
to `records.db`, and neither commits with a flow: chapter 7 makes their writes
harmless to repeat instead — an event's id, a sample sent again — so a crash
between a write and the step's answer writes nothing twice.

**All of this is one writer.** Saving a dashboard, signing in and a thousand
flows' commit units share `dashbin.db`'s single writer, which is the price of
their one transaction; what it costs a sign-in while flows run is measured
before it is promised.

## Plugins

A package's bytes are in `blobs/` and its row in `dashbin.db`, two files, so
installing keeps an order that a crash cannot break:

1. **the package first**, written and checked against its SHA-256;
2. **its row after**, which is what makes it installed;
3. **a package is removed only by a sweep**, after its row has been gone for a
   while, never in the same moment as its row;
4. **at start, and after a restore, every row is checked against its
   package**: a row whose package is missing marks its plugin to be installed
   again, and the hub starts anyway.

Measured on 0.3.0: a process killed between the two writes leaves a package no
row names in one order, and a row naming no package in the other. The order
above leaves only the first, which the sweep clears. A set of files opened
inside the database, committing with its row, is asked of TinyStore
([tinyshed/tinystore#36](https://github.com/tinyshed/tinystore/issues/36));
these four steps go when it lands.

## Secrets

**A secret is sealed by TinyStore, not by Dashbin.** An instance's secrets
sit in an encrypted bucket inside `dashbin.db`: each value sealed with
XChaCha20-Poly1305 under a nonce of its own and bound to its place, so a
sealed value copied to another key does not open. Go Dashbin's own AES-GCM
code is not carried over.

- **The key is 32 bytes in a file**: `encryption.key` beside the store, which
  TinyStore makes the first time, or a file the operator names and TinyStore
  only reads. Moving the key to a vault later rewrites a file, not every
  secret.
- **A key lost is never replaced quietly.** A bucket keeps the id of the key
  that sealed it; with that key missing TinyStore refuses to open the bucket
  rather than make a new key, and with another key it reads nothing and writes
  nothing until the bucket is cleared — measured on 0.3.0.
- **Masked, not hidden from the plugin** (chapter 6): sealing is at rest; a
  call reveals what its instance holds.

## Migrations

- **The schema is SQL files carried in the binary**, applied in order and
  checked at every open; a released file never changes, as Go Dashbin's
  checksums enforced. Migrations run before the hub serves anything.
- **A document carries its schema's number.** A dashboard or a flow written
  by an older version is read by the newer one and upgraded in code as it is
  read; its next save writes the new shape. Each old shape a version still
  reads has a test with a document of that shape. A release stops reading an
  old shape only after a migration has rewritten every document still in it.
- **Tables are `STRICT`**, as in Go Dashbin, so a value of the wrong type is
  an error and not a column half of whose rows are something else.

## Backup and restore

**`dashbin backup <file>` writes one zip of the whole store while it runs**:
each engine's file, the plugins' packages, and a manifest with each file's
size and SHA-256. `dashbin restore <file> <dir>` checks every byte into an
empty directory, before a hub opens there.

**What a restore promises**: everything Dashbin keeps, as it was when the
backup was made, with nothing in it naming what it does not hold — a window
that claims samples, a state that claims an event, a row that claims a
package. A scrape and a probe after the backup began are a gap, as after any
backup.

**TinyStore copies each engine at its own moment**, in the order the program
opened them, so that promise is Dashbin's to keep until TinyStore gives one
moment for the whole store
([tinyshed/tinystore#35](https://github.com/tinyshed/tinystore/issues/35)).
Measured on 0.3.0, twenty backups taken while a collector wrote a sample and
then its window, over and over:

| Opened first | The restored window against the restored samples |
|---|---|
| the database | behind in 20 of 20, by 9 or 10 samples |
| metrics | **ahead** in 20 of 20, by 3 to 10: samples the window says are kept and the backup lacks, which the collector would never read again |
| the database, against events in records | behind in 20 of 20 |

So until #35:

- **The hub opens `dashbin.db` before every other engine.** What names
  something is then copied first and is behind what it names; a collector
  restored reads again from its window, and what it reads again is harmless
  to write twice (chapter 7). A plugin row is behind its package by the same
  rule, and the sweep never removes a package a row could still name.
- **A test restores a backup taken under writes and fails if the direction
  ever turns**, because the order is TinyStore's behaviour and not its
  promise.

**The key, and a restore without it.**

- **The key is left out unless asked for** (`--with-key`): a backup holding
  its key opens every secret in it. A backup without it says so every time,
  with where the key is and what to keep apart.
- **A hub restored without its key starts.** The secrets' bucket does not
  open, every instance that needs a secret says so, and the admin is asked for
  them again. Entering them clears the bucket first — the only way TinyStore
  0.3.0 lets a new key seal into it — and the hub says plainly that a key found
  afterwards will not bring the old values back. Keeping old keys for reading
  beside the one that seals is asked of TinyStore
  ([tinyshed/tinystore#37](https://github.com/tinyshed/tinystore/issues/37)).
- **The bytecode cache is not backed up**; it is rebuilt from the packages.

## Memory

**One budget, split four ways, under the 64 MiB promise** (chapter 1):

| Part | What bounds it |
|---|---|
| TinyStore's engines | `Options::memory`, the bytes every engine's work may hold at once |
| frames in flight | chapter 5's budget, with each agent's share (chapter 8) |
| JavaScript | chapter 6's slots for loaded runtimes and each runtime's limit |
| the rest | the binary, the runtime, connections and sockets, measured rather than bounded |

The numbers come from the load run: the hub measured 9.4 MiB idle and about
22 after heavy load before any of these existed, so the split is set once the
whole hub runs, not guessed here.

**The disk is bounded too.** TinyStore refuses a write that would leave less
than its free space — 1 GiB unless said — as a `Limit` error that leaves
nothing behind; the hub shows it as a disk nearly full, by name, rather than
as a failed save.

## Measuring itself

TinyStore measures its own commits, waits, files and memory into
`metrics.db` under `tinystore_` when asked; Dashbin turns that on and adds
its own under `dashbin_` — calls, frames in flight, loaded runtimes, agents
connected. They are a dataset like any other, so the hub's own dashboard is
drawn by the hub (chapter 12).

## Retention

- **A dataset's retention is the dataset's**, kept by TinyStore: a name's
  keep in Metrics, a stream's in Records.
- **A document's history keeps its versions for an age the admin sets**, and
  always the last few, so a dashboard edited every day does not grow without
  end and one edited once a year keeps its past.
- **Runs' reports and alert transitions are events with a keep**, read through
  `datasets` like any other.

## What TinyStore is asked

| Issue | What it would give | What Dashbin does until then |
|---|---|---|
| [#35](https://github.com/tinyshed/tinystore/issues/35) one moment for a backup | a restore as the whole store at one instant | opens its database first, and a test watches the direction |
| [#36](https://github.com/tinyshed/tinystore/issues/36) files inside a database | a package and its row in one commit | the package first, the row after, a sweep, a check at start |
| [#37](https://github.com/tinyshed/tinystore/issues/37) old keys kept for reading | a restore without its key that loses nothing, and key rotation | clears the bucket and asks for the secrets again |

Each workaround carries its issue's link in the code and goes when the issue
is closed.

## Not in the first version

- **Replication.** TinyStore can make a store a node that others copy; a
  second hub reading a copy is later.
- **A vault for the key.** The key is a file the operator may move; a vault
  replaces a file.

## Against four scenarios

### A password changed

An admin's password is changed. The new hash and the revocation of every
other session are one write in `dashbin.db`; a stolen session dies with it,
and a crash between them is not possible because there is no between.

### An alert fires

The commit unit of a run (chapter 7) writes `spb`'s new state into its
flow's bucket and one notification job into the notifications queue, both
inside `dashbin.db`, in one transaction. Either both exist or neither does;
the job then delivers at least once.

### A new server, with the key

`dashbin backup --with-key` on the old one, the zip copied over,
`dashbin restore` into an empty directory, the hub started. Migrations pass,
the dashboards read, the secrets open, every collector reads again from its
window what may be missing, and the ten agents, whose files name the hub's
address, dial the new server once the address points at it.

### A new server, without the key

The same, with a backup made by default. The hub starts, dashboards and flows
are there, and each instance with a secret asks for it; a collector reading a
database waits until its password is entered again. The admin who later finds
the old key is told it is too late for the values already entered, and that
it opens nothing in this store until #37.

## Forks

After the review:

1. **One application database holding its buckets, queues and search
   inside it.** Kept: what must commit together does, at the price of one
   writer, which is measured before it is promised.
2. **Secrets in TinyStore's encrypted bucket.** Kept.
3. **Plugin packages in TinyStore's files, bytecode in a cache that is not
   backed up** — kept, with the order of an install, a sweep, and a check at
   start and after a restore.
4. **A document upgraded as it is read**, its schema numbered. Kept, with a
   test for every old shape still read.
5. **The key left out of a backup unless asked for** — kept, and a hub
   restored without it starts, marks what needs a secret, and asks again.
6. **Datasets never in `dashbin.db`.** Kept.

Added: what a restore promises — nothing in it naming what it lacks — and how
it is kept while TinyStore copies each engine at its own moment: the database
opened first, measured both ways, and a test on the direction; three issues
asked of TinyStore, each with the workaround it replaces.

## What it asks of other chapters

- Chapter 7: its notifications queue and states' buckets inside `dashbin.db`;
  a delivery at least once, whatever the transaction promised.
- Chapter 10: accounts, sessions and tokens as tables; who may make and read a
  backup, and who holds the key.
- Chapter 12: `backup` and `restore` as verbs of the binary, the order the hub
  opens its engines in, the memory split under the load run,
  self-measurement turned on, the disk's free space.
- Chapter 13: migrations carried in the binary, a test for every document
  shape still read, and the test on a backup's direction.

## Not checked yet

- A process killed at random points of a commit unit, a thousand times: the
  states and the notification jobs both there or both not.
- A thousand flows in a queue inside `dashbin.db`, with their states' buckets
  beside them, under a commit unit every minute — and what that does to saving
  a dashboard and signing in.
- An encrypted bucket inside a database's file, rather than in `kv.db` where
  the key was measured.
- A backup of a store holding a month of `node` from ten sites: its size and
  how long it takes.
- The memory split under the load run.
