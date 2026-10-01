package main

import (
	"context"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/kv"
)

func main() {
	probe.Main(consumer.Open(openKV))
}

func openKV(ctx context.Context, store *tinystore.Store) error {
	state, err := kv.Open(ctx, store, kv.Options{})
	if err != nil {
		return err
	}
	_, err = kv.OpenBucket[[]byte](ctx, state, "sessions")
	return err
}
