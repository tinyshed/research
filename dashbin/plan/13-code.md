# 13. Code and repository

**Status: draft, second round, 27 October 2026.** Where the code lives was
decided on 25 October. The first round split the code into twelve crates;
the second follows TinyStore's repository, which already holds Dmitry's way
of laying out Rust: three crates, a library whose parts are features, a
generator for what crosses a process, `just`, and contracts with ids. Nothing
in it is agreed until Dmitry says so; the choices a review should look at are
under *Forks*.

How the Rust rewrite is laid out, what holds it together, and the rules a
contributor — a person or an agent — is held to.

## Where it lives

- **The code is the `rust` branch of `tinyshed/dashbin`**, and only finished
  code goes there; spikes, prototypes and measurements stay in `research`, as
  decided on 25 October. When the Rust version runs the first user's ten
  sites, `rust` becomes `main`, and the Go code is kept under a tag.
- **This plan becomes the branch's `AGENTS.md` and `CONTRACTS.md`**: a decided
  chapter's rules move to the first, its promises to the second; the plan
  keeps the reasoning and the reports keep the numbers. `CLAUDE.md` imports
  `AGENTS.md`, and `.agents/skills/` holds the procedures with traps.
- **Versions are `v0.x.y`** until the product is usable by someone who is not
  its author, and a release is a tag that opens a draft for review.

## The repository

```
crates/
  dashbin/      the library: a module per concept, a cargo feature where a program may leave one out
  cli/          the binary `dashbin`: a file per command, and the one place everything is put together
  protocol/     a generator: the frame wire and the call written once, codecs and vectors made from them
protocol/       the definitions, *.wire
fuzz/           the decoders' fuzz targets
web/            SvelteKit, built with Bun, embedded by the binary
plugins/        the official JavaScript plugins, telegram first, embedded by the binary
sdk/            what a plugin's author imports: the types of `dashbin`, later the SDKs of process plugins
vendor/         llrt_fetch and rquickjs-core, patched, with their UPSTREAM and licences
testdata/       what the Rust and TypeScript sides test against together
docs/           the guides
justfile  CONTRACTS.md  AGENTS.md  rust-toolchain.toml  rustfmt.toml
```

### The library

**One crate, `dashbin`, a module per concept**, as `tinystore` holds an
engine a module:

```
frame     the frame, its types and units                    kernel    a query's path: resolve, bind, limits, calls, joining, calculations
access    the context, allowed, visible, scopes, grants     store     Dashbin's own data on TinyStore, migrations, the audit's outbox
host      QuickJS: runtimes, network policy, timers, arena  plugins   the built-ins, a module each
flows     flows on TinyStore's jobs                         agent     the agent's half
hub       the api on axum, sessions, the run stream          draw      a frame laid out for a terminal: lines, bars, tables
```

- **A part a program can leave out is a feature**, as each of TinyStore's
  engines is: `hub`, `agent`, `host`, `draw`, each built-in plugin. The binary
  turns them all on, because it is one file (chapter 12); the features are
  there so that a part can be built and tested alone, and so that the kernel
  can be embedded somewhere else without a web server or QuickJS — a program
  pays for what it links, as TinyStore's STORE-07 says of its own.
- **A module is a concept, and its types do not repeat its name**:
  `kernel::Call`, `store::Documents`, never `kernel::KernelCall`.
- **What a second deployment would reuse knows nothing of where it runs**:
  `frame`, `kernel`, `access` and the plugins import nothing of `hub`, `cli` or
  `host`; a test reads each module's imports and fails on one that reaches up.

### The binary

**`cli` is the binary**, a file per command as TinyStore's command line has
them — `see.rs`, `show.rs`, `serve.rs`, `agent.rs`, `plugin.rs` — and
`main.rs`, the one place that builds the hub, the agent or a command from the
library's parts, in one readable sequence. No container, no service locator,
no global set at start.

### The protocol, written once

**What crosses a process is described once and generated for every side.**
The frame wire (chapter 3) and the call — its envelope, `call`, `credit`,
`$cancel`, `agent.hello` (chapter 8) — are written in `protocol/*.wire`, and
`crates/protocol` is a program that writes from them:

- the Rust codecs the library uses;
- the TypeScript codecs the browser and the plugins' SDK use;
- the test vectors in `testdata/`, which both sides must read and write to the
  byte.

A process plugin in another language gets its codec the same way when its SDK
comes. This is how TinyStore keeps its Rust, TypeScript, Python and Go codecs
from drifting, and why chapter 3's two decoders cannot disagree about a frame:
neither is written by hand. Both are still fuzzed.

## The workspace's rules, in Cargo

As TinyStore's workspace sets them, for every crate at once:

```toml
[workspace.lints.rust]
unsafe_code = "deny"
unreachable_pub = "warn"
missing_debug_implementations = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
undocumented_unsafe_blocks = "deny"
unwrap_used = "deny"
dbg_macro = "deny"
todo = "deny"
```

- **`unsafe` is denied everywhere and allowed in `host` alone**, with an
  `allow` on that module and a `SAFETY:` comment on every block, which Clippy
  refuses to do without; `vendor/` is outside the workspace's lints. Miri runs
  over `host`'s arena and the frame decoder in CI.
- **A panic is a bug, never an error**: `unwrap` is refused outside tests, and
  a decoder answers with an error on any input.
- **The toolchain is pinned** in `rust-toolchain.toml`, edition 2024, moved on
  purpose; `rustfmt.toml` is the formatting, and nobody argues it.
- **Debug builds keep line tables only**, as TinyStore measured: about two
  fifths less of the target directory, and the same rebuild time.

## Rules carried from Go Dashbin

Each was a rule because something broke without it, and each stays:

- **Dependencies are passed, never resolved**: positional arguments while
  there are few, configuration last.
- **An error has a kind, a message safe to show and a hidden cause.** HTTP
  maps a kind to a status and the command line to an exit code, and nothing
  below them speaks either. A refused connection never names the host that
  refused.
- **Whoever stops an error logs it**, and nobody else.
- **The clock is passed**, never read inside, and TinyStore takes the same
  clock through its options.
- **The context is a parameter**: chapter 10's `Context` is passed to what
  needs it, never fetched from a task-local.
- **A trait exists for a second implementation or a test double**, declared
  where it is used.
- **A test double is never kinder than the real thing.**
- **A name that needs a comment is renamed instead**, and a comment says why,
  in one line.

## Building

- **No library of the system at run time**: the binary is static musl, and a
  check fails when it links anything dynamically. C is compiled in — SQLite
  through TinyStore, QuickJS, jemalloc — which needs a C compiler to build and
  nothing to run.
- **SQLite is built with TinyStore's flags**, in `.cargo/config.toml`:
  `SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT SQLITE_ENABLE_DBPAGE_VTAB
  -DHAVE_FDATASYNC=1`.
- **A release build is fat LTO, one codegen unit, stripped**, with jemalloc's
  configuration compiled in.
- **`web/build` carries a committed placeholder**, so `cargo build` works in a
  clean checkout without Bun.

## Dependencies

**A dependency is a decision** (chapter 1): its weight in the binary and in
memory against what it buys.

- **`cargo deny`** refuses a licence the AGPL cannot take, an advisory, and a
  second version of a crate where one would do.
- **The binary's size is in every CI job's summary**, with what grew since the
  last; no threshold to game, only growth that everyone sees.
- **What only tests use stays a dev-dependency.**
- **The api's schema is derived from the handlers' types** by one crate,
  chosen by what it adds to the binary and whether it reads axum's handlers as
  they are; the candidates are measured before one is taken.

## `just`

**The recipes a contributor runs are `just`'s, as in TinyStore; CI calls
cargo itself:**

```
just              the recipes
just quick        only what changed since the branch's upstream: fmt, clippy, the tests of each part touched
just check        everything CI checks
just contracts    every MUST in CONTRACTS.md named by a test, and no test naming an id it lacks
just protocol     the codecs and vectors written again from protocol/*.wire
just web          the browser's build, into web/build
just size         the release binary, and what grew
just fuzz         a long fuzz run, which CI does on its own
```

## Contracts

**`CONTRACTS.md` holds what Dashbin promises**, as TinyStore's holds what it
promises: what a user, an operator or a plugin can observe, never how it is
built. **MUST** and **MUST NOT** are absolute, **MAY** says what nobody can rely
on. Each contract has an id that never changes meaning; a test that keeps one
names it — `// contract: ACC-01` — and `just contracts` fails when a MUST is
kept by no test or a comment names an id the file lacks. A change that means
to move a contract changes the file in the same commit, and says so in its
subject.

The chapters' promises become its sections, for instance:

| Area | A contract it holds |
|---|---|
| FRAME | a decoder MUST answer every input with a frame or an error, and MUST NOT panic or allocate past the message it holds |
| CALL | a call's `$cancel` MUST stop its work on the other side; an answer for no call in flight MUST be dropped |
| QUERY | an empty selector MUST be refused; a viewer MUST NOT run a value a variable does not allow |
| CALC | a group missing an instance it depends on MUST NOT be answered as a number |
| ACC | nobody MUST receive data or cause an effect beyond their authority, except through a grant in force |
| STORE | a restore MUST hold nothing naming what it lacks |
| FLOW | a state and the notifications it causes MUST be written in one transaction |
| AGENT | an agent MUST answer only for the instances its operator configured, and within its ceilings |
| PLUG | the end of a call MUST stop every request its plugin started |
| API | every operation MUST be public on purpose; a listing MUST NOT be a shape a client can send back |
| CLI | a dashboard's file MUST survive being written, read and written again |
| PROC | the hub MUST fit 64 MiB under the scripted load; no secret MUST reach a log |

**Tests that need a database read its address from the environment and skip
without it**; CI always gives them PostgreSQL and MySQL, so the contracts they
keep are kept there. The 64 MiB contract runs the hub in a 64 MiB container
under a scripted load on Linux.

## Commits and branches

As in Go Dashbin, TinyStore and `research`:

- **One Conventional Commits subject line**, English, lower case after the
  type, scoped to what it touches; **no body, no footers, no trailers of any
  kind**, co-author and generated-by lines included.
- **A commit is a coherent piece of work**, not a save point.
- **`feat:` and `fix:` are what a user reads** in the changelog; `chore:`,
  `ci:`, `docs:` and `test:` are left out of it.
- **Work lands on the branch directly**, after `just check`; pull requests are
  for people outside the project.
- **A release** is a tag: the checks, the binaries for each target, the
  container image, their checksums and a software bill of materials, in a
  draft release a person publishes.

## The new `AGENTS.md`

Its shape follows Go Dashbin's and TinyStore's:

```
Status            what works today, and nothing that does not
Shape             the repository's table, the library's modules, what may import what
Architecture      the rules of chapters 2 to 12, one paragraph each, as decided
Weight            the 64 MiB promise, the size in CI, the rule on technology
Decided, not built     with the issue or the round each waits for
Editing rules     the ones above
Commits, branches, releases
```

The promises are `CONTRACTS.md`'s and not repeated here. **Skills for the
traps** in `.agents/skills/`: adding a built-in plugin, an api operation, a
migration and a document's shape, a contract and its test, a change to
`protocol/*.wire`.

## Forks

After the first round:

1. **Three crates, a library whose parts are features** — changed from twelve
   crates: a module is a concept, a feature is what may be left out, and a
   crate is what is built or published apart, as in TinyStore.
2. **The protocol written once and generated for Rust and TypeScript** — added:
   two codecs that cannot drift, with shared vectors.
3. **`just`** — changed from `cargo xtask`: the same tool as TinyStore, and CI
   calls cargo itself.
4. **`CONTRACTS.md` with ids, kept by `just contracts`** — changed from a table
   of gates in a chapter: a promise kept by no test fails the build.
5. **`unsafe` denied in the workspace and allowed in `host` alone**, every block
   explained. Kept, in TinyStore's lints.
6. **No library of the system at run time, C compiled in.** Kept.
7. **The 64 MiB promise as a contract run in CI.** Kept.
8. **`rust` becomes `main`** when it runs the first user's ten sites. Kept.
9. **Commits without trailers.** Kept.

## What it asks of other chapters

- Chapter 3: the frame wire written in `protocol/*.wire`, its vectors shared.
- Chapter 8: the call and the agent's greeting written there too.
- Every chapter: its promises as contracts with ids, its gates as tests that
  name them.
- Chapter 12: the scripted load the 64 MiB contract runs.
- Chapter 14: which modules the first version needs first.

## Not checked yet

- The library's build time from clean and after a change, with every feature
  on and with the kernel alone.
- The crates that derive an OpenAPI schema from axum's handlers, by weight and
  by what they read.
- A `.wire` definition for the frame wire and the call that both generated
  codecs pass the vectors of, and what TinyStore's generator needs to write
  Dashbin's.
- Miri over the arena and the decoder, and what it costs a CI run.
