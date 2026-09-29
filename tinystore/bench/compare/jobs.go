package main

import (
	"context"
	"database/sql"
	_ "embed"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"sync"
	"sync/atomic"
	"time"

	"maragu.dev/goqite"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/jobs"
)

// jobsStore is what every queue contender does: enqueue one job durably, and
// run every job waiting with workers until none is left.
type jobsStore interface {
	subject
	enqueue(ctx context.Context, n int) error
	drain(ctx context.Context, workers int) (int64, error)
}

var jobsEngine = engine{
	order: []string{"tinystore", "goqite"},
	contenders: map[string]opener{
		"tinystore": openTinyStoreJobs,
		"goqite":    openGoqiteJobs,
	},
	measure: measureJobs,
}

// job is a small message, as a scheduled email or a webhook is
type job struct {
	N    int    `json:"n"`
	Text string `json:"text"`
}

const drainWorkers = 8

// measureJobs times enqueues from 1, 8 and 64 goroutines, then drains what
// they left with eight workers whose handler returns at once
func measureJobs(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	store, ok := s.(jobsStore)
	if !ok {
		return nil, fmt.Errorf("%T is not a queue", s)
	}
	var stages []stage
	for _, goroutines := range []int{1, 8, 64} {
		stages = append(stages, timeStage(ctx, "enqueue", goroutines, seconds, func(ctx context.Context, _, n int) error {
			return store.enqueue(ctx, n)
		}))
	}
	before := spent()
	began := time.Now()
	done, err := store.drain(ctx, drainWorkers)
	elapsed := time.Since(began).Seconds()
	drained := stage{Name: "drain", Goroutines: drainWorkers, Ops: done, Seconds: elapsed, PerSecond: float64(done) / elapsed}
	drained.addUsage(before, spent())
	if err != nil {
		drained.Errors, drained.FirstError = 1, err.Error()
	}
	return append(stages, drained), nil
}

// TinyStore: a queue of its jobs engine, drained by its own Work loop.

type tinyStoreJobs struct {
	noService
	store *tinystore.Store
	queue *jobs.Queue[job]
}

func openTinyStoreJobs(ctx context.Context, dir string) (subject, error) {
	store, err := tinystore.Open(ctx, dir, tinystore.Options{})
	if err != nil {
		return nil, err
	}
	queues, err := jobs.Open(ctx, store, jobs.Options{})
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	queue, err := jobs.OpenQueue[job](ctx, queues, "mail")
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	return &tinyStoreJobs{store: store, queue: queue}, nil
}

func (t *tinyStoreJobs) enqueue(ctx context.Context, n int) error {
	return t.queue.Enqueue(ctx, job{N: n, Text: "send the weekly digest"})
}

func (t *tinyStoreJobs) drain(ctx context.Context, workers int) (int64, error) {
	var done atomic.Int64
	err := t.queue.Work(ctx, func(context.Context, jobs.Job[job]) error {
		done.Add(1)
		return nil
	}, jobs.Workers(workers), jobs.UntilIdle())
	return done.Load(), err
}

func (t *tinyStoreJobs) close() error { return t.store.Close(context.Background()) }

// goqiteSchema is goqite v0.4.0's schema_sqlite.sql, which it leaves to the
// application to run
//
//go:embed goqite_schema.sql
var goqiteSchema string

// goqite: a queue in a SQLite table, as its README sets it up, through one
// connection with TinyStore's pragmas; each worker receives and deletes.

type goqiteJobs struct {
	noService
	db    *sql.DB
	queue *goqite.Queue
}

func openGoqiteJobs(ctx context.Context, dir string) (subject, error) {
	dsn := "file:" + filepath.ToSlash(filepath.Join(dir, "queue.db")) +
		"?_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)&_pragma=busy_timeout(5000)&_txlock=immediate"
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	if _, err = db.ExecContext(ctx, goqiteSchema); err != nil {
		return nil, errors.Join(err, db.Close())
	}
	return &goqiteJobs{db: db, queue: goqite.New(goqite.NewOpts{DB: db, Name: "mail"})}, nil
}

func (g *goqiteJobs) enqueue(ctx context.Context, n int) error {
	body, err := json.Marshal(job{N: n, Text: "send the weekly digest"})
	if err != nil {
		return err
	}
	return g.queue.Send(ctx, goqite.Message{Body: body})
}

func (g *goqiteJobs) drain(ctx context.Context, workers int) (int64, error) {
	var (
		done     atomic.Int64
		wg       sync.WaitGroup
		firstErr error
		once     sync.Once
	)
	for range workers {
		wg.Go(func() {
			for {
				m, err := g.queue.Receive(ctx)
				if err == nil && m != nil {
					err = g.queue.Delete(ctx, m.ID)
				}
				if err != nil {
					once.Do(func() { firstErr = err })
					return
				}
				if m == nil {
					return
				}
				done.Add(1)
			}
		})
	}
	wg.Wait()
	return done.Load(), firstErr
}

func (g *goqiteJobs) close() error { return g.db.Close() }
