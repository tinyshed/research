package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"strconv"

	"github.com/tinyshed/tinystore/blobs"
	"github.com/tinyshed/tinystore/sqldb"
)

// stack-steps takes a request of the stack round apart: each of its steps
// alone, from 64 goroutines, embedded TinyStore beside the services, so that
// the step a whole request's rate is lost in shows by itself.

type stepper interface {
	appStore
	steps() []step
}

type step struct {
	name string
	op   operation
}

func init() {
	engines["stack-steps"] = engine{order: []string{"tinystore", "services"}, contenders: stackEngine.contenders,
		measure: func(ctx context.Context, s subject, seconds float64) ([]stage, error) {
			app, ok := s.(stepper)
			if !ok {
				return nil, fmt.Errorf("%T has no steps", s)
			}
			if err := app.seed(ctx); err != nil {
				return nil, fmt.Errorf("seed: %w", err)
			}
			var stages []stage
			for _, st := range app.steps() {
				stages = append(stages, counted(ctx, app, func() stage {
					return timeStage(ctx, st.name, 64, seconds, st.op)
				}))
			}
			return stages, nil
		}}
}

func stepNote(worker, n int) int { return pick(worker, n, 52)%appNotes + 1 }

func (a *tinyStoreApp) steps() []step {
	return []step{
		{"session", func(ctx context.Context, worker, n int) error {
			key := sessionKey(pick(worker, n, 53) % appUsers)
			_, found, err := a.sessions.Get(ctx, key)
			if err == nil {
				err = errNotFound(key, found)
			}
			return err
		}},
		{"read-note", func(ctx context.Context, worker, n int) error {
			id := stepNote(worker, n)
			_, found, err := sqldb.One[note](ctx, a.db, notesGet, id)
			if err == nil {
				err = errNotFound("note "+strconv.Itoa(id), found)
			}
			return err
		}},
		{"update-note", func(ctx context.Context, worker, n int) error {
			id := stepNote(worker, n)
			_, err := a.db.Exec(ctx, notesUpdate, makeNote(int64(id), n).Body, id)
			return err
		}},
		{"enqueue", func(ctx context.Context, _, n int) error {
			err := a.queue.Enqueue(ctx, job{N: n, Text: "index the note"})
			if err == nil {
				a.enqueued.Add(1)
			}
			return err
		}},
		{"write", func(ctx context.Context, worker, n int) error {
			err := a.write(ctx, appRequest{note: stepNote(worker, n), n: n})
			if err == nil {
				a.enqueued.Add(1)
			}
			return err
		}},
		{"upload", func(ctx context.Context, worker, n int) error {
			_, err := a.files.Put(ctx, fmt.Sprintf("steps/%d-%d", worker, n), bytes.NewReader(attachment),
				blobs.Size(int64(len(attachment))))
			return err
		}},
		{"log", func(ctx context.Context, worker, n int) error {
			a.log.InfoContext(ctx, "request", "user", worker, "note", n, "write", false)
			return nil
		}},
	}
}

func (a *servicesApp) steps() []step {
	return []step{
		{"session", func(ctx context.Context, worker, n int) error {
			return a.redis.Get(ctx, sessionKey(pick(worker, n, 53)%appUsers)).Err()
		}},
		{"read-note", func(ctx context.Context, worker, n int) error {
			_, err := a.pg.getNote(ctx, int64(stepNote(worker, n)))
			return err
		}},
		{"update-note", func(ctx context.Context, worker, n int) error {
			id := stepNote(worker, n)
			return a.pg.updateNote(ctx, int64(id), makeNote(int64(id), n).Body)
		}},
		{"enqueue", func(ctx context.Context, _, n int) error {
			body, err := json.Marshal(job{N: n, Text: "index the note"})
			if err == nil {
				_, err = a.pg.db.ExecContext(ctx, jobsInsert, body)
			}
			if err == nil {
				a.enqueued.Add(1)
			}
			return err
		}},
		{"write", func(ctx context.Context, worker, n int) error {
			return a.write(ctx, stepNote(worker, n), n)
		}},
		{"upload", func(ctx context.Context, worker, n int) error {
			return a.files.put(ctx, fmt.Sprintf("steps/%d-%d", worker, n), attachment)
		}},
		{"log", func(ctx context.Context, worker, n int) error {
			a.log.InfoContext(ctx, "request", "user", worker, "note", n, "write", false)
			return nil
		}},
	}
}
