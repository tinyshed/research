# Rust research source notices

The TinyStore-derived code in `rust-spike`, `sqlite-bench`, `metrics-bench`,
`metrics-opt-bench`, `metrics-max-bench`, `records-native-bench` and
`kv-native-bench`, `records-opt-bench`, `kv-opt-bench`, `metrics-layout-bench`
and `metrics-payload-bench` follow TinyStore's Apache License
2.0; a copy is provided in [RUST-RESEARCH-LICENSE](RUST-RESEARCH-LICENSE).
The upstream TinyStore source remains in the pinned `source` submodule.

The Rust Unicode print tables in each metrics prototype's `query/quote.rs`
are translated from Go 1.27.1 `strconv/isprint.go`; the Go Authors' copyright,
BSD conditions and disclaimer are retained in those source files.

Third-party crates and the SQLite/zstd sources are fetched by the locked
builds, not vendored here. Their own licenses apply. SQLite's official public
domain amalgamation is fetched by URL and checked against a pinned SHA-256.
The bundled-linkage smoke in `sqlite-rust-spike` documents its different
SQLite version and is retained only as configuration/linkage evidence.
