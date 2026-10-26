# Dashbin's architecture

The design of the Rust rewrite, written before its code. A chapter is
**decided** when a measurement or an explicit decision stands behind it,
**proposed** when it was discussed and not yet agreed, **open** otherwise. A
decided chapter's rules move into the `rust` branch's `AGENTS.md`; this plan
keeps the reasoning, the reports keep the numbers.

| | Chapter | Status | Behind it |
|---|---|---|---|
| 1 | [Product and promises](01-product.md): what Dashbin is, for whom, the 64 MiB promise, how technology is chosen, what it is not | partly decided: 64 MiB, choosing technology | [memory and allocators](../reports/memory-and-allocators-2026-10-24.md) |
| 2 | [Concepts and words](02-concepts.md): one meaning a word | decided | three scenarios in the chapter |
| 3 | [The frame](03-frame.md): column types, nulls, time, money, units, dimensions, encodings, one schema for Rust and TypeScript | decided, our own wire included; caps open | Go Dashbin's gates and its PostgreSQL and MySQL mappings, [the wire](../reports/wire-format-2026-10-25.md) |
| 4 | [Storage on TinyStore](04-storage.md): the directory, what commits together in one file, secrets, migrations, backup, the memory split, retention | draft, first round | TinyStore 0.3.0's source: engines inside a database, encrypted buckets, backup |
| 5 | [Running a query](05-running-a-query.md): which instances, parameters, `from`/`to`/`step`, limits, cancellation, joining identical calls, failures, reading collected data | decided, calculations added 27 October; numbers and cancellation to be measured | [TinyStore 0.1.1 recheck](../reports/tinystore-0.1.1-recheck-2026-10-24.md), Go Dashbin's parameter rules |
| 6 | [Plugins](06-plugins.md): functions, settings, ids and translations, packages, installs and upgrades, the host, processes, built-ins, official plugins | decided, official `telegram` added 27 October; the host measured, the rest listed as unmeasured | [SDKs in QuickJS](../reports/sdks-in-quickjs-2026-10-25.md), [plugin host](../reports/plugin-host-2026-10-25.md) |
| 7 | [Flows](07-flows.md): one executor on TinyStore's jobs for collectors, alerts, webhooks and buttons; windows, alert states, idempotency | draft, second round after review | TinyStore 0.1.1's jobs, read from its source; 0.2.0 rerun by programs |
| 8 | [Agents](08-agents.md): one binary, placement and approval, the protocol, the operator's file, ceilings, plugins on an agent, gaps while away | draft, second round after review | Go Dashbin's agent, proven on two servers; TinyStore 0.2.0's `config::load` |
| 9 | [Dashboards, the browser and the terminal](09-dashboards.md): a dashboard as one document, a panel's own and shared queries, rows of panels, range and variables, laying out frames, suggestions, plugin visualizations in a sandbox, themes, `dashbin show`, chats after the first version | draft, second round after review | Go Dashbin's dashboards, suggestion weights and frontend contract |
| 10 | Security and access: accounts, roles, sessions, CSRF, TLS, sealed secrets, audit, trust in plugins | decided in Go Dashbin, plugins open | — |
| 11 | The HTTP API: shape, types, generated TypeScript | open | — |
| 12 | The running process: modes of the binary, configuration, allocator, logs, own metrics, backup, upgrades | allocator and modes decided | [memory and allocators](../reports/memory-and-allocators-2026-10-24.md) |
| 13 | Code and repository: crates, the gates, the new `AGENTS.md` | open | Go Dashbin's `AGENTS.md` |
| 14 | The first version and its phases, and the slices `research` needs before the code | open | — |
