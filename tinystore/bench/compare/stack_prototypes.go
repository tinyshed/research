package main

import (
	"context"
	"sync"
	"time"

	"github.com/tinyshed/tinystore/sqldb"
)

// Designs TinyStore does not have, measured on the stack's requests before one
// is built. Neither is an engine: the numbers say what building one could win.
//
//	tinystore-outbox    a note's job kept in the note's own database, written
//	                    by a trigger of the update's statement: one commit a
//	                    write request, as a queue in the application's
//	                    database would give; a worker in the process leases
//	                    and deletes them, stackJobBatch a write, as the
//	                    services' worker does
//	tinystore-parallel  the note's update and its job's enqueue sent at once,
//	                    the request waiting for both commits together instead
//	                    of one after the other
func init() {
	stackEngine.contenders["tinystore-outbox"] = openTinyStoreOutboxApp
	stackEngine.contenders["tinystore-parallel"] = openTinyStoreParallelApp
}

const outboxSchema = `create table if not exists outbox (
	id    integer primary key,
	note  integer not null,
	lease integer
) strict;
create index if not exists outbox_free on outbox (id) where lease is null;
create trigger if not exists note_outbox after update of body on note begin
	insert into outbox (note) values (new.id);
end;
`

const (
	outboxLease = `update outbox set lease = ?1 where id in
		(select id from outbox where lease is null order by id limit ?2) returning id`
	outboxDone = `delete from outbox where lease = ?1`
)

func openTinyStoreOutboxApp(ctx context.Context, dir string) (subject, error) {
	app, err := openTinyStoreAppWith(ctx, dir, notesSchema+";\n"+outboxSchema, inJobsDB)
	if err != nil {
		return nil, err
	}
	app.write = func(ctx context.Context, r appRequest) error {
		_, err := app.db.Exec(ctx, notesUpdate, makeNote(int64(r.note), r.n).Body, r.note)
		return err
	}
	app.running.Go(func() { drainOutbox(app.background, app) })
	return app, nil
}

// drainOutbox leases the jobs the outbox holds and deletes them once handled,
// each lease a token of its own
func drainOutbox(ctx context.Context, app *tinyStoreApp) {
	for token := int64(1); ctx.Err() == nil; token++ {
		rows, err := sqldb.ExecQuery(ctx, app.db, outboxLease, token, stackJobBatch)
		leased := len(rows.Values)
		if err == nil && leased > 0 {
			if _, err = app.db.Exec(ctx, outboxDone, token); err == nil {
				app.handled.Add(int64(leased))
			}
		}
		if err != nil || leased == 0 {
			select {
			case <-ctx.Done():
			case <-time.After(20 * time.Millisecond):
			}
		}
	}
}

func openTinyStoreParallelApp(ctx context.Context, dir string) (subject, error) {
	app, err := openTinyStoreAppWith(ctx, dir, notesSchema+";\n", inJobsDB)
	if err != nil {
		return nil, err
	}
	app.write = func(ctx context.Context, r appRequest) error {
		var wg sync.WaitGroup
		var enqueued error
		wg.Go(func() { enqueued = app.queue.Enqueue(ctx, job{N: r.note, Text: "index the note"}) })
		_, updated := app.db.Exec(ctx, notesUpdate, makeNote(int64(r.note), r.n).Body, r.note)
		wg.Wait()
		if updated != nil {
			return updated
		}
		return enqueued
	}
	return app, nil
}
