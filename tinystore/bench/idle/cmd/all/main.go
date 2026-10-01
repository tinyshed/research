package main

import (
	"context"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/blobs"
	"github.com/tinyshed/tinystore/jobs"
	"github.com/tinyshed/tinystore/kv"
	"github.com/tinyshed/tinystore/metrics"
	"github.com/tinyshed/tinystore/records"
	"github.com/tinyshed/tinystore/sqldb"
)

func main() {
	probe.Main(consumer.Open(openKV, openSQL, openJobs, openBlobs, openRecords, openMetrics))
}

func openKV(ctx context.Context, store *tinystore.Store) error {
	state, err := kv.Open(ctx, store, kv.Options{})
	if err != nil {
		return err
	}
	_, err = kv.OpenBucket[[]byte](ctx, state, "sessions")
	return err
}

func openSQL(ctx context.Context, store *tinystore.Store) error {
	_, err := sqldb.Open(ctx, store, "app", nil, nil)
	return err
}

func openJobs(ctx context.Context, store *tinystore.Store) error {
	queues, err := jobs.Open(ctx, store, jobs.Options{})
	if err != nil {
		return err
	}
	_, err = jobs.OpenQueue[[]byte](ctx, queues, "work")
	return err
}

func openBlobs(ctx context.Context, store *tinystore.Store) error {
	files, err := blobs.Open(ctx, store, blobs.Options{})
	if err != nil {
		return err
	}
	_, err = blobs.OpenBucket(ctx, files, "files")
	return err
}

func openRecords(ctx context.Context, store *tinystore.Store) error {
	_, err := records.Open(ctx, store, records.Options{})
	return err
}

func openMetrics(ctx context.Context, store *tinystore.Store) error {
	_, err := metrics.Open(ctx, store, metrics.Options{})
	return err
}
