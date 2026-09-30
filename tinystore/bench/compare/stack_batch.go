//go:build batchapi

package main

import (
	"context"

	"github.com/tinyshed/tinystore/jobs"
	"github.com/tinyshed/tinystore/sqldb"
)

// tinystore-batch is the application on the design the outbox prototype
// measured, as TinyStore builds it: the queue kept In the application's
// database, and a write request's note and job in one sqldb Batch, one commit.
// It needs a TinyStore with Batch and jobs.Options.In, the tag batchapi.
func init() {
	stackEngine.contenders["tinystore-batch"] = openTinyStoreBatchApp
}

func inTheDatabase(db *sqldb.DB) jobs.Options { return jobs.Options{In: db} }

func openTinyStoreBatchApp(ctx context.Context, dir string) (subject, error) {
	app, err := openTinyStoreAppWith(ctx, dir, notesSchema+";\n", inTheDatabase)
	if err != nil {
		return nil, err
	}
	app.write = func(ctx context.Context, r appRequest) error {
		return app.db.Batch(ctx, func(b *sqldb.Batch) error {
			b.Exec(notesUpdate, makeNote(int64(r.note), r.n).Body, r.note)
			b.Add(app.queue.Enqueued(ctx, job{N: r.note, Text: "index the note"}))
			return nil
		})
	}
	return app, nil
}
