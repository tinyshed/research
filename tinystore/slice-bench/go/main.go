// The Go engines under the slice round's load: one case a process, a store of
// its own, the same keys, rows and jobs as the Rust program beside it.
//
// slice-bench --dir <dir> --case <case> --callers <n> --seconds <s> prints one
// line of JSON: the calls made, the time they took and their latencies.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"sort"
	"strings"
	"sync"
	"sync/atomic"
	"testing/fstest"
	"time"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/jobs"
	"github.com/tinyshed/tinystore/kv"
	"github.com/tinyshed/tinystore/sqldb"
)

const (
	keys         = 100_000
	valueBytes   = 128
	notes        = 100_000
	rows         = 25_000
	drained      = 20_000
	drainWorkers = 8

	schema      = `create table note (id integer primary key, title text not null, body text not null) strict`
	insertNote  = `insert into note (id, title, body) values (?, ?, ?)`
	pointNote   = `select id, title, body from note where id = ?`
	pageOfNotes = `select id, title, body from note order by id limit 25000`
)

type note struct {
	ID    int64
	Title string
	Body  string
}

type job struct {
	N    int64  `json:"n"`
	Text string `json:"text"`
}

// measured is what one case measured: the calls and each one's nanoseconds.
type measured struct {
	ops       int64
	elapsed   time.Duration
	latencies []uint32
}

func main() {
	dir := flag.String("dir", "", "the store's directory")
	name := flag.String("case", "", "the case to run")
	callers := flag.Int("callers", 1, "goroutines calling at once")
	seconds := flag.Float64("seconds", 3, "how long the timed part runs")
	flag.Parse()

	ctx := context.Background()
	store, err := tinystore.Open(ctx, *dir, tinystore.Options{})
	check(err)
	m, err := run(ctx, store, *name, *callers, time.Duration(*seconds*float64(time.Second)))
	check(err)
	check(store.Close(ctx))

	at := func(quantile float64) uint32 {
		if len(m.latencies) == 0 {
			return 0
		}
		return m.latencies[int(float64(len(m.latencies)-1)*quantile+0.5)]
	}
	check(json.NewEncoder(os.Stdout).Encode(map[string]any{
		"engine": "go", "case": *name, "callers": *callers, "ops": m.ops,
		"elapsed_ns": m.elapsed.Nanoseconds(), "per_second": float64(m.ops) / m.elapsed.Seconds(),
		"p50_ns": at(0.50), "p99_ns": at(0.99),
	}))
}

func check(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(ctx context.Context, store *tinystore.Store, name string, callers int, d time.Duration) (measured, error) {
	switch name {
	case "kv-get", "kv-set":
		state, err := kv.Open(ctx, store, kv.Options{})
		if err != nil {
			return measured{}, err
		}
		bucket, err := kv.OpenBucket[[]byte](ctx, state, "sessions")
		if err != nil {
			return measured{}, err
		}
		if name == "kv-set" {
			return timed(callers, d, func(caller int, call uint64) error {
				return bucket.Set(ctx, fmt.Sprintf("s%02d-%010d", caller, call), value(call))
			})
		}
		if err := fill(64, keys, func(n uint64) error { return bucket.Set(ctx, key(n), value(n)) }); err != nil {
			return measured{}, err
		}
		return timed(callers, d, func(caller int, call uint64) error {
			found, ok, err := bucket.Get(ctx, key(pick(caller, call)%keys))
			if err == nil && (!ok || len(found) != valueBytes) {
				err = fmt.Errorf("a key the fill wrote is missing")
			}
			return err
		})
	case "sql-point", "sql-insert", "sql-rows":
		count := map[string]int64{"sql-point": notes, "sql-insert": 0, "sql-rows": rows}[name]
		db, err := notesOf(ctx, store, count)
		if err != nil {
			return measured{}, err
		}
		switch name {
		case "sql-point":
			return timed(callers, d, func(caller int, call uint64) error {
				id := int64(pick(caller, call)%notes) + 1
				found, ok, err := sqldb.One[note](ctx, db, pointNote, id)
				if err == nil && (!ok || found.ID != id) {
					err = fmt.Errorf("note %d is missing", id)
				}
				return err
			})
		case "sql-insert":
			return timed(callers, d, func(caller int, call uint64) error {
				id := int64(caller+1)*1_000_000_000 + int64(call)
				_, err := db.Exec(ctx, insertNote, id, title(id), body(id))
				return err
			})
		default:
			return timed(1, d, func(int, uint64) error {
				page, err := sqldb.All[note](ctx, db, pageOfNotes)
				if err == nil && len(page) != rows {
					err = fmt.Errorf("%d rows of %d", len(page), rows)
				}
				return err
			})
		}
	case "jobs-add", "jobs-drain":
		queues, err := jobs.Open(ctx, store, jobs.Options{})
		if err != nil {
			return measured{}, err
		}
		queue, err := jobs.OpenQueue[job](ctx, queues, "mail")
		if err != nil {
			return measured{}, err
		}
		add := func(n uint64) error {
			return queue.Enqueue(ctx, job{N: int64(n), Text: "send the weekly digest"})
		}
		if name == "jobs-add" {
			return timed(callers, d, func(_ int, call uint64) error { return add(call) })
		}
		if err := fill(64, drained, add); err != nil {
			return measured{}, err
		}
		var ran atomic.Int64
		began := time.Now()
		err = queue.Work(ctx, func(context.Context, jobs.Job[job]) error {
			ran.Add(1)
			return nil
		}, jobs.Workers(drainWorkers), jobs.UntilIdle())
		return measured{ops: ran.Load(), elapsed: time.Since(began)}, err
	}
	return measured{}, fmt.Errorf("no case %s", name)
}

// notesOf opens a database of count notes, written a thousand a transaction.
func notesOf(ctx context.Context, store *tinystore.Store, count int64) (*sqldb.DB, error) {
	migrations := fstest.MapFS{"0001_notes.sql": {Data: []byte(schema + ";\n")}}
	db, err := sqldb.Open(ctx, store, "app", migrations, nil)
	if err != nil {
		return nil, err
	}
	for from := int64(1); from <= count; from += 1000 {
		err := db.Tx(ctx, func(tx *sqldb.Tx) error {
			for id := from; id < from+1000 && id <= count; id++ {
				if _, err := tx.Exec(ctx, insertNote, id, title(id), body(id)); err != nil {
					return err
				}
			}
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	return db, nil
}

func key(n uint64) string { return fmt.Sprintf("k%08d", n) }

func value(n uint64) []byte {
	bytes := make([]byte, valueBytes)
	for at := range bytes {
		bytes[at] = byte(int(n) + at)
	}
	return bytes
}

func title(id int64) string { return fmt.Sprintf("note %012d", id) }

func body(id int64) string { return strings.Repeat(fmt.Sprintf("%010d ", id), 10)[:100] }

// pick is the call'th choice of a caller: a xorshift of its own, the same in
// Rust and in Bun, so that every program reads the same keys in the same order.
func pick(caller int, call uint64) uint64 {
	x := (uint64(caller)+1)*0x9E3779B97F4A7C15 ^ call*0xBF58476D1CE4E5B9
	x ^= x >> 30
	x *= 0xBF58476D1CE4E5B9
	x ^= x >> 27
	x *= 0x94D049BB133111EB
	return x ^ (x >> 31)
}

// fill makes count calls from callers goroutines, untimed.
func fill(callers int, count uint64, call func(uint64) error) error {
	var next atomic.Uint64
	var wg sync.WaitGroup
	errs := make(chan error, callers)
	for range callers {
		wg.Go(func() {
			for {
				n := next.Add(1) - 1
				if n >= count {
					return
				}
				if err := call(n); err != nil {
					errs <- err
					return
				}
			}
		})
	}
	wg.Wait()
	close(errs)
	return <-errs
}

// timed calls call from callers goroutines for d, each call timed.
func timed(callers int, d time.Duration, call func(caller int, n uint64) error) (measured, error) {
	var stop atomic.Bool
	var wg sync.WaitGroup
	start := make(chan struct{})
	all := make([][]uint32, callers)
	errs := make(chan error, callers)
	for caller := range callers {
		wg.Go(func() {
			latencies := make([]uint32, 0, 1<<16)
			<-start
			for made := uint64(0); !stop.Load(); made++ {
				began := time.Now()
				if err := call(caller, made); err != nil {
					errs <- err
					return
				}
				latencies = append(latencies, uint32(min(time.Since(began).Nanoseconds(), 1<<32-1)))
			}
			all[caller] = latencies
		})
	}
	began := time.Now()
	close(start)
	time.Sleep(d)
	stop.Store(true)
	wg.Wait()
	elapsed := time.Since(began)
	close(errs)
	if err := <-errs; err != nil {
		return measured{}, err
	}
	var latencies []uint32
	for _, one := range all {
		latencies = append(latencies, one...)
	}
	sort.Slice(latencies, func(a, b int) bool { return latencies[a] < latencies[b] })
	return measured{ops: int64(len(latencies)), elapsed: elapsed, latencies: latencies}, nil
}
