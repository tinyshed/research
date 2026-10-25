# Dashbin's architecture

The design of the Rust rewrite, written before its code. A chapter is
**decided** when a measurement or an explicit decision stands behind it,
**proposed** when it was discussed and not yet agreed, **open** otherwise. A
decided chapter's rules move into the `rust` branch's `AGENTS.md`; this plan
keeps the reasoning, the reports keep the numbers.

| | Chapter | Status | Behind it |
|---|---|---|---|
| 1 | Product and promises: what Dashbin is, for whom, the 64 MiB promise, what it is not | partly decided | [memory and allocators](../reports/memory-and-allocators-2026-10-24.md) |
| 2 | [Concepts and words](02-concepts.md): one meaning a word | decided | three scenarios in the chapter |
| 3 | [The frame](03-frame.md): column types, nulls, time, money, units, dimensions, encodings, one schema for Rust and TypeScript | decided, our own wire included; caps open | Go Dashbin's gates and its PostgreSQL and MySQL mappings, [the wire](../reports/wire-format-2026-10-25.md) |
| 4 | Storage on TinyStore: which engine holds what, migrations, backup, the memory budget | proposed | [TinyStore 0.1.1 recheck](../reports/tinystore-0.1.1-recheck-2026-10-24.md) |
| 5 | [Running a query](05-running-a-query.md): which instances, parameters, `from`/`to`/`step`, limits, cancellation, joining identical calls, failures, reading collected data | decided; numbers and cancellation to be measured | [TinyStore 0.1.1 recheck](../reports/tinystore-0.1.1-recheck-2026-10-24.md), Go Dashbin's parameter rules |
| 6 | [Plugins](06-plugins.md): functions, settings, ids and translations, packages, installs and upgrades, the host, processes, built-ins | decided; the host measured, the rest listed as unmeasured | [SDKs in QuickJS](../reports/sdks-in-quickjs-2026-10-25.md), [plugin host](../reports/plugin-host-2026-10-25.md) |
| 7 | Flows: one executor on TinyStore's jobs for collectors, alerts, webhooks and buttons; idempotency | proposed | — |
| 8 | Agents: one binary, placement, the protocol, the operator's trust, local configuration, buffered streams | proposed | Go Dashbin's agent |
| 9 | Dashboards and the browser: the dashboard model, the panel contract, built-in charts, suggestions, themes | open | Go Dashbin's suggestion weights |
| 10 | Security and access: accounts, roles, sessions, CSRF, TLS, sealed secrets, audit, trust in plugins | decided in Go Dashbin, plugins open | — |
| 11 | The HTTP API: shape, types, generated TypeScript | open | — |
| 12 | The running process: modes of the binary, configuration, allocator, logs, own metrics, backup, upgrades | allocator and modes decided | [memory and allocators](../reports/memory-and-allocators-2026-10-24.md) |
| 13 | Code and repository: crates, the gates, the new `AGENTS.md` | open | Go Dashbin's `AGENTS.md` |
| 14 | The first version and its phases, and the slices `research` needs before the code | open | — |
