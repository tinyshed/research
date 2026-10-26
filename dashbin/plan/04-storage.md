# 4. Storage on TinyStore

**Status: draft, first round, 27 October 2026.** Nothing is agreed until
Dmitry says so; the choices a review should look at are under *Forks*.

Everything Dashbin keeps of its own lives in one TinyStore directory, and
nothing else holds state: no second database, no cache server, no file beside
it that only the code knows about. Chapters 5 to 9 each said what they keep;
this chapter puts it in files, and decides which things share a file because
they must commit together.

What it rests on, read in TinyStore 0.3.0's source: an application database
in `sql/<name>.db` with migrations checked at every open; a KV bucket, a job
queue and a search index opened **inside** that database's file, which commit
with its rows in one transaction; encrypted buckets sealed with
XChaCha20-Poly1305 under a key in a file; files by path in named sets; a
backup of the whole store as one checked zip; a memory budget for every
engine's work.

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
| plugins installed: version, SHA-256, source, who and when | a table; the package's bytes in `blobs/` | the previous version kept for a rollback |
| dashboards, flows, shared queries | one document a row, a table of references and a table of history (chapter 9) | a save checks the version, writes its references and its history in one write |
| what can be searched | a search index following those tables | changed in the commit that changes a row |
| flows' schedules and runs | a queue | — |
| subjects' states and collectors' windows | buckets under each flow | an alert's new states and the notifications they cause in one write (chapter 7) |
| notifications | a queue | the same write |

**Datasets are apart, on purpose.** A series goes to `metrics.db` and an event
to `records.db`, and neither commits with a flow: chapter 7 makes their writes
harmless to repeat instead — an event's id, a sample sent again — so a crash
between a write and the step's answer writes nothing twice.

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
- **A store opened with another key says so** rather than calling the value
  corrupt: the sealed value carries the key's id.
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
each engine's file as one moment of that engine, the plugins' packages, and
a manifest with each file's size and SHA-256. `dashbin restore <file> <dir>`
checks every byte into an empty directory, before a hub opens there.

- **The key is left out unless asked for** (`--with-key`), and the backup says
  so: a backup holding its key opens every secret in it, and one without it
  restores everything but the secrets.
- **The bytecode cache is not backed up**; it is rebuilt from the packages.
- **A restored hub starts like any other**: migrations checked, documents
  read, agents dialling in again.

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

## Not in the first version

- **Replication.** TinyStore can make a store a node that others copy; a
  second hub reading a copy is later.
- **A vault for the key.** The key is a file the operator may move; a vault
  replaces a file.

## Against three scenarios

### A password changed

An admin's password is changed. The new hash and the revocation of every
other session are one write in `dashbin.db`; a stolen session dies with it,
and a crash between them is not possible because there is no between.

### An alert fires

The commit unit of a run (chapter 7) writes `spb`'s new state into its
flow's bucket and one notification job into the notifications queue, both
inside `dashbin.db`, in one transaction. Either both exist or neither does;
the notification is never lost and the state never moves without it.

### A new server

`dashbin backup --with-key` on the old one, the zip copied over,
`dashbin restore` into an empty directory, the hub started. Migrations pass,
the dashboards read, the secrets open with the key that came along, and the
ten agents, whose files name the hub's address, dial the new server once the
address points at it.

## Forks

1. **One application database holding its buckets, queues and search
   inside it** — against each engine's own file, `kv.db` and `jobs.db`, which
   cannot commit with a row.
2. **Secrets in TinyStore's encrypted bucket** — against Go Dashbin's own
   AES-GCM code beside the database.
3. **Plugin packages in TinyStore's files, bytecode in a cache that is not
   backed up** — against both in the store, which makes every backup carry
   what a start rebuilds.
4. **A document upgraded as it is read**, its schema numbered — against
   rewriting every document in a migration at each change.
5. **The key left out of a backup unless asked for** — against always in,
   which is simpler to restore and makes every copy of a backup a copy of
   every secret.
6. **Datasets never in `dashbin.db`**, Go Dashbin's rule kept.

## What it asks of other chapters

- Chapter 7: its notifications queue and states' buckets inside `dashbin.db`.
- Chapter 10: accounts, sessions and tokens as tables; who may read a backup
  and the key.
- Chapter 12: `backup` and `restore` as verbs of the binary, the memory split
  under the load run, self-measurement turned on, the disk's free space.
- Chapter 13: migrations carried in the binary, and a test for every document
  shape still read.

## Not checked yet

- A thousand flows in a queue inside `dashbin.db`, with their states'
  buckets beside them, under a commit unit every minute.
- An encrypted bucket inside a database's file, sealed and restored from a
  backup with and without its key.
- A backup of a store holding a month of `node` from ten sites: its size and
  how long it holds each engine.
- The memory split under the load run.
