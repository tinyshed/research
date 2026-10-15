# Verification — WAL round, 15 October 2026

## Local Linux checks

- Product `690cdf3`: workspace clippy and engine feature subsets from `just
  check`; additional kv,sql and kv,jobs; capture-experiment with defaults and
  alone (it includes SQL); formatting and protocol generation check pass.
- Default library: 828 passed, 5 ignored. Other workspace crates and 30
  documentation tests pass. Capture-enabled library: 829 passed, 5 ignored.
- Complete local Bun SDK: 991 passed, 1 skipped, 0 failed; SDK lint/type check
  pass. Bun 1.4.2. Biome emits informational suggestions, no errors.
- `just check` itself exits nonzero at its final Docker actionlint wrapper:
  the daemon's bind mount does not expose this checkout at `/repo`. Running
  the same actionlint 1.7.12 locally passes. This is an equivalent check, not
  a claim that the Docker recipe passed. No separate nested Linux container
  suite was claimed: the current environment is already Linux.
- Research final verification `5c88ca9`: 16 parent tests passed, 3 ignored
  child entry points, all 33 controlled abrupt workers exercised. Clippy
  all-targets with -D warnings and formatting pass. Research uses SQLite
  dbpage + HAVE_FDATASYNC=1; the ordinary product suite uses its workspace
  configuration. Release binaries built before timing.
- The capture-before-ACK gate was mutation-tested: removing the commit
  observation makes it fail; restored source passes and is what was committed.
- Measured runs: capture 48 cases, network 48, grouped 18, pipeline 27, threshold
  control 18 (159 cases), each with three ordered passes. Every source capture case checks its final root;
  every replica checks final canonical root and SQLite integrity_check.

Raw proof: [tests](tests.txt), [clippy](clippy.txt),
[core capture suite](core-capture-tests-tail.txt),
[core gate restored](capture-gate.txt), [gate mutation](capture-gate-mutation.txt).

## CI is not green

[Product CI](https://github.com/tinyshed/tinystore/actions/runs/38742104053)
passes quality and Windows. On the first attempt Linux and macOS each fail
with 5-second SDK `events` shutdown-hook timeouts for the private child and
TCP remote. All named events assertions pass before the shutdown hook.
One rerun of failed jobs still fails the private-child hook on both platforms;
the TCP hook passes then. Rust build/lint/tests in those jobs passed.

After measurement, the exact 13-file CI SDK command was run locally three
times against the default-feature product binaries. All three pass with
839 tests, 1 skipped, 0 failed (5.46; 5.52; 5.58 seconds). The full local suite
also passes. The baseline Rust commit has an earlier green CI run, so this
round does not assert that the timeout is pre-existing. Attribution and the
CI-specific shutdown behavior remain unresolved. Capture is disabled in these
default builds, but that fact alone does not establish the cause.

No test was skipped, weakened or given a larger timeout to change this result.
This is an open integration/release gate; it does not invalidate the standalone
WAL measurements, and it prevents describing the product branch as fully
verified. [Structured CI result](ci.json).

Exact local CI reproduction (from `<product>/sdk/js`, with the release library
and executable selected through TINYSTORE_LIBRARY and TINYSTORE_BIN):

```sh
bun test test/pipe.test.ts test/kv.test.ts test/jobs.test.ts test/sql.test.ts \
  test/queries.test.ts test/clock.test.ts test/direct.test.ts \
  test/durability.test.ts test/protocol.test.ts test/console.test.ts \
  test/events.test.ts test/logger.test.ts test/background.test.ts
```

Native WAL capture, directory-sync behavior and fault scenarios were not run
on macOS or Windows. Their default-feature CI results are not evidence for
the experimental capture mode on those platforms.
