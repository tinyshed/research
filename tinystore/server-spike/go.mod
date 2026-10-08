module github.com/tinyshed/tinystore/server/spike

go 1.27.0

require (
	github.com/tinyshed/tinystore v0.0.0
	github.com/tinyshed/tinystore/server v0.0.0
)

require (
	github.com/klauspost/compress v1.20.1 // indirect
	github.com/ncruces/go-sqlite3 v0.35.6 // indirect
	github.com/ncruces/go-sqlite3-wasm/v6 v6.3.35304 // indirect
	github.com/ncruces/julianday v1.0.0 // indirect
	golang.org/x/sys v0.48.0 // indirect
)

replace (
	github.com/tinyshed/tinystore => ../source
	github.com/tinyshed/tinystore/server => ../source/server
)
