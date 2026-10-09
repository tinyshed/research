# TinyStore's design documents

The documents TinyStore was designed and built to, moved here from its
`docs/` on 3 October 2026, when the guides took that directory. Each says why
an engine has its shape, what was measured before it was built and what
building it settled; their history is TinyStore's, `git log --follow --
docs/<file>` there. They are kept as they were and are not updated with the
code: what TinyStore promises today is each package's README and
[its guides](https://github.com/tinyshed/tinystore/tree/main/docs).

| | |
|---|---|
| [architecture.md](architecture.md) | the runtime: one directory, a file per engine, opening, memory, time, logs, errors, weight |
| [metrics.md](metrics.md) | the metrics store: why not the Prometheus TSDB, blocks, the head, the watermark, retention, capacity |
| [aggregate-contract.md](aggregate-contract.md) | exact aggregate arithmetic: sums rounded once, counter resets, boundaries, groups |
| [format.md](format.md) | the bytes of a metrics payload and a group directory, version by version |
| [group-commit-contract.md](group-commit-contract.md) | how grouped writes share a commit and fail alone |
| [records.md](records.md) | logs and events: the record, order, segments, encoding, search |
| [sqldb.md](sqldb.md) | the application's SQLite: models, values, migrations, the tool |
| [kv.md](kv.md) | current state: keys, values, expiry, versions, counters, configs, limiter, quota, once |
| [jobs.md](jobs.md) | work at its time: keys, a job's life, repeats, working, steps |
| [blobs.md](blobs.md) | files: keys, uploads, crash consistency, integrity, readers on Windows |
| [server.md](server.md) | the server and its sidecar: modes, discovery, capabilities, versions, limits |
| [sdk.md](sdk.md) | the API in Go, Bun and Python side by side, and the rules that keep them alike |

The wire protocol stayed with TinyStore, as
[docs/wire.md](https://github.com/tinyshed/tinystore/blob/main/docs/wire.md):
the server and every SDK are tested against it.
