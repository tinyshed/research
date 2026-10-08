package main

import (
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strconv"
	"strings"
	"time"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/internal/dbstat"
	"github.com/tinyshed/tinystore/internal/sqlite"
	"github.com/tinyshed/tinystore/metrics"
)

const epoch int64 = 1700000000000
const normalNow int64 = epoch + 6000
const fnvOffset uint64 = 14695981039346656037

var ctx = context.Background()
var sink output
var streamSink metrics.Result

type output struct {
	Reads         []metrics.Result
	Aggregates    []metrics.AggregateResult
	Maintenance   metrics.Maintenance
	StreamSamples int
}

type app struct {
	runtime *tinystore.Store
	store   *metrics.Store
}

func check(err error) {
	if err != nil {
		panic(err)
	}
}
func emit(value any) { check(json.NewEncoder(os.Stdout).Encode(value)) }
func options() metrics.Options {
	return metrics.Options{MaxHeadSamples: 1 << 20, MaxHeadBytes: 16 << 20, MaxBatchSamples: 100000, MaxBatchBytes: 64 << 20, MaxReaders: 1, MaxConcurrentReads: 1, MaintenanceSeries: 64}
}
func open(path string, now int64) *app {
	r, err := tinystore.Open(ctx, filepath.Dir(path), tinystore.Options{Manual: true, Clock: func() time.Time { return time.UnixMilli(now) }})
	check(err)
	s, err := metrics.Open(ctx, r, options())
	if err != nil {
		_ = r.Close(ctx)
		check(err)
	}
	return &app{r, s}
}
func (a *app) close() { check(a.runtime.Close(ctx)) }
func makeSeries(name string, id int, kind metrics.Kind) metrics.Series {
	return metrics.Series{Name: name, Kind: kind, Labels: metrics.Labels{"group": "bench", "id": strconv.Itoa(id)}}
}
func points(id, n int) []metrics.Sample {
	out := make([]metrics.Sample, n)
	for i := range out {
		out[i] = metrics.Sample{At: epoch + int64(i), Value: float64((i*17+id)%997) / 10}
	}
	return out
}
func fixture(path, kind string) {
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		panic("fixture already exists")
	}
	a := open(path, normalNow)
	var batches []metrics.Batch
	switch kind {
	case "sealed", "ready":
		for _, spec := range []struct {
			name string
			kind metrics.Kind
		}{{"bench_gauge", metrics.Gauge}, {"bench_counter", metrics.Counter}} {
			for id := range 8 {
				batches = append(batches, metrics.Batch{Series: makeSeries(spec.name, id, spec.kind), Samples: points(id, 4801)})
			}
		}
	case "head":
		for id := range 8 {
			batches = append(batches, metrics.Batch{Series: makeSeries("head", id, metrics.Gauge), Samples: points(id, 2001)})
		}
	case "scrape":
		for id := range 100 {
			batches = append(batches, metrics.Batch{Series: makeSeries("scrape", id, metrics.Gauge), Samples: points(id, 240)})
		}
	case "edge":
		batches = edgeBatches()
	case "empty":
	default:
		panic("unknown fixture")
	}
	if len(batches) > 0 {
		check(a.store.Ingest(ctx, batches))
	}
	var maintenance metrics.Maintenance
	if kind == "sealed" || kind == "edge" {
		var err error
		maintenance, err = a.store.Maintain(ctx)
		check(err)
	}
	a.close()
	emit(map[string]any{"fixture": kind, "series": len(batches), "maintenance": maintenance})
}

func readRange(name string) metrics.Range {
	return metrics.Range{Name: name, From: epoch, To: epoch + 4801}
}
func queryFor(name string) metrics.Range {
	r := readRange("bench_gauge")
	switch name {
	case "read_head_point":
		r = metrics.Range{Name: "head", Match: metrics.Labels{"id": "0"}, From: epoch + 1000, To: epoch + 1001}
	case "read_head_full8":
		r = metrics.Range{Name: "head", From: epoch, To: epoch + 2001}
	case "read_sealed_point":
		r.Match = metrics.Labels{"id": "3"}
		r.From = epoch + 1000
		r.To = epoch + 1001
	case "read_sealed_boundary":
		r.From = epoch + 4797
	case "read_filtered_one":
		r.Match = metrics.Labels{"id": "3"}
	case "read_sealed_full16":
		r.Name = ""
		r.Match = metrics.Labels{"group": "bench"}
	case "read_edge":
		r = metrics.Range{Match: metrics.Labels{"group": "edge"}, From: epoch, To: epoch + 5000}
	case "read_where":
		r.Where = metrics.Where{"id": metrics.OneOf("1", "3", "5")}
	case "read_prefix":
		r.Where = metrics.Where{"id": metrics.Prefix("1")}
	case "read_noneof":
		r.Where = metrics.Where{"id": metrics.NoneOf("0", "1")}
	case "read_since":
		r = metrics.Range{Name: "bench_gauge", Since: 2000 * time.Millisecond}
	}
	return r
}
func aggregateFor(name string) metrics.AggregateRequest {
	op := strings.TrimPrefix(name, "aggregate_")
	r := metrics.AggregateRequest{Range: readRange("bench_gauge"), Width: 5000 * time.Millisecond, Op: metrics.AggregateOp(op)}
	if strings.HasPrefix(op, "edge_") {
		parts := strings.Split(op, "_")
		if len(parts) != 3 {
			panic("invalid edge aggregate")
		}
		r.Op = metrics.AggregateOp(parts[1])
		r.Range = metrics.Range{Name: "edge", Match: metrics.Labels{"id": parts[2]}, From: epoch, To: epoch + 5000}
		if parts[2] == "10" {
			r.Range.Name = "edge_counter"
		}
		return r
	}
	switch op {
	case "increase", "rate":
		r.Range.Name = "bench_counter"
	case "cut_sum", "cut_avg":
		r.Range.From++
		r.Width = 481 * time.Millisecond
		r.Op = metrics.AggregateOp(strings.TrimPrefix(op, "cut_"))
	case "grouped_sum":
		r.Op = metrics.AggregateSum
		r.By = []string{"group"}
	case "grouped_increase":
		r.Op = metrics.AggregateIncrease
		r.Range.Name = "bench_counter"
		r.By = []string{"group"}
	}
	return r
}

func (a *app) run(name string, index uint64, verification bool) (out output) {
	if strings.HasPrefix(name, "read_") {
		var err error
		out.Reads, err = a.store.Read(ctx, queryFor(name))
		check(err)
		return
	}
	if strings.HasPrefix(name, "aggregate_") {
		var err error
		out.Aggregates, err = a.store.Aggregate(ctx, aggregateFor(name))
		check(err)
		return
	}
	if name == "stream_sealed_full8" {
		check(a.store.Stream(ctx, readRange("bench_gauge"), func(r metrics.Result) error {
			out.StreamSamples += len(r.Samples)
			if verification {
				out.Reads = append(out.Reads, r)
			} else {
				streamSink = r
			}
			return nil
		}))
		return
	}
	if name == "maintain_ready" || name == "expire_all" {
		var err error
		out.Maintenance, err = a.store.Maintain(ctx)
		check(err)
		return
	}
	var batches []metrics.Batch
	switch name {
	case "ingest_append1":
		batches = []metrics.Batch{{Series: makeSeries("head", 0, metrics.Gauge), Samples: []metrics.Sample{{At: epoch + 2001 + int64(index), Value: float64((index*17)%997) / 10}}}}
	case "ingest_replace1":
		batches = []metrics.Batch{{Series: makeSeries("head", 0, metrics.Gauge), Samples: []metrics.Sample{{At: epoch + 1000, Value: float64(index+1) / 10}}}}
	case "ingest_scrape100":
		for id := range 100 {
			batches = append(batches, metrics.Batch{Series: makeSeries("scrape", id, metrics.Gauge), Samples: []metrics.Sample{{At: epoch + 240 + int64(index), Value: float64((index*17+uint64(id))%997) / 10}}})
		}
	case "ingest_register8":
		for j := range 8 {
			q := index*8 + uint64(j)
			s := makeSeries("register", 0, metrics.Gauge)
			s.Labels["id"] = fmt.Sprintf("%06d", q)
			batches = append(batches, metrics.Batch{Series: s, Samples: points(int(q), 240)})
		}
	case "ingest_shuffled240", "ingest_duplicates240":
		p := make([]metrics.Sample, 240)
		for j := range p {
			k := j * 53 % 240
			if name == "ingest_duplicates240" {
				k = j % 120
			}
			p[j] = metrics.Sample{At: epoch + 1000 + int64(k), Value: float64((index*13+uint64(j)*17)%997) / 10}
		}
		batches = []metrics.Batch{{Series: makeSeries("head", 0, metrics.Gauge), Samples: p}}
	default:
		panic("unknown case " + name)
	}
	check(a.store.Ingest(ctx, batches))
	return
}

func hashBytes(h uint64, b []byte) uint64 {
	for _, v := range b {
		h = (h ^ uint64(v)) * 1099511628211
	}
	return h
}
func hashWord(h uint64, n uint64) uint64 {
	var b [8]byte
	binary.LittleEndian.PutUint64(b[:], n)
	return hashBytes(h, b[:])
}
func hashString(h uint64, s string) uint64 { return hashBytes(hashWord(h, uint64(len(s))), []byte(s)) }
func hashSeries(h uint64, s metrics.Series) uint64 {
	h = hashString(h, s.Name)
	h = hashString(h, string(s.Kind))
	h = hashWord(h, uint64(len(s.Labels)))
	keys := make([]string, 0, len(s.Labels))
	for k := range s.Labels {
		keys = append(keys, k)
	}
	slices.Sort(keys)
	for _, k := range keys {
		h = hashString(h, k)
		h = hashString(h, s.Labels[k])
	}
	return h
}
func hashReads(h uint64, reads []metrics.Result) uint64 {
	h = hashWord(h, uint64(len(reads)))
	for _, r := range reads {
		h = hashSeries(h, r.Series)
		h = hashWord(h, uint64(len(r.Samples)))
		for _, p := range r.Samples {
			h = hashWord(h, uint64(p.At))
			h = hashWord(h, math.Float64bits(p.Value))
		}
	}
	return h
}
func hashOutput(h uint64, out output) uint64 {
	h = hashReads(h, out.Reads)
	h = hashWord(h, uint64(len(out.Aggregates)))
	for _, r := range out.Aggregates {
		h = hashSeries(h, r.Series)
		h = hashWord(h, uint64(len(r.Buckets)))
		for _, b := range r.Buckets {
			h = hashWord(h, uint64(b.From))
			h = hashWord(h, uint64(b.To))
			h = hashWord(h, uint64(b.Count))
			h = hashWord(h, uint64(b.Resets))
			h = hashWord(h, math.Float64bits(b.Value))
			h = hashWord(h, boolWord(b.Overflow))
			h = hashWord(h, boolWord(b.Partial))
		}
	}
	for _, n := range []int{out.Maintenance.SealedBlocks, out.Maintenance.ExpiredSamples, out.Maintenance.Conflicts, out.Maintenance.QuarantinedSeries, out.Maintenance.ReclaimedSeries, out.StreamSamples} {
		h = hashWord(h, uint64(n))
	}
	return h
}
func boolWord(v bool) uint64 {
	if v {
		return 1
	}
	return 0
}
func (a *app) digest() uint64 {
	reads, err := a.store.Read(ctx, metrics.Range{Match: metrics.Labels{"group": "bench"}, From: epoch, To: epoch + 1000000})
	check(err)
	return hashReads(fnvOffset, reads)
}
func rssKiB() uint64 {
	b, err := os.ReadFile("/proc/self/status")
	check(err)
	for _, l := range strings.Split(string(b), "\n") {
		if strings.HasPrefix(l, "VmRSS:") {
			n, e := strconv.ParseUint(strings.Fields(l)[1], 10, 64)
			check(e)
			return n
		}
	}
	panic("RSS absent")
}

func main() {
	path := flag.String("db", "", "metrics.db path")
	mode := flag.String("mode", "verify", "fixture,verify,bench,digest,guards,memory,physical")
	kind := flag.String("fixture", "sealed", "fixture kind")
	name := flag.String("case", "read_sealed_full8", "case")
	iterations := flag.Uint64("iterations", 16, "fixed operations")
	warm := flag.Uint64("warm", 32, "warm operations")
	now := flag.Int64("now", normalNow, "frozen epoch ms")
	flag.Parse()
	if *path == "" {
		panic("--db required")
	}
	if filepath.Base(*path) != "metrics.db" {
		panic("--db must name metrics.db: the public engine owns that filename")
	}
	if *mode == "fixture" {
		fixture(*path, *kind)
		return
	}
	if *mode == "physical" {
		physical(*path)
		return
	}
	a := open(*path, *now)
	defer a.close()
	if *name == "maintain_ready" || *name == "expire_all" {
		if *iterations != 1 {
			panic("one-shot maintenance requires iterations=1")
		}
		*warm = 0
	}
	switch *mode {
	case "digest":
		emit(map[string]any{"final_digest": fmt.Sprintf("%016x", a.digest())})
	case "guards":
		emit(a.guards())
	case "edge-guards":
		_, err := a.store.Aggregate(ctx, aggregateFor("aggregate_edge_sum_7"))
		emit(map[string]any{"nonfinite": errors.Is(err, metrics.ErrNonFinite)})
	case "plan":
		var p metrics.Plan
		var err error
		if strings.HasPrefix(*name, "aggregate_") {
			p, err = a.store.ExplainAggregate(ctx, aggregateFor(*name))
		} else {
			p, err = a.store.ExplainRead(ctx, queryFor(*name))
		}
		check(err)
		emit(map[string]any{"series": p.Series, "blocks": p.Blocks, "summarized": p.Summarized, "bytes": p.PayloadBytes, "decoded": p.DecodedSamples, "stops": p.Stops})
	case "verify":
		h := fnvOffset
		var samples, buckets int
		for i := uint64(0); i < *iterations; i++ {
			out := a.run(*name, i, true)
			h = hashOutput(h, out)
			for _, r := range out.Reads {
				samples += len(r.Samples)
			}
			for _, r := range out.Aggregates {
				buckets += len(r.Buckets)
			}
		}
		emit(map[string]any{"implementation": "go_metrics", "case": *name, "iterations": *iterations, "checksum": fmt.Sprintf("%016x", h), "samples": samples, "buckets": buckets, "final_digest": fmt.Sprintf("%016x", a.digest())})
	case "bench":
		for i := uint64(0); i < *warm; i++ {
			sink = a.run(*name, i, false)
		}
		start := time.Now()
		for i := uint64(0); i < *iterations; i++ {
			sink = a.run(*name, i+*warm, false)
		}
		elapsed := time.Since(start)
		emit(map[string]any{"implementation": "go_metrics", "case": *name, "iterations": *iterations, "warm": *warm, "ns_per_op": float64(elapsed.Nanoseconds()) / float64(*iterations)})
	case "memory":
		var held []output
		for i := uint64(0); i < *iterations; i++ {
			out := a.run(*name, i, false)
			held = append(held, out)
		}
		runtime.GC()
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		var samples int
		for _, out := range held {
			for _, r := range out.Reads {
				samples += len(r.Samples)
			}
		}
		emit(map[string]any{"implementation": "go_metrics", "case": *name, "iterations": *iterations, "rss_kib": rssKiB(), "retained_samples": samples, "heap_alloc_bytes": m.HeapAlloc, "heap_sys_bytes": m.HeapSys})
		runtime.KeepAlive(held)
		runtime.KeepAlive(streamSink)
	default:
		panic("unknown mode")
	}
}

func (a *app) guards() map[string]any {
	before := a.digest()
	s := makeSeries("bench_gauge", 0, metrics.Gauge)
	out := map[string]any{}
	for name, p := range map[string]metrics.Sample{"too_old": {At: epoch + 10, Value: 1}, "too_new": {At: normalNow + 600001, Value: 1}} {
		err := a.store.Ingest(ctx, []metrics.Batch{{Series: s, Samples: []metrics.Sample{p}}})
		want := metrics.ErrTooOld
		if name == "too_new" {
			want = metrics.ErrTooNew
		}
		out[name] = errors.Is(err, want)
	}
	err := a.store.Ingest(ctx, []metrics.Batch{{Series: s, Samples: []metrics.Sample{{At: epoch + 4801, Value: 123}}}, {Series: makeSeries("bench_gauge", 1, metrics.Gauge), Samples: []metrics.Sample{{At: epoch + 10, Value: 456}}}})
	out["atomic_rollback"] = errors.Is(err, metrics.ErrTooOld) && a.digest() == before
	r := queryFor("read_sealed_point")
	r.Limits.DecodedSamples = 1
	_, err = a.store.Read(ctx, r)
	out["decode_limit"] = errors.Is(err, metrics.ErrLimit)
	r = readRange("bench_gauge")
	r.Limits.OutputSamples = 1
	_, err = a.store.Read(ctx, r)
	out["output_limit"] = errors.Is(err, metrics.ErrLimit)
	r.Limits = metrics.Limits{Series: 1}
	_, err = a.store.Read(ctx, r)
	out["series_limit"] = errors.Is(err, metrics.ErrLimit)
	out["final_digest"] = fmt.Sprintf("%016x", a.digest())
	return out
}

func physical(path string) {
	objects, err := dbstat.Read(ctx, path)
	check(err)
	f, err := sqlite.Open(ctx, path, sqlite.Config{Readers: 1})
	check(err)
	out := map[string]any{"objects": objects}
	info, err := os.Stat(path)
	check(err)
	out["file_bytes"] = info.Size()
	check(f.Lookup(ctx, func(r sqlite.Reader) error {
		counts := map[string]int64{}
		for _, name := range []string{"series", "label_values", "postings", "series_state", "groups", "clocks", "payloads"} {
			var n int64
			if e := sqlite.QueryRow(ctx, r, "select count(*) from "+name).Scan(&n); e != nil {
				return e
			}
			counts[name] = n
		}
		out["row_counts"] = counts
		for key, sql := range map[string]string{"head_bytes": "select coalesce(sum(length(tail)),0) from series_state", "directory_bytes": "select coalesce(sum(length(directory)),0) from groups", "clock_bytes": "select coalesce(sum(length(body)),0) from clocks", "payload_bytes": "select coalesce(sum(length(body)),0) from payloads", "head_samples": "select coalesce(sum(head_count),0) from series_state"} {
			var n int64
			if e := sqlite.QueryRow(ctx, r, sql).Scan(&n); e != nil {
				return e
			}
			out[key] = n
		}
		settings := map[string]any{}
		for _, name := range []string{"page_size", "synchronous", "fullfsync", "checkpoint_fullfsync", "foreign_keys", "busy_timeout", "cache_size", "trusted_schema", "wal_autocheckpoint", "mmap_size"} {
			var n int64
			if e := sqlite.QueryRow(ctx, r, "pragma "+name).Scan(&n); e != nil {
				return e
			}
			settings[name] = n
		}
		var version, source string
		if e := sqlite.QueryRow(ctx, r, "select sqlite_version(),sqlite_source_id()").Scan(&version, &source); e != nil {
			return e
		}
		settings["version"] = version
		settings["source_id"] = source
		out["settings"] = settings
		return nil
	}))
	check(f.Close())
	emit(out)
}
