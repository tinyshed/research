package main

import (
	"context"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/sqldb"
)

func main() {
	probe.Main(consumer.Open(openSQL))
}

func openSQL(ctx context.Context, store *tinystore.Store) error {
	_, err := sqldb.Open(ctx, store, "app", nil, nil)
	return err
}
