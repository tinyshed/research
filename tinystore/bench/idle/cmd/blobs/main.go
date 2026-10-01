package main

import (
	"context"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/blobs"
)

func main() {
	probe.Main(consumer.Open(openBlobs))
}

func openBlobs(ctx context.Context, store *tinystore.Store) error {
	files, err := blobs.Open(ctx, store, blobs.Options{})
	if err != nil {
		return err
	}
	_, err = blobs.OpenBucket(ctx, files, "files")
	return err
}
