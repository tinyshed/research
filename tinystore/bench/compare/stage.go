package main

import (
	"context"
	"math/bits"
	"sync"
	"sync/atomic"
	"time"
)

// stage is one timed load: goroutines calling one operation for a fixed time.
type stage struct {
	Name       string  `json:"name"`
	Goroutines int     `json:"goroutines"`
	Ops        int64   `json:"ops"`
	Errors     int64   `json:"errors"`
	Seconds    float64 `json:"seconds"`
	PerSecond  float64 `json:"per_second"`
	P50Micros  float64 `json:"p50_us"`
	P99Micros  float64 `json:"p99_us"`
	FirstError string  `json:"first_error,omitempty"`
}

// operation is one call of a stage; worker and n tell it which goroutine it is
// on and how many calls that goroutine made before, so that each chooses its
// keys without sharing a generator.
type operation func(ctx context.Context, worker int, n int) error

// timeStage runs op on goroutines for seconds, after a warm-up of a fifth of
// that which it does not count.
func timeStage(ctx context.Context, name string, goroutines int, seconds float64, op operation) stage {
	warm := time.Duration(seconds * float64(time.Second) / 5)
	_ = runFor(ctx, goroutines, warm, op)
	result := runFor(ctx, goroutines, time.Duration(seconds*float64(time.Second)), op)
	result.Name, result.Goroutines = name, goroutines
	return result
}

func runFor(ctx context.Context, goroutines int, length time.Duration, op operation) stage {
	var (
		ops, failed atomic.Int64
		firstError  atomic.Value
		wg          sync.WaitGroup
		latencies   = make([]histogram, goroutines)
	)
	start := time.Now()
	deadline := start.Add(length)
	for worker := range goroutines {
		wg.Go(func() {
			for n := 0; time.Now().Before(deadline) && ctx.Err() == nil; n++ {
				began := time.Now()
				err := op(ctx, worker, n)
				latencies[worker].add(time.Since(began))
				ops.Add(1)
				if err != nil {
					failed.Add(1)
					firstError.CompareAndSwap(nil, err.Error())
				}
			}
		})
	}
	wg.Wait()
	elapsed := time.Since(start).Seconds()

	var all histogram
	for i := range latencies {
		all.merge(&latencies[i])
	}
	s := stage{Ops: ops.Load(), Errors: failed.Load(), Seconds: elapsed, PerSecond: float64(ops.Load()) / elapsed,
		P50Micros: all.quantile(0.50), P99Micros: all.quantile(0.99)}
	if text, ok := firstError.Load().(string); ok {
		s.FirstError = text
	}
	return s
}

// histogram counts durations in buckets eight to a power of two, so a
// quantile it gives is within an eighth of the true one:
//
//	1300 ns  →  bits.Len 11, next three bits 010  →  bucket 11·8+2
type histogram struct {
	counts [64 * 8]int64
	total  int64
}

func (h *histogram) add(d time.Duration) {
	ns := uint64(max(d, 1))
	length := bits.Len64(ns)
	var fraction uint64
	if length > 3 {
		fraction = (ns >> (length - 4)) & 7
	}
	h.counts[length*8+int(fraction)]++
	h.total++
}

func (h *histogram) merge(o *histogram) {
	for i, c := range o.counts {
		h.counts[i] += c
	}
	h.total += o.total
}

// quantile is the upper edge of the bucket the quantile falls in, in µs
func (h *histogram) quantile(q float64) float64 {
	want := int64(q * float64(h.total))
	var seen int64
	for i, c := range h.counts {
		seen += c
		if seen > want {
			length, fraction := i/8, i%8
			if length <= 3 {
				return float64(uint64(1)<<length) / 1000
			}
			upper := (uint64(8+fraction+1) << (length - 4))
			return float64(upper) / 1000
		}
	}
	return 0
}
