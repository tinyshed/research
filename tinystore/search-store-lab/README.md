# Standalone Search owns its documents

Follow-up registered after the first paired RU ranked-query results: Tantivy's
large top-10 latency advantage justifies checking an alternative atomicity
boundary. A standalone Search collection need not maintain a separate SQLite
source if Tantivy stores its document fields itself. SQL-table Search still
requires same-source transactional consistency or an explicit outbox contract.

This is a new comparison, not permission to equate an external postings-only
index with a whole collection. Both candidates here own keys/title/body,
persist them with the search structures, and hydrate the actual top-10 fields:

- SQLite contentful FTS5 + unique key/id mapping, FULL WAL transactions.
- Tantivy with stored key/title/body, indexed key and fast/stored numeric id;
  delete/add and commit/reload before returning an operation as complete.

The shared `auto` analyzer is imported unchanged from search-lab. Whole-collection
bulk import is one durable commit. Incremental phases compare individual versus
100-document commits separately. Snapshot hydration uses one SQLite statement
or one pinned Tantivy Searcher. No secondary SQLite source or hidden outbox is
charged to Tantivy here; its original document bytes are in its stored fields.

This candidate has not inherited TinyStore's grouped writer, memory admission,
SQL transactions, SQLite backup/WAL replication, OS coverage or Wire API. Those
integration costs must be assessed before recommending a product backend.
This module is research-only and never changes Core/public API.
