// Command badger opens a store, writes a key and reads it back.
package main

import (
	"os"

	"github.com/dgraph-io/badger/v4"
)

func main() {
	db, err := badger.Open(badger.DefaultOptions(os.Args[1]).WithSyncWrites(true))
	if err != nil {
		panic(err)
	}
	defer db.Close()
	if err = db.Update(func(txn *badger.Txn) error { return txn.Set([]byte("k"), []byte("v")) }); err != nil {
		panic(err)
	}
	_ = db.View(func(txn *badger.Txn) error {
		item, err := txn.Get([]byte("k"))
		if err != nil {
			return err
		}
		value, err := item.ValueCopy(nil)
		_, _ = os.Stdout.Write(value)
		return err
	})
}
