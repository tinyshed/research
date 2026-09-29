// Command tinystore opens a kv bucket, writes a key and reads it back.
package main

import (
	"context"
	"os"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/kv"
)

func main() {
	ctx := context.Background()
	store, err := tinystore.Open(ctx, os.Args[1], tinystore.Options{})
	if err != nil {
		panic(err)
	}
	defer store.Close(ctx)
	state, err := kv.Open(ctx, store, kv.Options{})
	if err != nil {
		panic(err)
	}
	bucket, err := kv.OpenBucket[[]byte](ctx, state, "sessions")
	if err != nil {
		panic(err)
	}
	if err = bucket.Set(ctx, "k", []byte("v")); err != nil {
		panic(err)
	}
	value, _, err := bucket.Get(ctx, "k")
	if err != nil {
		panic(err)
	}
	_, _ = os.Stdout.Write(value)
}
