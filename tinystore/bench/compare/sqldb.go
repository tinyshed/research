package main

import (
	"context"
	"fmt"
	"sync"
	"sync/atomic"
)

// sqlStore is what every SQL contender does with one table of notes: a read
// by id, an insert and an update, each write committed and synced before it
// returns.
type sqlStore interface {
	subject
	getNote(ctx context.Context, id int64) (note, error)
	insertNote(ctx context.Context, n note) error
	updateNote(ctx context.Context, id int64, body string) error
}

// note is one row, as sqldb maps its fields to columns by name.
type note struct {
	ID    int64
	Title string
	Body  string
}

const (
	sqlNotes     = 100_000
	sqlBodyBytes = 256
)

// The one schema every contender creates; Postgres spells its key otherwise.
const notesSchema = `create table if not exists note (
	id    integer primary key,
	title text not null,
	body  text not null
) strict`

var sqlEngine = engine{
	order: []string{"tinystore", "sqlite", "ncruces", "postgres"},
	contenders: map[string]opener{
		"tinystore": openTinyStoreSQL,
		"sqlite":    openSQLiteSQL,
		"ncruces":   openNcrucesSQL,
		"postgres":  openPostgresSQL,
	},
	measure: measureSQL,
}

// measureSQL fills the table, then times reads by id, inserts of new notes and
// a mix of nine reads to one update, each from 1, 8 and 64 goroutines.
func measureSQL(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	store, ok := s.(sqlStore)
	if !ok {
		return nil, fmt.Errorf("%T is not an SQL store", s)
	}
	if err := fillNotes(ctx, store); err != nil {
		return nil, fmt.Errorf("fill: %w", err)
	}

	next := atomic.Int64{} // ids for inserts, past the ones the fill wrote
	next.Store(sqlNotes)
	var stages []stage
	for _, load := range []struct {
		name string
		op   operation
	}{
		{"get", sqlGet(store)},
		{"insert", sqlInsert(store, &next)},
		{"mixed", sqlMixed(store)},
	} {
		for _, goroutines := range []int{1, 8, 64} {
			stages = append(stages, timeStage(ctx, load.name, goroutines, seconds, load.op))
		}
	}
	return stages, nil
}

func fillNotes(ctx context.Context, store sqlStore) error {
	const writers = 64
	errs := make([]error, writers)
	var wg sync.WaitGroup
	for w := range writers {
		wg.Go(func() {
			for i := w; i < sqlNotes && errs[w] == nil; i += writers {
				errs[w] = store.insertNote(ctx, makeNote(int64(i+1), 0))
			}
		})
	}
	wg.Wait()
	for _, err := range errs {
		if err != nil {
			return err
		}
	}
	return nil
}

func sqlGet(store sqlStore) operation {
	return func(ctx context.Context, worker, n int) error {
		id := int64(pick(worker, n, 11)%sqlNotes) + 1
		got, err := store.getNote(ctx, id)
		if err == nil && got.ID != id {
			err = fmt.Errorf("note %d came back as %d", id, got.ID)
		}
		return err
	}
}

func sqlInsert(store sqlStore, next *atomic.Int64) operation {
	return func(ctx context.Context, _, n int) error {
		return store.insertNote(ctx, makeNote(next.Add(1), n))
	}
}

func sqlMixed(store sqlStore) operation {
	get := sqlGet(store)
	return func(ctx context.Context, worker, n int) error {
		if pick(worker, n, 12)%10 != 0 {
			return get(ctx, worker, n)
		}
		id := int64(pick(worker, n, 13)%sqlNotes) + 1
		return store.updateNote(ctx, id, makeNote(id, n).Body)
	}
}

// makeNote is a note whose body changes with each version and does not
// compress to nothing
func makeNote(id int64, version int) note {
	body := kvValue(int(id), version)
	text := make([]byte, 0, sqlBodyBytes)
	for len(text) < sqlBodyBytes {
		text = fmt.Appendf(text, "%x", body)
	}
	return note{ID: id, Title: fmt.Sprintf("note %d", id), Body: string(text[:sqlBodyBytes])}
}
