package main

import (
	"context"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/records"
)

func main() {
	probe.Main(consumer.Open(openRecords))
}

func openRecords(ctx context.Context, store *tinystore.Store) error {
	_, err := records.Open(ctx, store, records.Options{})
	return err
}
