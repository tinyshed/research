if an app forgets `LIBSQLITE3_FLAGS` in `.cargo/config.toml`, tinystore says so once, with `tracing::warn!`, when the first connection opens (`warn_of_a_slow_build` in `sqlite/connection.rs`). problem: if you set up logging the way "your first store" does, that first connection opens **before** anyone is listening:

```rust
let store = Store::open("data", Options::default())?;
let log = store.logger("hello").layer()?;        // Records::of opens records.db here -> the warning fires, no subscriber yet
tracing_subscriber::registry().with(log).init(); // listening starts here
```

and it's a `std::sync::Once`, so it never fires again. net result: nobody ever sees it. the docs example even opens a bucket before `init()`, same thing.

## repro

tinystore 0.1.0 from crates.io, rust 1.97.1, windows 11, **no** `.cargo/config.toml`.

```rust
use tinystore::{Options, Store};
use tracing_subscriber::prelude::*;

fn main() -> tinystore::Result<()> {
    let listen_first = std::env::args().any(|a| a == "--listen-first");
    let dir = std::env::temp_dir().join("ts-repro-warning");
    let _ = std::fs::remove_dir_all(&dir);

    if listen_first {
        tracing_subscriber::fmt().with_target(true).init();
        let store = Store::open(&dir, Options::default())?;
        let users = store.bucket::<String>("users").open()?;
        users.set("42", &"ada".to_string())?;
        tracing::info!("bucket written");
        return store.close();
    }

    // the order from "your first store"
    let store = Store::open(&dir, Options::default())?;
    let log = store.logger("hello").layer()?;
    tracing_subscriber::registry().with(log).init();
    let users = store.bucket::<String>("users").open()?;
    users.set("42", &"ada".to_string())?;
    tracing::info!("bucket written");
    store.close()
}
```

docs order, output:

```
{"time":"2026-10-23T00:02:20.870Z","level":"INFO","stream":"hello","msg":"bucket written"}
```

`--listen-first`, output:

```
WARN tinystore: SQLite was built with SQLITE_ENABLE_MEMORY_MANAGEMENT, so every connection shares one page cache behind one mutex and readers stop one another: build with LIBSQLITE3_FLAGS="SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT SQLITE_ENABLE_DBPAGE_VTAB"
INFO warning: bucket written
```

the second warning in the same function ("SQLite had started before the store opened...") has the same problem.

## what i'd like (any of these)

- make it loud: `Store::open` fails `invalid` unless the app opts out explicitly (an `Options` flag or a cargo feature). impossible to miss.
- or keep it a warning but don't lose it: replay it into the store's own logger once one is attached, and/or show it in `tinystore status`.

## also, fyi: couldn't measure what the flags buy

tried to see the cost and didn't. 8 threads x 400 `select sum(v) from t where x % 7 = 3` over a 200k-row table, two runs each:

| build | 1 thread | 8 threads |
|---|---|---|
| default (no flags) | 210 / 211 reads/s | 1325 / 1320 reads/s |
| with `LIBSQLITE3_FLAGS` | 220 / 215 reads/s | 1400 / 1108 reads/s |

so either my workload doesn't hit the shared page cache mutex, or it's mostly a linux thing. would be good if the warning or the docs pointed at a workload where the difference shows, so people know what they lose. and if it turns out small, it still shouldn't be a message nobody can see.
