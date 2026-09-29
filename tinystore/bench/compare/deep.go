package main

import (
	"context"
	"fmt"
	"sync"
	"sync/atomic"
	"time"
)

// The deep rounds ask what a five-second stage cannot: how a store behaves
// after half an hour, when its compactions and checkpoints have caught up
// with it, and what latency it gives at a load below its maximum, which is
// where an application lives.

// runDir is the child's contender's directory, whose size a timeline reads
var runDir string

// window is one minute of a long stage.
type window struct {
	At         float64 `json:"at_seconds"`
	PerSecond  float64 `json:"per_second"`
	P50Micros  float64 `json:"p50_us"`
	P99Micros  float64 `json:"p99_us"`
	Errors     int64   `json:"errors"`
	DiskBytes  int64   `json:"disk_bytes"`
	CPUSeconds float64 `json:"cpu_seconds"`
	WriteBytes int64   `json:"write_bytes"`
}

const timelineWindow = time.Minute

// timeline runs op on goroutines for seconds in one-minute windows, and says
// what each window did; the stage is the whole of it
func timeline(ctx context.Context, name string, goroutines int, seconds float64, op operation) stage {
	total := stage{Name: name, Goroutines: goroutines}
	before := spent()
	end := time.Now().Add(time.Duration(seconds * float64(time.Second)))
	var elapsed float64
	for time.Now().Before(end) && ctx.Err() == nil {
		start := spent()
		w := runFor(ctx, goroutines, min(timelineWindow, time.Until(end)), op)
		after := spent()
		elapsed += w.Seconds
		disk, _ := directoryBytes(runDir)
		total.Timeline = append(total.Timeline, window{At: elapsed, PerSecond: w.PerSecond, P50Micros: w.P50Micros,
			P99Micros: w.P99Micros, Errors: w.Errors, DiskBytes: disk, CPUSeconds: after.cpu - start.cpu,
			WriteBytes: after.write - start.write})
		total.Ops += w.Ops
		total.Errors += w.Errors
		if total.FirstError == "" {
			total.FirstError = w.FirstError
		}
	}
	total.Seconds = elapsed
	total.PerSecond = float64(total.Ops) / elapsed
	total.addUsage(before, spent())
	return total
}

// fixedLoads finds what op sustains from 64 goroutines, then offers a quarter,
// a half, three quarters and nine tenths of it, each for seconds
func fixedLoads(ctx context.Context, name string, seconds float64, op operation) []stage {
	most := timeStage(ctx, name+"-max", 64, seconds, op)
	stages := []stage{most}
	for _, share := range []float64{0.25, 0.5, 0.75, 0.9} {
		s := openLoop(ctx, most.PerSecond*share, seconds, op)
		s.Name = fmt.Sprintf("%s-%d%%", name, int(share*100))
		stages = append(stages, s)
	}
	return stages
}

// openLoop offers op at rate a second, on time whatever the last calls did,
// and measures each call from when it was due, so that a queue behind a slow
// call counts in the latency instead of hiding it
func openLoop(ctx context.Context, rate, seconds float64, op operation) stage {
	const workers = 1024
	due := make(chan time.Time, workers)
	var (
		latencies [workers]histogram
		ops, errs atomic.Int64
		first     atomic.Value
		wg        sync.WaitGroup
		missed    atomic.Int64
	)
	for worker := range workers {
		wg.Go(func() {
			n := 0
			for at := range due {
				err := op(ctx, worker, n)
				latencies[worker].add(time.Since(at))
				ops.Add(1)
				n++
				if err != nil {
					errs.Add(1)
					first.CompareAndSwap(nil, err.Error())
				}
			}
		})
	}
	before := spent()
	start := time.Now()
	interval := time.Duration(float64(time.Second) / rate)
	length := time.Duration(seconds * float64(time.Second))
	for next := start; next.Sub(start) < length && ctx.Err() == nil; next = next.Add(interval) {
		time.Sleep(time.Until(next))
		select {
		case due <- next:
		default:
			missed.Add(1) // every worker busy: the call is late, and counted as not offered
		}
	}
	close(due)
	wg.Wait()
	elapsed := time.Since(start).Seconds()

	var all histogram
	for i := range latencies {
		all.merge(&latencies[i])
	}
	s := stage{Goroutines: workers, Ops: ops.Load(), Errors: errs.Load(), Seconds: elapsed,
		PerSecond: float64(ops.Load()) / elapsed, P50Micros: all.quantile(0.5), P99Micros: all.quantile(0.99),
		Offered: rate, Missed: missed.Load()}
	s.addUsage(before, spent())
	if text, ok := first.Load().(string); ok {
		s.FirstError = text
	}
	return s
}

// The deep engines: the same contenders as kv and stack, a different measure.

func init() {
	engines["kv-steady"] = engine{order: steadyKV, contenders: kvEngine.contenders,
		measure: func(ctx context.Context, s subject, seconds float64) ([]stage, error) {
			store, err := filledKV(ctx, s)
			if err != nil {
				return nil, err
			}
			return []stage{timeline(ctx, "mixed", 64, seconds, kvMixed(store))}, nil
		}}
	engines["kv-latency"] = engine{order: steadyKV, contenders: kvEngine.contenders,
		measure: func(ctx context.Context, s subject, seconds float64) ([]stage, error) {
			store, err := filledKV(ctx, s)
			if err != nil {
				return nil, err
			}
			return fixedLoads(ctx, "mixed", seconds, kvMixed(store)), nil
		}}
	engines["stack-steady"] = engine{order: stackEngine.order, contenders: stackEngine.contenders,
		measure: func(ctx context.Context, s subject, seconds float64) ([]stage, error) {
			app, err := seededApp(ctx, s)
			if err != nil {
				return nil, err
			}
			return []stage{timeline(ctx, "request", 64, seconds, appRequests(app))}, nil
		}}
	engines["stack-latency"] = engine{order: stackEngine.order, contenders: stackEngine.contenders,
		measure: func(ctx context.Context, s subject, seconds float64) ([]stage, error) {
			app, err := seededApp(ctx, s)
			if err != nil {
				return nil, err
			}
			return fixedLoads(ctx, "request", seconds, appRequests(app)), nil
		}}
}

// the long rounds leave out the variants of a contender, which the short ones
// already set beside it
var steadyKV = []string{"tinystore", "sqlite", "bbolt", "badger", "pebble", "redis"}

func filledKV(ctx context.Context, s subject) (kvStore, error) {
	store, ok := s.(kvStore)
	if !ok {
		return nil, fmt.Errorf("%T is not a key-value store", s)
	}
	return store, fillKV(ctx, store)
}

func seededApp(ctx context.Context, s subject) (appStore, error) {
	app, ok := s.(appStore)
	if !ok {
		return nil, fmt.Errorf("%T is not an application's storage", s)
	}
	return app, app.seed(ctx)
}

func appRequests(app appStore) operation {
	return func(ctx context.Context, worker, n int) error {
		return app.serve(ctx, requestFor(worker, n))
	}
}
