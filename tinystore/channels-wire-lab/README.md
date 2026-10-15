# Core/Wire/Bun research adapter

No production module or public API is changed. This adapter uses unmodified
TinyStore `Pipe` for HELLO/WELCOME and ordinary protocol traffic. Experimental
private methods 60000–60005 attach a bounded channel to that boundary. Message
delivery uses unchanged Bun SDK `Session`, `Connection`, frame parsing, stream
cancellation and CREDIT; Unix/TCP use the unchanged Bun runtime transport.
Embedded uses a research FFI wrapper around the same adapter.

**This is not a native Channels route in Core.** Its event queue, post-commit
dispatch and output scheduling belong to the research adapter. It serializes
Core KV commit completion before assigning channel order; it does not measure a
production grouped-commit notification hook. The private binary event envelope
is minimal: generated Channels MessagePack/JSON codecs do not exist and their
future cost is not included. The per-connection polling driver
is also an explicit prototype cost, not a recommended implementation.

Both algorithms retain 256 shared `Arc` payloads. The shared log is a simple
mutex/VecDeque with subscriber cursors, not a new lock-free structure. The
baseline is `tokio::sync::broadcast`. Both use identical framing and scheduling.
Payloads are 1024 bytes. Fast offered rate is 200/s. Slow runs voluntarily grant
zero initial send allowance to 1% of subscriptions, publish 1200 events (above
retention), then resume with real Wire CREDIT. This tests withheld credit
without filling every SDK's normal 2 MiB initial window; it is not a measured
slow socket bandwidth profile. Standalone publishes stay in memory. The separate `commit` profile follows an
actual Core KV FULL commit per event; ten rolled-back Core transactions in every
profile must produce no events. `burst` keeps 32 publisher calls in flight.
`slow-window` leaves 1% of real SDK iterators unread until their normal 2 MiB
Wire window exhausts, then resumes them. Idle CPU is sampled over 200 ms.

Build with Rust 1.99 and run with Bun 1.4.2, two CPU quota and six GiB memory.
Use a disk-backed scratch volume. `SDK_ROOT` is an unchanged TinyStore checkout
at the pinned revision. `LAB_LIB` points to `libchannels_wire_lab.so`.

```sh
cargo build --release
SDK_ROOT=<tinystore>/sdk/js LAB_LIB=<target>/release/libchannels_wire_lab.so \
  BENCH_DB=<scratch>/store bun client.ts embedded broadcast 1000 1 slow
```

For sidecar/TCP start `channels-wire-lab <store> broadcast unix <socket>` or
`channels-wire-lab <store> broadcast tcp unused`, then select `unix`/`tcp` in
the client. The sidecar experiment measures a Unix endpoint of the adapter;
production SERVE discovery/authentication/startup are not included. Server
memory and wire counts come from the adapter's stats; client memory/CPU from
Bun. `run.py` additionally records the daemon's CPU from `/proc/<pid>/stat`.

The sum of bounded-log retention, Wire allowances, native transport queues,
SDK frame copies and application-held messages is the memory budget. A shared
log alone cannot bound all of these. Search-pattern routing is not benchmarked;
namespace matching, authorization and subscriber limits remain API decisions.
