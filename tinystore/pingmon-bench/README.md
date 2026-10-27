# Pingmon replayed

Pingmon pings six hosts every second and writes two gauges a host,
`ping_rtt_ms` and `ping_lost`, into TinyStore's metrics through the Python
SDK's sidecar: 10 s of samples an ingest, maintenance every minute. Its
times are when each answer came, `ping -D`, so they lie a few milliseconds
either side of their step.

`pingmon-bench` replays a CSV of `name,target,ms,value` into a store as
Pingmon wrote it, on a clock that follows the data, and `division` divides
the file by b-tree with `dbstat`.

The corpus is private: the owner's own Pingmon store, read through its
running server by `dump.py`, and never committed.

```sh
python3 dump.py <pingmon store> > <corpus>/pingmon.csv # on the Pingmon host, read-only
git -C ../source checkout <commit>
cargo build --release
target/release/pingmon-bench <corpus>/pingmon.csv <store>
target/release/division <store>/metrics.db <samples>
target/release/aggregate <store> <another store>
```
