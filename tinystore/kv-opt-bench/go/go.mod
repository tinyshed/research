module github.com/tinyshed/tinystore/kv-opt-bench

go 1.27.0

toolchain go1.27.1

require (
	github.com/ncruces/go-sqlite3 v0.35.6
	github.com/tinyshed/tinystore v0.0.0
)

require (
	github.com/ncruces/go-sqlite3-wasm/v6 v6.3.35304 // indirect
	github.com/ncruces/julianday v1.0.0 // indirect
	golang.org/x/sys v0.48.0 // indirect
)

replace github.com/tinyshed/tinystore => ../../source
