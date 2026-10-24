# Dashbin research

What the Rust rewrite of [tinyshed/dashbin](https://github.com/tinyshed/dashbin)
was decided on, before its repository was started. Nothing here is Dashbin;
what a round proved lands there as its own commit.

| | |
|---|---|
| [reports/](reports/README.md) | the dated rounds, each with its environment and reproduction command, raw output in `reports/data/` |
| [tinystore-repro/](tinystore-repro/) | one program a TinyStore issue, against 0.1.0 and again against 0.1.1 |
| [memory-probe/](memory-probe/) | the hub, QuickJS, the agent and a JS plugin process, measured under four allocators |
| [sdk-probe/](sdk-probe/) | SDK bundles, the mock they call, and plugins written to leave a timer, a request or a promise behind, or to reach where they may not |
| [plugin-host/](plugin-host/) | the host: rquickjs, `llrt_modules`, timers of its own, and `vendor/llrt_fetch` with a policy per instance |

## Running the code

Everything heavy runs in a Linux container, as [AGENTS.md](../AGENTS.md) says.
`memory-probe/docker/Dockerfile` is the build image (`rust:1.99-bookworm` with
`musl-tools` and `cmake`); each report has the exact commands. What a build
produces is not kept: `target/`, `node_modules/`, `dist/` and LLRT's binary
come back from the commands, with versions pinned by the lock files.

`plugin-host/vendor/llrt_fetch` is LLRT's crate under Apache-2.0 with its
`LICENSE` and `NOTICE`; `UPSTREAM` names the version and every file Dashbin
changed.
