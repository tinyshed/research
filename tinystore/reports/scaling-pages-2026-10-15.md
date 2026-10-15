# Scaling, round 1: applying SQLite pages — 2026-10-15

Interim research for the implementation on `rust`. This note tests the page
application assumed by `plan/scaling.md` at TinyStore
`76b844cdc8c3410e746a5d5dc3e9aafec83c9962`. It changes no product promise.
Measurements used working-tree revisions on `experiment/cascading-replication`.
The harness and raw data are now recorded in research commit
`fa41d7ef2565315b9eb1018775633ef871b22468`; content hashes and archives
preserve the exact measured inputs. Measurements preceded that commit and
remain provisional pending independent reproduction.

| Question                                | Answer                                                                                               | What follows                                                                     |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Does a first copy match exactly?        | Yes, in 3 of 3 fresh runs: two 4,096-byte pages.                                                     | The first copy alone does not validate incremental replication.                  |
| Does an incremental copy match?         | No, in 3 of 3 runs. Two bytes differed: page 1 offsets 27 and 95 held 3 where the source held 2.     | The plan's byte-for-byte gate does not hold for this applier.                    |
| What do those bytes belong to?          | SQLite's two 32-bit change counters, at offsets 24–27 and 92–95.                                     | Define a canonical checksum excluding exactly those 8 bytes, or change applier.  |
| Can schema replacement be a loop?       | Repeated prepared INSERTs failed when adding SQL, KV and jobs tables together, growing to 31 pages.  | Install the pages and truncation in one statement, within one transaction.       |
| Can a copy run the normal engines?      | Opening their writers would permit expiry, jobs and migrations on a copy.                            | Imported files have only query-only readers and the page applier.                |

## Environment and reproduction

Managed Linux container, kernel 6.18.44, x86-64, AMD EPYC 9V74 reported by the
hypervisor; 3 CPUs visible, cgroup quota 200,000 µs per 100,000 µs, 8 GiB
memory limit. Rust 1.99.0, rusqlite 0.40.2, bundled SQLite 3.53.2. This probe
used temporary files in the container's `/tmp`; it measures correctness,
not storage latency. SQLite uses WAL and `synchronous=FULL`.

```sh
cd <repo>/tinystore/replication-spike
sh bootstrap.sh
cargo run --locked -- probe
cargo test --locked
```

The isolated source worktree is pinned by `source-revision`; research's
existing `tinystore/source` submodule stays at its previous commit. No old
Go research module is moved onto the Rust rewrite. SQLite is built with
`SQLITE_ENABLE_DBPAGE_VTAB` by the spike's own `.cargo/config.toml`.

Raw results: [probe.jsonl](data/scaling-pages-2026-10-15/probe.jsonl),
[environment and source hashes](data/scaling-pages-2026-10-15/environment.json),
and [run timestamps](data/scaling-pages-2026-10-15/resources.json).
The final probe was repeated against the source preserved in round 2's
[follow-up archive](data/scaling-cascade-2026-10-15/batch/harness.tar.gz).
The fixture is generated: `items(id INTEGER PRIMARY KEY, value TEXT)`, one
row `hello`, then a second row `world`. Each of the 3 passes creates a new
source and destination. The destination applier closes and reopens between
states. The second state changes one 4,096-byte data page.

## The checksum is a proposed semantic change

The experiment hashes every page in page order with SHA-256, substituting
zero for only the two change counters in page 1. It still covers the page
count, schema cookie, schema, freelist, indices and every value. All 3
incremental copies had equal canonical checksums and unequal raw images.
This is a canonical page image, not a byte-for-byte copy of the source.

Excluding these fields needs review before a product makes this its integrity
contract. This round covers ordinary SQLite tables and the tested TinyStore
schemas; it does not prove extension or cross-version compatibility.

## Replacing a schema is more than inserting its pages

`sqlite_dbpage` exposes new schema bytes while SQLite may still have the old
b-tree size and prepared schema cached. The prototype uses a dedicated
connection with `writable_schema=ON`, places page 1 after the other changed
pages, and issues one multi-row INSERT containing all pages followed by the
NULL row that truncates the file. A single bound blob holds page data, so
the number of pages does not consume one parameter each. Only trusted page
packets enter this connection. Application queries cannot reach it.

The connection then closes. Verification and queries open fresh readers.
Splitting the insertion into repeated statements reproduced `database disk
image is malformed` when a file grew to 31 pages after adding KV and jobs
tables to SQL; the single-statement version passed the same test. Growth,
VACUUM shrinkage, DDL and an 8,192-byte page size also have regression tests.

## What follows

Round 2 builds a per-file durable feed, checks restart and relay behaviour,
and measures capture overhead, page amplification, compression and fan-out.
The current capture scans a whole snapshot and diffs its pages. Its sequence
counts captures, not SQLite commits. It cannot claim to implement the
writer's WAL capture described in the scaling plan.

The implementation handoff must decide the integrity contract explicitly
and must replace snapshot scanning with capture of committed WAL frames
before using this in a large store.
