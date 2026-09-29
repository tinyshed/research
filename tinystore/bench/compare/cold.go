package main

import (
	"context"
	"fmt"
	"os"
	"time"
)

// The cold rounds read from a store just opened after the kernel dropped its
// page cache, as after a reboot, then read the same keys again warm: a
// memory-mapped store answers a warm read from memory, and only the cold one
// says what its layout costs on the disk.
//
// Dropping the cache needs a privileged container; without one the stage says
// so and its numbers are warm ones.

const coldReads = 20_000

func init() {
	engines["kv-cold"] = engine{
		order:      []string{"tinystore", "sqlite", "bbolt", "badger", "pebble"},
		contenders: kvEngine.contenders,
		measure: func(ctx context.Context, s subject, _ float64) ([]stage, error) {
			store, err := filledKV(ctx, s)
			if err != nil {
				return nil, err
			}
			reopened, dropped, err := closeDropReopen(ctx, store)
			if err != nil {
				return nil, err
			}
			return coldAndWarm(ctx, dropped, kvGet(reopened.(kvStore))), nil
		}}
	engines["sqldb-cold"] = engine{
		order:      []string{"tinystore", "sqlite", "postgres"},
		contenders: sqlEngine.contenders,
		measure: func(ctx context.Context, s subject, _ float64) ([]stage, error) {
			store, ok := s.(sqlStore)
			if !ok {
				return nil, fmt.Errorf("%T is not an SQL store", s)
			}
			if err := fillNotes(ctx, store); err != nil {
				return nil, err
			}
			reopened, dropped, err := closeDropReopen(ctx, store)
			if err != nil {
				return nil, err
			}
			return coldAndWarm(ctx, dropped, sqlGet(reopened.(sqlStore))), nil
		}}
}

// closeDropReopen closes the contender, syncs, drops the page cache and opens
// the contender again, which the child then closes
func closeDropReopen(ctx context.Context, s subject) (subject, error, error) {
	if err := s.close(); err != nil {
		return nil, nil, err
	}
	dropped := dropPageCache()
	reopened, err := reopen(ctx)
	if err != nil {
		return nil, nil, fmt.Errorf("open again: %w", err)
	}
	active = reopened
	return reopened, dropped, nil
}

func dropPageCache() error {
	if err := syncAll(); err != nil {
		return err
	}
	return os.WriteFile("/proc/sys/vm/drop_caches", []byte("3"), 0o200)
}

// coldAndWarm reads coldReads keys one after another, then the same again
func coldAndWarm(ctx context.Context, dropped error, op operation) []stage {
	cold := countStage(ctx, "get-cold", coldReads, op)
	if dropped != nil {
		cold.FirstError = "the page cache was not dropped: " + dropped.Error()
	}
	return []stage{cold, countStage(ctx, "get-warm", coldReads, op)}
}

// countStage calls op n times from one goroutine and times them
func countStage(ctx context.Context, name string, n int, op operation) stage {
	var latencies histogram
	s := stage{Name: name, Goroutines: 1, Ops: int64(n)}
	before := spent()
	began := time.Now()
	for i := range n {
		at := time.Now()
		if err := op(ctx, 0, i); err != nil {
			s.Errors++
			if s.FirstError == "" {
				s.FirstError = err.Error()
			}
		}
		latencies.add(time.Since(at))
	}
	s.Seconds = time.Since(began).Seconds()
	s.PerSecond = float64(n) / s.Seconds
	s.P50Micros, s.P99Micros = latencies.quantile(0.5), latencies.quantile(0.99)
	s.addUsage(before, spent())
	return s
}
