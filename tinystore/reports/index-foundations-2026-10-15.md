# Indexes from first principles — 15 October 2026

Research prototypes only. Product source and public API are unchanged. This
round extends [the first index round](indexes-2026-10-15.md); those measurements
remain historical evidence, not architectural premises. The runnable harness is
[`index-foundations`](../index-foundations/README.md). Raw output is under
[`data/index-foundations-2026-10-15`](data/index-foundations-2026-10-15/).

## Publication status and measured findings

This is an incremental publication: the static 96-process matrix is complete,
with two separate passes at 100k/1m rows. Concurrent rebuild and transport
matrices are still running; this revision does not claim their final results.
[Measurement tables](index-foundations-measurements-2026-10-15.md) retain each pass.

At 1m repeated Russian rows, compact external-content FTS5 (`detail=none`,
`columnsize=0`) builds in 11.858/14.835 s, occupies 29.99 MiB and has query p99
6.166/6.151 ms. Tantivy Basic builds in 9.885/7.608 s, occupies 40.63/39.61 MiB
and has p99 7.096/10.055 ms. Both materialize the same single-token result sets;
neither measures ranked/phrase search. Tantivy has not established a general
win even before the cost of coordinating its independent commit is included.

At 1m uniform rows, ordinary B-tree build is 0.77/0.81 s and query p99
4.25/4.39 ms; relational postings are 1.74/1.78 s and 19.93/21.39 ms. Roaring
cache p99 is 1.02/0.92 ms, with about 21.5 MiB process HWM, but lacks the
durable source transaction protocol. These are different operational contracts.

The actual unchanged Core SQL API passes FTS5/RTree trigger maintenance,
rollback and co-located SQL/KV/Jobs transactions with `trusted_schema=0` in
both probes. A contentless-delete smoke probe also succeeds; its scaled cost
is unmeasured. No suspected trusted-schema incompatibility was reproduced.

## What needs an index

| Owner | Existing structures in `rust` | Query and update shape | Consistency / memory constraints | Recommendation to evaluate |
|---|---|---|---|---|
| SQL application tables | SQLite PK, B-tree, partial and expression indexes; arbitrary SQL | Equality/range/order, joins; application-controlled rows and transactions | A query must see index and rows in one SQLite snapshot | Ordinary SQLite indexes; optional explicit FTS/spatial recipes or narrowly scoped managed projections |
| KV | WITHOUT ROWID `(bucket,path)`, expiry index, revisions, cleared branches, spills | Exact key / prefix branch, TTL and idle renewal; typed or encrypted values | Source values can be opaque; expiry and branch clearing must remove search visibility | Do not index arbitrary serialized values implicitly. First specify opt-in projection, owner, codec, expiry and encryption policy |
| Records | Compressed heads, segments and blocks; time boundaries; trace/value Bloom filters | Append, time window, trace selection, retention; packed events rather than SQL document rows | Per-event identity must survive compaction; deleting a segment must retire its index | Keep existing time/trace pruning. A text side index needs a demonstrated query requirement and retention protocol |
| Metrics | Name/series identity; label values and postings; ready/due state; compressed time groups | Label intersections and time ranges; many samples per series | Index labels/series, not every compressed sample; budget active-series memory | Existing postings already solve the relevant secondary lookup problem |

The tested sizes are 100,000 and 1,000,000 independent SQL document rows. They
are **not** a measurement of packed Records events, Metrics samples, arbitrary
KV codecs or encrypted KV search. No requirement here justifies an engine-wide
schema that makes those different units look interchangeable.

## Architectural candidates

| Candidate | Embedded / runtime | Atomicity and restart | Operational and implementation cost |
|---|---|---|---|
| Native SQLite B-tree | Already embedded; synchronous; no async runtime dependency | Same source transaction, SQLite recovery, ordinary prepared SQL | Lowest additional cost; supports range/order/partial/expression predicates; not a full-text tokenizer |
| Relational postings or grid in SQLite | Same SQLite file; ordinary SQL | Same transaction with source, triggers or explicit writer operations | Extra mapping and maintenance; query selectivity determines benefit; grid needs cell size and exact refinement |
| SQLite FTS5 / RTree | Bundled SQLite, no extension loading or Tokio requirement | Same-file source and projection can share commit and snapshot | Special update/delete rules, tokenizer/precision semantics, generation lifecycle, extra WAL; availability must be checked in the pinned SQLite build |
| Tantivy 0.26.2 | Embedded directory, mmap and background indexing/merge threads; synchronous API usable outside Tokio | Its own durable commit is independent of SQLite's | Requires persistent outbox/revision barrier, idempotent replay, generation publication and disk budgeting; no evidence justifies hiding weaker consistency behind the native API |
| rstar 0.12.2 | In-memory Rust R*-tree, no async runtime | Rebuildable cache; not a durable transactional index by itself | Bulk loading and exact f64 geometry are convenient, but source loading/startup/RAM and query/write barriers are additional work |
| Roaring 0.11.3 | In-memory compressed integer sets; no async runtime | Same cache limitations; serialization is not a commit protocol | Good set intersections; range predicates still need values or another structure. Sparse/signed identities need mapping and its maintenance |

Tantivy is MIT; rstar and Roaring are MIT OR Apache-2.0; rust-stemmers 1.2.0
is MIT/BSD-3-Clause. Dependency versions and features are pinned in Cargo.lock.
The measured Tantivy configuration indexes term membership only, with Basic
postings and no ranking/positions/stored document body. Default FTS5 also
provides positions; the cost of that additional capability must not be silently
credited to an implementation difference. Measurements do not establish
equivalence for phrases, BM25, prefixes, substring search or multilingual ranking.

Other reasonable designs remain candidates rather than implementations:
immutable sorted postings plus a transactional delta log, shadow-table trigger
capture, a stable internal surrogate ID, and copy-on-write segment manifests.
Snapshot + replay needs ordered tombstones and a bounded catch-up tail; a
long-lived SQLite snapshot pins WAL and copies stale rows, so an outbox alone
does not make final publication online or cheap. The dirty-ID prototype below
is the simplest measured member of that family.

Vector search has no established requirement in these workloads. SQLite-native
exact vector scans/extensions and external ANN libraries such as USearch or
hnsw_rs are possible later candidates. ANN would introduce dimension/model,
distance, recall, update, persistence and source/index consistency contracts.
No vector prototype, ANN benchmark or new vector API is part of this round.

## Workload and interpretation

The source has an integer identity, original body, pre-normalized terms,
category, score, f64 longitude/latitude and an unrelated update column. Source
body **and** terms occupy disk and are reported separately from index objects.
All matching IDs are materialized and sorted. Native predicates and Rust
candidates perform the same result-set operation within each class. Secondary
search is category equality plus a score lower bound; geo search is an inclusive
rectangle over authoritative f64 coordinates; text search is a single
pre-normalized token. No LIKE performance is presented as MATCH equivalence.

Text distributions are a repeated, augmented Russian treebank, a synthetic
skewed hot/rare-term mixture (named `zipf`, not a fitted mathematical Zipf law),
and repetitive business text with unique identifiers. Coordinates are uniform
worldwide or concentrated within 0.01 degrees near Moscow. Rectangle widths
vary over four selectivities. Query quantiles describe the specified fixed
mixture, not each selectivity separately. Source insertion is ordered in the
main matrix; shuffled insertion is a separate sensitivity scenario.

The pinned Russian treebank contains 24,516 sentences. At 100k/1m rows they
repeat, with document/order identifiers appended; neither dataset contains
100k/1m independent natural sentences. Corpus source, revision and SHA-256
are in `corpus.json`. Its CC BY-NC-SA 4.0 text is downloaded and ignored, not
redistributed in this repository.

## Incremental WAL is not index size

For 1m uniform source rows, 100-row transactions and 10,000 rows per mutation
phase, both passes have identical WAL frame counts. Total source-plus-index
WAL divided by the matched source-only scan baseline is:

| Projection | Insert amplification | Indexed update | Delete |
|---|---|---|---|
| B-tree | 20.475× | 22.787× | 12.152× |
| Relational postings | 13.075× | 11.740× | 12.095× |
| Compact FTS term set | 12.910× | 8.563× | 3.452× |
| Positional external-content FTS | 20.797× | 14.938× | 3.466× |
| RTree | 22.734× | 28.212× | 26.140× |
| One-degree grid | 13.671× | 11.785× | 12.274× |

These ratios describe this fixture/transaction locality, not a constant per
index type or amplification relative to logical payload bytes. Ordinary B-tree
maintenance also dirties many pages when category inserts scatter across leaves.
Absolute WAL bytes/row and batch tails are in the tables. A future physical
replica would receive source and derived-page frames, including repeated page
versions; a small final index does not guarantee a cheap update stream.

## Online rebuild algorithms

All generations carry an immutable definition, state, optional fixed maximum
source key `H`, optional processed frontier `F`, and active flag. Source changes,
metadata changes and index changes use the same SQLite file and transaction.
The writer is shared with foreground writes. Readers fetch the active generation
and search it inside **one** SQLite read transaction. Retiring generations stay
present in this prototype; cleanup and old-reader admission require a separate
lifecycle contract.

The lifecycle matrix uses contentful positional FTS (`fts`) and RTree. Its new
FTS definition appends a common `def2` marker, allowing definition/cardinality
checks; it is not a byte-identical rebuild of the compact external-content
format. All competing rebuild algorithms build that same definition, but the
no-rebuild control keeps the original definition. Static compact FTS savings
cannot be multiplied into these rebuild timings. A compact managed projection
needs its own OLD-token/delete and generation-specific-content integrity gate;
especially dirty-ID replay cannot delete external-content postings using only
the source's already changed text.

| Algorithm | Backfill rule | Concurrent source-write rule | Main cost / failure mode |
|---|---|---|---|
| No rebuild | Active generation only | Maintain active projection | Baseline for the same foreground mutations and reader mix |
| Monolithic | Build and publish in one writer transaction | Foreground waits | Simple atomicity, writer pause proportional to build, large transaction/WAL |
| Frontier | Keyset scan up to fixed `H`; commit projection and `F` together | Maintain new projection for `id <= F` and `id > H`; backfill owns the interval between | No per-row seen set; requires OLD/NEW identity logic and atomic frontier advancement |
| Seen IDs | Skip IDs already represented in the new generation | Eagerly index touched/inserted rows and maintain membership | More metadata and writes; no reliance on identity being immutable |
| Dirty IDs | Scan live source; transactionally log changed OLD and NEW IDs | Active projection is maintained; build projection is repaired at the end | Small write-path capture, but final dirty reconciliation is an unbounded writer tail in this prototype |

For frontier, an update that moves a key must handle OLD and NEW independently:
remove OLD only if covered, add NEW only if covered. Inserts into holes at/below
`F` are indexed immediately. A move into the unprocessed interval is indexed by
backfill; a move out to `>H` is indexed immediately. NULL frontier means no rows
processed; it is not integer zero. A NULL high watermark represents an empty
source and all subsequent inserts must be indexed. Keyset SQL must retain a
usable key range: a nullable `(? IS NULL OR id > ?)` predicate can defeat the
seek and turn many small chunks into repeated scans. The harness uses an
inclusive, checked lower key bound, including `i64::MIN`.

Time targets are checked between rows, before COMMIT; they cannot bound fsync,
FTS internal merges or a single expensive row. WAL-frame targets adapt the next
chunk's row count. They are **not hard dirty-page limits**: repeated versions of
one page and new pages both count as frames, and the first chunk can overrun.
Setup, checkpoints and final publication also hold the writer. A production
scheduler needs explicit accounting for all of these, bounded input bytes, and
admission that yields to foreground work.

Selective maintenance compares only indexed columns and identity. Unrelated
column updates still change the source but do not delete/reinsert a projection.
Both the old active and new building projection use the same policy in each
comparison; therefore the measured savings include both generations' work.

```text
writer transaction:
  retain OLD identity and indexed values; apply source mutation
  for active/building generation G:
    if OLD is covered by G: delete OLD projection using OLD token values
    if NEW is covered by G: insert projection from NEW using G.definition
  commit source, projections and any capture metadata together

builder transaction:
  read next keyset chunk within fixed H
  index rows using immutable G.definition
  advance F in this same transaction; commit; yield writer

publication transaction:
  verify G is still the requested definition and backfill is complete
  reconcile remaining capture if the algorithm requires it
  atomically retire previous generation and activate G
```

The logical handle's prepared-query cache must include generation/definition;
SQL table identifiers cannot be replaced by value parameters. A same-file
projection also needs an ownership boundary: arbitrary raw SQL can corrupt a
managed virtual table even when SQLite itself is transactionally correct.
Choose user-owned raw SQL or protect managed objects and resolve logical names.

## Correctness and failure evidence

The deterministic suite exercises minimum/negative/zero IDs, holes, updates and
deletes on both sides of the frontier, key moves in both directions, reuse of a
deleted key and inserts beyond `H`. Superseding a definition aborts the old build
and creates a new generation; it never relabels an existing partial projection.
Concurrent scenarios combine INSERT, indexed/unrelated UPDATE, DELETE, identity
changes and independent reader snapshots.

Crash points cover committed generation creation, an uncommitted chunk,
committed chunk/frontier, uncommitted activation, committed activation, and
uncommitted retirement. A parent waits for a flushed handshake, SIGKILLs the
worker, reopens SQLite, resumes the generation and builds a fresh oracle from
the current source. Search results, projection cardinality/definition markers,
bidirectional RTree differences and SQLite/FTS integrity checks must agree.
These are process-crash tests on Linux, not simulated power-loss or torn-write
tests. Geo validation compares every stored bound in both directions plus eight
queries (two per selectivity); FTS compares complete stored terms/cardinality
and 120 queries. Large crash probes repeat resume and committed switch at both
sizes; the small suite covers all six stages and three online algorithms.
Definition changes append a deterministic marker, not arbitrary tokenizer/DDL
changes. That wider schema-change lifecycle remains a product gate.

### Russian text

`unicode61` lowercases/tokenizes but is not Russian morphology. Explicit ё→е
normalization plus Snowball improves the small gold set's city/car inflections,
but fails suppletion (`человек`/`люди`) and conflates `сталь` with `стали` in an
unrelated verb context. The raw file contains every expected/actual ID and
precision/recall per query. Seven sentences and five hand-labelled queries
demonstrate these counterexamples; they are not an estimate of search quality
on a population. Neither FTS5 nor Tantivy supplies correct Russian lemmatization
merely by selecting it as the storage engine. A dictionary/lemmatizer, its
license/size and a much larger gold set need a product decision.

The common stored `terms` field is a fixture choice, not a universal schema
recommendation. Its disk cost is included for every candidate. A new normalizer
must recompute a generation-specific projection from original body; one global
normalized source column cannot represent two concurrent definitions. Native
Unicode61 works today, but Snowball here runs in the research producer, not a
new Core FTS tokenizer. A product must choose application-provided terms,
managed projection storage or a versioned custom tokenizer, account for its
space/CPU, and verify connection registration/trusted-schema compatibility.

### Geo and direct SQL

RTree coordinates are outward-rounded float32 bounds. Two points separated by
1e-8 degrees become candidates for a rectangle that should contain one; joining
the authoritative f64 source and applying the exact predicate returns one.
A certified alternative can skip a source lookup when its outward box is
wholly inside the query, using SQLite lazy CASE; only boundary candidates need
source refinement. This is correct only for finite, non-NULL, validated earth
coordinates and an atomically consistent source/index snapshot. The quality
probe shows RTree accepts NULL as zero and that skipping refinement can then
return a false positive. Input validation is part of the proof.

Inclusive overlap for candidate selection is necessary: containment against
rounded bounds can introduce false negatives. Longitude wrap must split a
dateline-crossing box; Euclidean degree distance is not a spherical radius or
nearest-neighbour contract. ±180/±90 are valid boundaries; non-finite/out-of-range
coordinates, NULL and SRID policy need explicit validation in a public API.

Changing the active generation does not redirect SQL naming `g0`. Cached SQL
continues to address that physical table. A separate ordinary-table rename
probe shows SQLite reparses a prepared statement when the old SQL name is
rebound to a new table; this is **not** proof that arbitrary virtual-table
renames, shadow objects and user-created triggers are a safe online API.
Direct virtual-table access and a managed generation abstraction cannot both
silently promise a stable physical table identity. Keep raw SQL user-owned,
or expose a logical handle with generation resolution and documented retirement.

## Recommendation to hand to Opus

Do not build a universal Index Engine before v0.1.0. Keep existing engine-local
indexes and ordinary SQL B-trees. If owner requirements confirm public full-text
and spatial search, implement narrow SQL-owned projections in the same database
as their source; prototype measurements decide their default structure and
rebuild policy, not the phase list. KV/Records indexing requires independent
identity/projection/retention design; Metrics already has label postings.

For a managed same-file rebuild, carry an immutable definition and durable
generation state, maintain source/index atomically, use keyset batches, and
resolve active generation in the reader's snapshot. Prefer the simplest measured
algorithm that meets foreground tail-latency and WAL budgets. Retire only after
readers/handles release the old generation. Keep a failed build resumable or
abort it transactionally; never publish it merely because a scan returned no
rows once. Space preflight includes source, two index generations, WAL pinned by
readers, temporary work and safety reserve. Old generations must eventually be
reclaimed; a successful switch alone does not reclaim disk.

An external Rust text engine is an optional future design with an explicit
consistency boundary: source transaction appends a revisioned outbox; replay
updates the external index idempotently; only a committed external watermark
can be published. Crashing after external commit but before SQLite acknowledgement
must replay safely. A source snapshot plus tail repair must cover deletions,
identity changes and definition versions. Query consistency requires a revision
barrier, source validation or a declared lag. Do not present this unimplemented
protocol as measured atomicity.

Before v0.1.0, document SQL-native index support and test its pinned build,
trigger maintenance, rollback, SQLite recovery and SDK result semantics. Add a
managed online rebuild only if its owner-approved availability requirement
cannot be satisfied by migrations/maintenance windows. Defer generic cross-engine
search, dictionary morphology and external index synchronization unless a
concrete workload pays for them. Vector search and replication remain outside
implementation scope.

## Owner decisions and product acceptance gates

The owner must choose whether any managed public index handle is needed at all;
which engines expose projections; token/phrase/ranking/Russian normalization
semantics; geo rectangle/radius/coordinate precision; direct SQL ownership;
rebuild cancellation/redefinition/observability; tolerated write p99, WAL and
disk reserve; and whether external-index lag is ever acceptable. These are API
decisions, not permission requests to run further research.

The product gate must cover source/index equality after every lifecycle stage,
transaction rollback and grouped-commit order, randomized mutation histories,
empty/sparse/extreme identities, long readers, overlapping definitions, cleanup,
disk full, memory exhaustion, restart and pinned prepared statements. Run the
same matrix on supported OS/filesystems and actual SDK paths. Enforce memory
reservation for both Rust and SQLite, bound final reconciliation, and account
checkpoint/merge/publication work in writer budgets. Physical replication cost
is the source **plus** derived-index WAL; an online rebuild can lower peak
writer pause while increasing cumulative physical bytes. This round does not
measure a replica sender or establish a production SLO.

## Environment and reproducibility

Linux 6.18.44, Xeon Platinum 8573C, three visible CPUs/two-CPU quota,
6 GiB container RAM, disk-backed named volume, Rust 1.99.0, Bun 1.4.2,
SQLite 3.53.2 via rusqlite 0.40.2. Research native connections use FULL WAL,
4096-byte pages, 4 MiB cache per connection and a common WAL observer/manual
PASSIVE checkpoint policy. The actual Core default is 1 MiB cache and
`trusted_schema=0`; cache sensitivity is reported separately. Native timing is
not a claim to measure a future Core index API. Two unpaced reader threads
compete with an offered 1000 foreground transactions/s; 20% inserts, 10%
deletes, 10% identity moves and 60% updates. Update-column share and transaction
size are separate cases. Scheduled latency includes overload backlog, whereas
writer service excludes admission wait.

The [evidence ledger](data/index-foundations-2026-10-15/README.md) provides
commands, configuration, cohort boundaries, fixes and raw-file provenance.
The [measurement tables](index-foundations-measurements-2026-10-15.md) keep
each pass separate. Stripped whole research CLI sizes are 2,871,944 bytes
for native baseline, 6,888,760 with text, 2,963,144 with geo and 2,923,720
with bitmap. These reachable executable sizes are not incremental library
artifact sizes; baseline already includes SQLite and Snowball.
