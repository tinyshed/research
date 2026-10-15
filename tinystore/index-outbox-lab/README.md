# Transactional outbox cost for an external term index

Companion to index-foundations. This experiment shares the exact source fixture,
SQLite settings and native FTS-set baseline, but implements a second durable
commit boundary for Tantivy. It is not product code and changes no public API.

Source changes append OLD/NEW identities to a SQLite outbox in the source
transaction. Backfill reads a stable source snapshot. Replay deduplicates dirty
IDs and reconciles their current snapshot state, delete-before-add. The source
revision is stored in Tantivy's own committed metadata payload; SQLite progress
acknowledgement and outbox cleanup happen afterward. A crash after the external
commit but before acknowledgement can use the durable external watermark.

Strong queries serialize index dispatch, establish a source read snapshot,
catch the index up to that snapshot, commit/reload and retain a Searcher for
that revision. Source writers continue. This deliberately measures a consistent
read barrier, rather than calling a stale external search a SQLite snapshot.
It does not implement a public query router, definition changes, old-generation
retirement or multi-index transaction semantics.

Build and timing phases, workload and crash commands are recorded by run.py.
Build before measurements and keep all other experiments stopped. Database and
index directories must be on the disk-backed scratch volume. The Russian corpus
is fetched by ../index-foundations/corpus.py and is never committed.
