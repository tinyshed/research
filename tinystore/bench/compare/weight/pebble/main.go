// Command pebble opens a store, writes a key and reads it back.
package main

import (
	"os"

	"github.com/cockroachdb/pebble/v2"
)

func main() {
	db, err := pebble.Open(os.Args[1], &pebble.Options{})
	if err != nil {
		panic(err)
	}
	defer db.Close()
	if err = db.Set([]byte("k"), []byte("v"), pebble.Sync); err != nil {
		panic(err)
	}
	value, closer, err := db.Get([]byte("k"))
	if err != nil {
		panic(err)
	}
	_, _ = os.Stdout.Write(value)
	_ = closer.Close()
}
