package main

import (
	"context"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/metrics"
)

func main() {
	probe.Main(consumer.Open(openMetrics))
}

func openMetrics(ctx context.Context, store *tinystore.Store) error {
	_, err := metrics.Open(ctx, store, metrics.Options{})
	return err
}
