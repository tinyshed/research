# TinyStore research

What TinyStore's decisions were measured on, moved here from
[tinyshed/tinystore](https://github.com/tinyshed/tinystore) at
[`0936f0f`](https://github.com/tinyshed/tinystore/tree/0936f0f6487202923315178fb0a4976e59f28f05),
where every file below had its history.

| | |
|---|---|
| [reports/](reports/README.md) | the dated rounds, each with its environment and reproduction command |
| [measurements.md](measurements.md) | every number the rounds settled on |
| [open-questions.md](open-questions.md) | what was left open, and the gate each question must pass |
| [design/](design/README.md) | the design documents TinyStore was built to, as they were when its guides replaced them |
| [storage-runtime-direction.md](storage-runtime-direction.md) | the 22 September assessment of outside designs for shared SQLite engines |
| [rewrite.md](rewrite.md), [samples/](samples/) | how the metrics code was rewritten for people |
| [spike/](spike/) | the prototypes and measurements behind the rounds |
| [server-spike/](server-spike/) | the server measured through `tinystore serve` |
| [bench/](bench/) | corpora runners and harnesses comparing against other engines |
| [source/](https://github.com/tinyshed/tinystore) | TinyStore itself, the submodule everything here builds against |

## Running the code

`source/` is TinyStore as a submodule, pinned to the commit the latest round
measured. `spike/`, `server-spike/` and `bench/*` are modules named under
`github.com/tinyshed/tinystore/`, so Go lets them import TinyStore's
`internal/`, and each replaces TinyStore with `source/`:

```sh
git submodule update --init
cd tinystore/spike
GOWORK=off TINYSTORE_SPIKE=1 go test -run <name> -v -count=1 .
```

A report names the TinyStore commit it measured. To reproduce an older round,
check out its matching research harness revision as well as its TinyStore
`source/` commit; later compatibility edits can target a newer public API. The rules for a round are in
[AGENTS.md](../AGENTS.md).

Corpora are fetched, never committed: the runners in `bench/` download and
normalise them into `bench/corpus/`, and a hash file pins each one.

## Rust evaluation, 7–8 October 2026

Start with the [research synthesis](reports/rust-research-summary-2026-10-08.md).
The retained standalone prototypes are [algorithm kernels](rust-spike/),
[SQLite linkage smoke](sqlite-rust-spike/), [matched native SQLite](sqlite-bench/),
[metrics paths](metrics-bench/), [first optimizations](metrics-opt-bench/) and
[deeper optimizations](metrics-max-bench/),
[records algorithms/native slice](records-native-bench/) and
[KV algorithms/native slice](kv-native-bench/). Each directory contains its source,
locked dependencies and reproduction instructions; the reports link raw output.
TinyStore is pinned to `e307c48a40126aad0e2873b6bf3aaedef8115483` for these rounds.
The production Go repository remains unchanged by this evaluation.
