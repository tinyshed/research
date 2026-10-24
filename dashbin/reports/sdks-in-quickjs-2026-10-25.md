# Real SDKs inside QuickJS, 25 October 2026

The plugin model gives authors ordinary JavaScript: `fetch`, `process.env`,
an npm SDK bundled into one file. That only holds if the SDKs people reach for
run in QuickJS with web APIs implemented in Rust. LLRT, AWS's runtime on
QuickJS and rquickjs, was the stand-in for the host Dashbin would build, with
Bun as the control.

| Question | Answer | What follows |
|---|---|---|
| Do popular SDKs run? | Stripe 23, Octokit, OpenAI, Notion, papaparse + zod + date-fns: yes, each with a real call against a mock | fetch-based SDKs are the normal case |
| Which do not? | `@slack/web-api` (axios and pieces of Node) and `pg` (TCP and TLS from Node) | a process plugin, or a built-in for databases |
| Can that be caught early? | `bun build --target=browser` already refuses `pg` on its first line | the plugin build refuses Node built-ins and smoke-tests in Dashbin's runtime |
| What does a real plugin cost? | 2–5 MiB over the runtime as bytecode, not the 0.2 MiB of a toy: Stripe +3.9, OpenAI +5.0, Notion +7.5 as a browser bundle and +2.0 as a node one | ship bytecode, bundle without browser polyfills |
| And as a Bun process? | 32–41 MiB each | — |

Memory inside Dashbin's own host, rather than in LLRT's process, is in
[plugin-host-2026-10-25](plugin-host-2026-10-25.md), which supersedes the
memory column here.

## Environment and reproduction

- LLRT v0.9.0-beta, `llrt-linux-x64-no-sdk.zip` from its GitHub release, run in
  `rust:1.99-bookworm`; Bun 1.4.2 (`oven/bun:1.4.2-slim`) as the control.
  Docker Desktop, kernel 6.18.33.2-microsoft-standard-WSL2, 16 vCPU.
- SDK versions as locked in `sdk-probe/bun.lock`: stripe 23.0.1,
  @octokit/rest 22.0.1, openai 7.43.0, @notionhq/client 5.28.0,
  @slack/web-api 8.2.0, pg 8.23.1, papaparse 5.7.0, zod 4.6.7, date-fns 4.4.0.
- Stripe talks to `stripe/stripe-mock@sha256:aeb96b83…`; the others to
  `sdk-probe/mock.mjs`, which answers each call the tests make; `pg` to
  `postgres:18-alpine`. A test passes when the SDK sent a request and parsed
  the answer.
- Memory is the bundle's own footer reading `/proc/self/status` after its call
  (`tests/_footer.txt`, appended to every test). One pass each.
- The harness was not committed before it ran; it is committed afterwards.

```sh
cd <research>/dashbin/sdk-probe
bun install
for t in empty stripe octokit openai notion slack pg libs; do
  bun build tests/$t.mjs --target=browser --format=esm --conditions=worker --outfile dist/web/$t.mjs
  bun build tests/$t.mjs --target=node --format=esm --outfile dist/node/$t.mjs
  bun build tests/$t.mjs --target=browser --format=esm --conditions=worker --minify --outfile dist/min/$t.mjs
done
mkdir -p llrt && curl -L -o llrt/llrt.zip https://github.com/awslabs/llrt/releases/download/v0.9.0-beta/llrt-linux-x64-no-sdk.zip && (cd llrt && unzip llrt.zip)
bash run.sh                                  # 01: every bundle in LLRT and in Bun
# 02: llrt compile dist/min/<t>.mjs dist/lrt/<t>.lrt, then each of web, min and lrt in LLRT
```

Raw output: [data/sdks-in-quickjs-2026-10-25/](data/sdks-in-quickjs-2026-10-25/).

## Which SDKs run (01)

| | LLRT, browser bundle | LLRT, node bundle | Bun |
|---|---|---|---|
| stripe 23 | ok: list with auto-pagination, refund with an idempotency key | `Error resolving module 'http'` | ok |
| @octokit/rest | ok | ok | ok |
| openai | ok | ok | ok |
| @notionhq/client | ok | ok | ok |
| @slack/web-api | `TypeError: Path must be a string` | `Error converting from js 'undefined' into type 'string'` | ok |
| pg | does not bundle: `Browser build cannot require() Node.js builtin: "tls"` | `Error resolving module 'util/types'` | ok |
| papaparse + zod + date-fns | ok | ok | ok |

## What a plugin costs (02)

Over the empty runtime, 7.4 MiB (7.7 for bytecode), one process each:

| | as bundled | minified | bytecode |
|---|---|---|---|
| Stripe | +5.9 | +4.5 | +3.9 |
| OpenAI | +9.2 | +7.1 | +5.0 |
| Notion, browser bundle | +12.0 | +9.7 | +7.5 |
| Notion, node bundle | +2.3 | +2.1 | +2.0 |
| Octokit | +2.9 | +2.8 | +2.6 |
| papaparse + zod + date-fns | +2.8 | +2.4 | +1.6 |

The browser bundle of Notion carries Bun's polyfills for Node built-ins, a
megabyte of code its node bundle does not need.

## What follows

The host Dashbin would build was measured next, with `llrt_modules` inside it:
[plugin-host-2026-10-25](plugin-host-2026-10-25.md).
