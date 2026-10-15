# Verification — sustained WAL round, 15 October 2026

## Local Linux

Product `31b10e441a61f75171601c45111c20d8ba5b65d6`:

- `just check`: default core 828 passed, five ignored; workspace crates and
  30 documentation tests passed. Full local Bun SDK 991 passed, one skipped,
  zero failed. SDK lint/type check passed. Engine feature clippy subsets,
  formatting and protocol generation passed.
- The last Docker actionlint recipe fails because the daemon does not see this
  checkout at its bind-mounted `/repo`. The equivalent local actionlint 1.7.12
  passed. The recipe itself is not described as passing.
- Additional capture-enabled complete core suite: 830 passed, five ignored.
  Capture-enabled clippy with defaults and alone (includes SQL) passed;
  additional kv,sql and kv,jobs combinations passed.
- Admission gate runs both exclusive and grouped writes: refusal before BEGIN
  changes no version; admitting again allows the same writer to resume. Removing
  grouped before_write makes the gate fail. Source was restored and the complete
  capture suite passed before commit.

Research `b09de68`: seven parent tests passed, one ignored child entry point;
`3654372` adds zero-cache recovery, eight parent tests pass. Both exercised all
16 process-exit child scenarios. All-target clippy with warnings denied and
formatting passed. Release builds completed before timed runs. The raw preflight
is excluded from rate findings. Main, component controls and soak all verify
version sums against ACKs, image/feed roots and SQLite integrity_check.

Artifacts: [research tests](verification/research-tests.txt),
[research clippy](verification/research-clippy.txt),
[core capture suite tail](verification/core-capture-tests-tail.txt),
[core clippy](verification/core-clippy.txt),
[admission mutation](verification/admission-mutation.txt),
[default workspace and SDK summary](verification/product-check-summary.txt),
[local actionlint](verification/actionlint.txt).

No new platform claim is made. Experimental native capture/fault/load checks
ran on Linux only. Default-feature CI does not exercise capture mode.

## Product CI remains an open gate

[Run 38754656519](https://github.com/tinyshed/tinystore/actions/runs/38754656519)
for the exact product commit is not green. Linux SDK `events through a sidecar`
and `events through a remote server over TCP` fail their unnamed teardown hook
after 5000 ms. The named event assertions finish successfully first. The
preceding WAL round also observed events teardown timeouts, but attribution
remains unresolved; this round does not declare them pre-existing or unrelated.
Quality, macOS and Windows jobs pass for this commit. Local complete SDK
passes. No test was skipped, weakened or given a longer
timeout to obtain that local result.

[CI job/step result](verification/ci.json) records platform status and URLs.
The failure excerpt is retained in [CI events](verification/ci-events.txt).
