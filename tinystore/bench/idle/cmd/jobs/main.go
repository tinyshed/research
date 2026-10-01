package main

import (
	"context"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/jobs"
)

func main() {
	probe.Main(consumer.Open(openJobs))
}

func openJobs(ctx context.Context, store *tinystore.Store) error {
	queues, err := jobs.Open(ctx, store, jobs.Options{})
	if err != nil {
		return err
	}
	_, err = jobs.OpenQueue[[]byte](ctx, queues, "work")
	return err
}
