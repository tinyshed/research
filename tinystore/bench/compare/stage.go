package main

import (
	"context"
	"fmt"
	"math/bits"
	"os"
	"path/filepath"
	"runtime"
	"runtime/pprof"
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
	// what the contender's processes, its services' included, spent on the
	// stage: CPU time and bytes the storage layer read and wrote
	CPUSeconds float64 `json:"cpu_seconds"`
	ReadBytes  int64   `json:"read_bytes"`
	WriteBytes int64   `json:"write_bytes"`
	// a long stage's minutes, and a fixed load's rate a second and the calls
	// it could not offer on time because every worker was busy
	Timeline []window `json:"timeline,omitempty"`
	Offered  float64  `json:"offered_per_second,omitempty"`
	Missed   int64    `json:"missed,omitempty"`
	// what an application kept of the stage's work, see appCounts
	App *appCounts `json:"app,omitempty"`
}

// operation is one call of a stage; worker and n tell it which goroutine it is
// on and how many calls that goroutine made before in this process, so that
// each chooses its keys without sharing a generator.
type operation func(ctx context.Context, worker int, n int) error

// callsMade is how many calls each worker made in this process: a warm-up,
// the stages after it and a long stage's minutes continue one numbering, so
// that no call makes a key an earlier one made, such as an upload's.
var callsMade []int

// resumeCalls makes room for goroutines' numbering before they start; each
// worker then reads and writes only its own entry
func resumeCalls(goroutines int) {
	for len(callsMade) < goroutines {
		callsMade = append(callsMade, 0)
	}
}

// timeStage runs op on goroutines for seconds, after a warm-up of a fifth of
// that which it does not count.
func timeStage(ctx context.Context, name string, goroutines int, seconds float64, op operation) stage {
	warm := time.Duration(seconds * float64(time.Second) / 5)
	_ = runFor(ctx, goroutines, warm, op)
	before := spent()
	result := profiled(name, goroutines, func() stage {
		return runFor(ctx, goroutines, time.Duration(seconds*float64(time.Second)), op)
	})
	result.Name, result.Goroutines = name, goroutines
	result.addUsage(before, spent())
	return result
}

func runFor(ctx context.Context, goroutines int, length time.Duration, op operation) stage {
	var (
		ops, failed atomic.Int64
		firstError  atomic.Value
		wg          sync.WaitGroup
		latencies   = make([]histogram, goroutines)
	)
	resumeCalls(goroutines)
	start := time.Now()
	deadline := start.Add(length)
	for worker := range goroutines {
		wg.Go(func() {
			ctx, cancel := workerContext(ctx)
			defer cancel()
			n := callsMade[worker]
			for ; time.Now().Before(deadline) && ctx.Err() == nil; n++ {
				began := time.Now()
				err := op(ctx, worker, n)
				latencies[worker].add(time.Since(began))
				ops.Add(1)
				if err != nil {
					failed.Add(1)
					firstError.CompareAndSwap(nil, err.Error())
				}
			}
			callsMade[worker] = n
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

// profiled runs measure under a CPU profile and with mutex and blocking
// contention recorded, when COMPARE_PROFILE names a directory and
// COMPARE_PROFILE_STAGE the stage, as request×64. A profiled stage's rate is
// the profile's, not a measurement.
func profiled(name string, goroutines int, measure func() stage) stage {
	dir, want := os.Getenv("COMPARE_PROFILE"), os.Getenv("COMPARE_PROFILE_STAGE")
	if dir == "" || want != fmt.Sprintf("%s×%d", name, goroutines) {
		return measure()
	}
	base := filepath.Join(dir, fmt.Sprintf("%d-%s-%d", os.Getpid(), name, goroutines))
	cpu, err := os.Create(base + ".cpu")
	if err == nil {
		err = pprof.StartCPUProfile(cpu)
	}
	runtime.SetMutexProfileFraction(5)
	runtime.SetBlockProfileRate(int(10 * time.Microsecond))
	s := measure()
	if err == nil {
		pprof.StopCPUProfile()
	}
	if cpu != nil {
		_ = cpu.Close()
	}
	for _, kind := range []string{"mutex", "block", "goroutine"} {
		if out, err := os.Create(base + "." + kind); err == nil {
			_ = pprof.Lookup(kind).WriteTo(out, 0)
			_ = out.Close()
		}
	}
	runtime.SetMutexProfileFraction(0)
	runtime.SetBlockProfileRate(0)
	return s
}

// sharedContext hands every worker the one context of the run, as an errgroup
// does, instead of a context of its own, as a server gives each request
var sharedContext = os.Getenv("COMPARE_SHARED_CONTEXT") == "1"

// workerContext is a worker's context, which the run's still cancels. A select
// locks every channel it names, so workers sharing one Done channel contend on
// its lock in every select of the store they call, even one that never waits.
func workerContext(ctx context.Context) (context.Context, context.CancelFunc) {
	if sharedContext {
		return ctx, func() {}
	}
	return context.WithCancel(ctx)
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

func (s *stage) addUsage(before, after usage) {
	s.CPUSeconds = after.cpu - before.cpu
	s.ReadBytes, s.WriteBytes = after.read-before.read, after.write-before.write
}
