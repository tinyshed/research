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

A report names the TinyStore commit it measured; an older round is reproduced
by checking `source/` out at that commit. The rules for a round are in
[AGENTS.md](../AGENTS.md).

Corpora are fetched, never committed: the runners in `bench/` download and
normalise them into `bench/corpus/`, and a hash file pins each one.
