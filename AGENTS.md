# tinyshed research

The research grounds of the tinyshed projects: prototypes, measurements and
the dated reports behind their decisions. Nothing here is a product, nothing
here may be quoted as a product's promise, and no product imports anything
from here.

This file is the contract for anyone — human or agent — working in this
repository. A project's own rules are in its repository's AGENTS.md, which
this one repeats only where a measurement needs them.

## Shape

| Path | What it is |
|---|---|
| `tinystore/source/` | TinyStore, a submodule pinned to the commit the latest round measured |
| `tinystore/reports/` | the dated rounds and their raw output in `data/`; `README.md` indexes them |
| `tinystore/measurements.md` | every number the rounds settled on |
| `tinystore/open-questions.md` | what is not built, and the gate each question must pass |
| `tinystore/design/` | TinyStore's design documents, moved from its `docs/` when the guides replaced them; kept as they were |
| `tinystore/spike/` | prototypes and measurements, a module of its own |
| `tinystore/server-spike/` | the server measured through `tinystore serve` |
| `tinystore/bench/` | corpus runners and harnesses against other engines, a module each |
| `.agents/skills/` | how the recurring work is done; `.claude/skills/` points to it |

## How the code builds

Every Go module under `tinystore/` is named under
`github.com/tinyshed/tinystore/`, because Go lets only a path under that
prefix import TinyStore's `internal/`, whichever repository holds it. Each one
`replace`s TinyStore with `../source` (or `../../source`), so it builds against
the submodule's commit and nothing else.

```sh
git submodule update --init
cd tinystore/spike
GOWORK=off TINYSTORE_SPIKE=1 go test -run <name> -v -count=1 .
```

**The submodule's commit is part of the round.** Move it on purpose, with
`git -C tinystore/source checkout <commit>`, fix what the move broke, and
commit the new pointer with the round that measured it. A report names the
TinyStore commit it measured; TinyStore never rewrites a commit a report
cites, and neither do we here.

To measure TinyStore work not yet on its `main`, commit it on a branch and
check that commit out in the submodule, as the `measure` skill says; the
report waits until the commit is on TinyStore's `main`.

A module that does not build against the pinned commit is broken, not
archived: fix it or delete it, and say so in the round that noticed.

## Measurements

**A measurement is a number with its environment, or it is an anecdote.**
Every figure carries what produced it — machine or container, versions,
fixture, sample count — and the command that reproduces it. A candidate is
compared against what it replaces on identical input, in the same run. Take
the economics from a measurement and not the explanation of the mechanism:
the number is evidence, the story about why is a hypothesis until a second
measurement separates it from the alternatives.

**A storage measurement is a division of the file, not a total.** `dbstat`
reports every b-tree's own pages, so a change that claims to save space says
which object it took the bytes from and which object it gave them to. An
index deleted here and an index created there net to zero, and only a
per-object report shows it.

**A payload's size is not a file's size.** SQLite stores rows in fixed pages,
so a payload that shrinks by a tenth can leave the file exactly as large, and
one that shrinks by a fiftieth can shrink it sharply by fitting one more row
on a page. Every codec result is reported twice: bytes a sample in the
payload, and bytes a sample in a real file.

**Heavy measurements run in a Linux container**, or their numbers cannot sit
beside the others; the `measure` skill has the command. Do not claim a
platform was measured from a different operating system.

## Rounds

**A round is prototype code and one dated report.** The report is
`<project>/reports/<topic>-<date>.md`, one line in that folder's `README.md`,
and the machine, the versions, the corpus, the commits and the command
inside the report itself. Raw output goes beside it in `reports/data/`.

**A later round supersedes an earlier one by saying so in the earlier one**,
never by editing the number it replaces.

**A reproduction command carries no path from the machine that ran it.**
`<repo>` for a repository, `<corpus>` for a prepared corpus. A real path is
useless to the reader, and a home directory is a username published for as
long as the history lasts.

**A corpus is fetched, never committed.** The runners in `bench/` download
and normalise one into `bench/corpus/`, which is ignored, and a hash file
pins it. A private corpus is named, never published.

**What a round proved is worth building lands in the project's repository**,
as its own commit there; the round here only reports it.

## Commits

One Conventional Commits subject line, English, imperative, lower case after
the type. No body, no footers, no trailers of any kind. A round is
`test(spike):`, its report `docs:`, moving the submodule `chore(tinystore):`.
Do not commit or push unless asked.

## Skills

`measure` is how a measurement is run, explained and written up. TinyStore's
own skills, `platform-traps` above all, are in
`tinystore/source/.agents/skills/`; read the one that fits before touching
files, processes, sockets or timing.
