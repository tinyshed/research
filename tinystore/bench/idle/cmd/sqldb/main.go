package main

import (
	"context"
	"testing/fstest"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
	"github.com/tinyshed/tinystore/sqldb"
)

func main() {
	probe.Main(consumer.Open(openSQL))
}

func openSQL(ctx context.Context, store *tinystore.Store) error {
	migrations := fstest.MapFS{"0001.sql": {Data: []byte("create table note (id integer primary key) strict;")}}
	_, err := sqldb.Open(ctx, store, "app", migrations, nil)
	return err
}
