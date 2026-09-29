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
| [rewrite.md](rewrite.md), [samples/](samples/) | how the metrics code was rewritten for people |
| [spike/](spike/) | the prototypes and measurements behind the rounds |
| [server-spike/](server-spike/) | the server measured through `tinystore serve` |
| [bench/](bench/) | corpora runners and harnesses comparing against other engines |

## The code is an archive

`spike/`, `server-spike/` and `bench/` do not build here. They import
TinyStore's `internal/` packages, which Go lets only TinyStore's own module
import, so each runs from a TinyStore checkout at the commit its report names:

```sh
git -C <tinystore> checkout <commit the report names>
cd <tinystore>
TINYSTORE_SPIKE=1 go test ./spike -run <name> -v -count=1
```

A report names the commit it measured, and those commits stay in TinyStore's
history. Links from a report to TinyStore's design documents point at the
commit this was moved from.

Corpora are fetched, never committed: the runners in `bench/` download and
normalise them into `bench/corpus/`, and a hash file pins each one.
