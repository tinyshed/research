// Command bbolt opens a bucket, writes a key and reads it back.
package main

import (
	"os"

	bolt "go.etcd.io/bbolt"
)

func main() {
	db, err := bolt.Open(os.Args[1], 0o600, nil)
	if err != nil {
		panic(err)
	}
	defer db.Close()
	err = db.Update(func(tx *bolt.Tx) error {
		b, err := tx.CreateBucketIfNotExists([]byte("kv"))
		if err != nil {
			return err
		}
		return b.Put([]byte("k"), []byte("v"))
	})
	if err != nil {
		panic(err)
	}
	_ = db.View(func(tx *bolt.Tx) error {
		_, _ = os.Stdout.Write(tx.Bucket([]byte("kv")).Get([]byte("k")))
		return nil
	})
}
