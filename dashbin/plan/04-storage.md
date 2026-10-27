# 4. Storage on TinyStore

**Status: draft, second round, 27 October 2026.** The first round went
through one review, which approved the files and what shares them and held
back the backup; what it changed is listed under *Forks*. TinyStore 0.4.0,
rerun the same day, took away the three workarounds it carried. Nothing is
agreed until Dmitry says so.

Everything Dashbin keeps of its own lives in one TinyStore directory, and
nothing else holds state: no second database, no cache server, no file beside
it that only the code knows about. Chapters 5 to 9 each said what they keep;
this chapter puts it in files, decides which things share a file because they
must commit together, and says what a restore promises across the files that
do not.

What it rests on, read in TinyStore's source and run against 0.3.0 and
0.4.0: an application database in `sql/<name>.db` with migrations checked at
every open; a KV bucket, a job queue, a search index and a set of files opened
**inside** that database's file, which commit with its rows in one
transaction; encrypted buckets sealed with XChaCha20-Poly1305 under a key in
a file, with older keys kept for opening; a backup of the whole store at one
moment as one checked zip; a memory budget for every engine's work.

## The directory

```
data/
├── LOCK
├── sql/dashbin.db        the application: its tables, and inside the same file
│                           buckets  flow states, collectors' windows, sealed secrets
│                           queues   flows' runs, notifications, the audit's outbox
│                           search   names of dashboards, panels, queries and flows
│                           files    plugin packages, each version kept for a rollback
├── sql/dashbin.blobs/    those files' bytes past 16 KiB, named by rows of dashbin.db
├── metrics.db            series datasets, TinyStore's own measurements and Dashbin's
├── records.db            events datasets, alert transitions, runs' reports, the audit
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
| plugins installed: version, SHA-256, source, who and when | a table, and the package as a file of the same database (*Plugins*, below) | the row and its package in one write; the previous version kept for a rollback |
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

**A plugin's package is a file of `dashbin.db`'s own set**, written in the
transaction that writes its row: committed, both are there; rolled back or cut
by a crash, neither is, and bytes a crash left behind are removed when the
store next opens. Measured on 0.4.0: one install committed kept both, one
rolled back kept neither, and a process killed inside the transaction left
neither and no bytes after reopening
([recheck](../reports/tinystore-0.4.0-recheck-2026-10-27.md)). An old
version's package stays for a rollback until its row goes, in the same
transaction.

On 0.3.0 the package and its row were two files, and a crash between them left
one alone either way round; the order of writes, the sweep and the check at
start that worked around it went when
[tinyshed/tinystore#36](https://github.com/tinyshed/tinystore/issues/36) was
closed.

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
- **A key lost is never replaced quietly.** With the key that sealed a
  bucket missing, TinyStore refuses to open it rather than make a new key.
  Given a new key, it seals new values with it beside the old ones, reads an
  old one only when its key is among `older_encryption_keys`, and `reseal()`
  moves the old values to the new key, after which the old key can go —
  measured on 0.4.0. Rotating the key is the same three steps.
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

**TinyStore copies the whole store at one moment**, since 0.4.0: for the
instant it takes to begin a read of every file, every commit waits, and then
each file is copied from its read. That is what keeps the promise. Measured
with a collector writing a sample and then its window, over and over, twenty
backups a run:

| Run | 0.3.0, each engine its own moment | 0.4.0, one moment |
|---|---|---|
| the database opened first | the window behind its samples in 20 of 20, by 9 or 10 | equal in 17, one behind in 3 |
| Metrics opened first | **ahead** in 20 of 20, by 3 to 10: samples the window says are kept and the backup lacks | equal in 19, one behind in 1 |
| a state in the database against its events in Records | behind in 20 of 20 | equal in 19, one behind in 1 |

One behind is the moment falling between the sample and its window, which a
collector restored reads again harmlessly (chapter 7); ahead never happens.
The order Dashbin opens its engines in no longer carries a promise
([tinyshed/tinystore#35](https://github.com/tinyshed/tinystore/issues/35),
closed; [recheck](../reports/tinystore-0.4.0-recheck-2026-10-27.md)).

**The key, and a restore without it.**

- **The key is left out unless asked for** (`--with-key`): a backup holding
  its key opens every secret in it. A backup without it says so every time,
  with where the key is and what to keep apart.
- **A hub restored without its key starts and loses nothing.** The operator
  gives it a new key; every instance whose secret the missing key sealed says
  so, a secret entered again seals with the new key beside the old values, and
  the old key, found later, goes among the older keys and opens the rest; a
  reseal then moves them to the new key and the old one can go. On 0.3.0 the
  only way forward was to clear every secret
  ([tinyshed/tinystore#37](https://github.com/tinyshed/tinystore/issues/37),
  closed).
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

## What TinyStore was asked

| Issue | What it gave, in 0.4.0 | The workaround it took away |
|---|---|---|
| [#35](https://github.com/tinyshed/tinystore/issues/35) one moment for a backup | a restore as the whole store at one instant | opening the database first, and a test on the direction |
| [#36](https://github.com/tinyshed/tinystore/issues/36) files inside a database | a package and its row in one commit | the package first, the row after, a sweep, a check at start |
| [#37](https://github.com/tinyshed/tinystore/issues/37) old keys kept for reading | a restore without its key that loses nothing, and key rotation | clearing the bucket and asking for every secret again |

All three were filed with a program, closed in the release after, and rerun
before the workarounds went.

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

The same, with a backup made by default. The operator gives the hub a new
key; dashboards and flows are there, and each instance with a secret asks for
it; a collector reading a database waits until its password is entered again.
The old key, found a day later, goes among the older keys: every secret not
yet entered opens again, and a reseal moves them all to the new key.

## Forks

After the review:

1. **One application database holding its buckets, queues and search
   inside it.** Kept: what must commit together does, at the price of one
   writer, which is measured before it is promised.
2. **Secrets in TinyStore's encrypted bucket.** Kept.
3. **Plugin packages in TinyStore's files, bytecode in a cache that is not
   backed up** — kept, the package a file of the database since 0.4.0, written
   with its row.
4. **A document upgraded as it is read**, its schema numbered. Kept, with a
   test for every old shape still read.
5. **The key left out of a backup unless asked for** — kept, and a hub
   restored without it starts on a new key and keeps the old secrets until the
   old key is found.
6. **Datasets never in `dashbin.db`.** Kept.

Added: what a restore promises — nothing in it naming what it lacks — kept by
TinyStore 0.4.0's one moment; three issues asked of TinyStore and closed, each
taking away the workaround the first draft of this round carried.

## What it asks of other chapters

- Chapter 7: its notifications queue and states' buckets inside `dashbin.db`;
  a delivery at least once, whatever the transaction promised.
- Chapter 10: accounts, sessions and tokens as tables; who may make and read a
  backup, and who holds the key.
- Chapter 12: `backup` and `restore` as verbs of the binary, the memory split
  under the load run, self-measurement turned on, the disk's free space.
- Chapter 13: migrations carried in the binary, a test for every document
  shape still read, and a restore of a backup taken under writes.

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
