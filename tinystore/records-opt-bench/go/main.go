package main

import (
	"bufio"
	"cmp"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"sort"
	"strings"
	"time"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/internal/dbstat"
	"github.com/tinyshed/tinystore/records"
)

const epoch int64 = 1700000000000000000
const now int64 = epoch + 3600000000000

var clockNow int64 = now

var ctx = context.Background()

func check(err error) {
	if err != nil {
		panic(err)
	}
}
func emit(v any) { check(json.NewEncoder(os.Stdout).Encode(v)) }

type Field struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}
type Record struct {
	At      int64   `json:"at"`
	Stream  string  `json:"stream"`
	Name    string  `json:"name"`
	Level   *int    `json:"level"`
	Body    *string `json:"body"`
	Trace   string  `json:"trace"`
	Span    string  `json:"span"`
	Context []Field `json:"context"`
	Attrs   []Field `json:"attrs"`
}

func portable(r records.Record) Record {
	p := Record{At: r.At.UnixNano(), Stream: r.Stream, Name: r.Name, Body: r.Body, Trace: hex.EncodeToString(r.TraceID[:]), Span: hex.EncodeToString(r.SpanID[:]), Context: []Field{}, Attrs: []Field{}}
	if r.Level != nil {
		p.Level = new(int(*r.Level))
	}
	for _, f := range r.Context {
		p.Context = append(p.Context, Field{f.Key, f.Value})
	}
	for _, f := range r.Attrs {
		p.Attrs = append(p.Attrs, Field{f.Key, f.Value})
	}
	return p
}
func fromPortable(r Record) records.Record {
	p := records.Record{At: time.Unix(0, r.At).UTC(), Stream: r.Stream, Name: r.Name, Body: r.Body}
	if r.Level != nil {
		p.Level = new(slog.Level(*r.Level))
	}
	t, e := hex.DecodeString(r.Trace)
	check(e)
	copy(p.TraceID[:], t)
	s, e := hex.DecodeString(r.Span)
	check(e)
	copy(p.SpanID[:], s)
	for _, f := range r.Context {
		p.Context = append(p.Context, records.Field{Key: f.Key, Value: f.Value})
	}
	for _, f := range r.Attrs {
		p.Attrs = append(p.Attrs, records.Field{Key: f.Key, Value: f.Value})
	}
	return p
}
func fixture(n int, corpus string) []records.Record {
	var lines []string
	if corpus != "" {
		f, e := os.Open(corpus)
		check(e)
		s := bufio.NewScanner(f)
		for s.Scan() {
			lines = append(lines, s.Text())
		}
		check(s.Err())
		check(f.Close())
	}
	out := make([]records.Record, n)
	for i := range out {
		at := epoch + int64((i*997)%max(1, n/2))*1000000
		r := records.Record{At: time.Unix(0, at).UTC(), Stream: fmt.Sprintf("stream-%d", i%2), Name: []string{"request", "click", "log"}[i%3]}
		if i%5 != 0 {
			r.Level = new(slog.Level((i%4)*4 - 4))
		}
		if i%7 != 0 {
			b := fmt.Sprintf("request %d completed route=/items/%d Unicode=Привет", i, i%17)
			if len(lines) > 0 {
				b = lines[i%len(lines)]
			}
			r.Body = &b
		}
		if i%3 == 0 {
			r.TraceID[0] = byte(i%251 + 1)
			r.TraceID[15] = byte(i%13 + 1)
			r.SpanID[0] = byte(i%251 + 1)
		}
		r.Context = []records.Field{{Key: "service", Value: `"api"`}, {Key: "host", Value: fmt.Sprintf(`"host-%d"`, i%4)}}
		r.Attrs = []records.Field{{Key: "seq", Value: fmt.Sprint(i)}, {Key: "status", Value: fmt.Sprint([]int{200, 404, 503}[i%3])}, {Key: "tag", Value: `"first"`}, {Key: "tag", Value: `"second"`}, {Key: "json", Value: `{"a":[true,null,1.25],"s":"a\\b"}`}}
		out[i] = r
	}
	return out
}
func input(batch []records.Record) int {
	size := 0
	for _, r := range batch {
		size += 32 + len(r.Stream) + len(r.Name)
		if r.Body != nil {
			size += len(*r.Body)
		}
		if r.TraceID != (records.TraceID{}) {
			size += 16
		}
		if r.SpanID != (records.SpanID{}) {
			size += 8
		}
		for _, fs := range [][]records.Field{r.Context, r.Attrs} {
			for _, f := range fs {
				size += 4 + len(f.Key) + len(f.Value)
			}
		}
	}
	return size
}

type app struct {
	r *tinystore.Store
	s *records.Store
}

func open(path string) app {
	r, e := tinystore.Open(ctx, filepath.Dir(path), tinystore.Options{Manual: true, Clock: func() time.Time { return time.Unix(0, clockNow) }})
	check(e)
	s, e := records.Open(ctx, r, records.Options{SealAge: time.Nanosecond})
	check(e)
	return app{r, s}
}
func (a app) close() { check(a.r.Close(ctx)) }
func query(name string) records.Query {
	q := records.Query{From: time.Unix(0, epoch-30000000000), To: time.Unix(0, epoch+3600000000000), Limit: 10000}
	switch name {
	case "scan_filtered":
		q.Attrs = []records.Field{{Key: "status", Value: "503"}}
		q.Context = []records.Field{{Key: "host", Value: `"host-2"`}}
		q.MinLevel = new(slog.Level(4))
	case "scan_search":
		q.Search = "COMPLETED"
	case "scan_trace":
		q.TraceID[0] = 1
		q.TraceID[15] = 1
	case "scan_page":
		q.Limit = 127
	case "scan_newest":
		q.Limit = 127
		q.Newest = true
	case "scan_budget":
		q.Budget = records.Budget{Blocks: 2, Decoded: 2048}
	case "scan_none":
		q.Attrs = []records.Field{{Key: "status", Value: "9999"}}
	case "scan_no_context":
		q.Context = []records.Field{{Key: "service", Value: `"missing"`}}
	}
	return q
}
func canon(batch []records.Record) string {
	h := sha256.New()
	for _, r := range batch {
		p := portable(r)
		h.Write(appendHeadRecord(nil, &r))
		h.Write([]byte(p.Stream))
		h.Write([]byte{0})
	}
	return hex.EncodeToString(h.Sum(nil))
}

var pageSink records.Page
var batchSink records.Batch
var payloadSink Payload

func engine(a app, name string, batch []records.Record) any {
	switch name {
	case "append":
		check(a.s.Append(ctx, batch...))
		return len(batch)
	case "seal":
		clockNow = now + 1
		m, e := a.s.Maintain(ctx)
		check(e)
		return m
	case "follow":
		b, e := a.s.Follow(ctx, records.Cursor{}, 127)
		check(e)
		batchSink = b
		return b
	case "follow_walk":
		var all []records.Record
		cursor := records.Cursor{}
		for {
			b, e := a.s.Follow(ctx, cursor, 127)
			check(e)
			if len(b.Records) == 0 {
				break
			}
			all = append(all, b.Records...)
			cursor = b.Next
		}
		batchSink = records.Batch{Records: all, Next: cursor}
		return batchSink
	default:
		p, e := a.s.Scan(ctx, query(name))
		check(e)
		pageSink = p
		return p
	}
}
func result(a app, name string) map[string]any {
	if name == "follow" {
		b, e := a.s.Follow(ctx, records.Cursor{}, 127)
		check(e)
		return map[string]any{"count": len(b.Records), "hash": canon(b.Records), "segment": b.Next.Segment, "row": b.Next.Row, "expired": b.Expired}
	}
	p, e := a.s.Scan(ctx, query(name))
	if errors.Is(e, tinystore.ErrLimit) {
		return map[string]any{"error": "limit"}
	}
	check(e)
	return map[string]any{"count": len(p.Records), "hash": canon(p.Records), "more": p.More, "from": p.Next.From.UnixNano(), "to": p.Next.To.UnixNano()}
}
func rss() int64 {
	b, e := os.ReadFile("/proc/self/status")
	if e != nil {
		return 0
	}
	for _, l := range strings.Split(string(b), "\n") {
		var n int64
		if _, e := fmt.Sscanf(l, "VmRSS: %d kB", &n); e == nil {
			return n
		}
	}
	return 0
}
func main() {
	mode := flag.String("mode", "verify", "")
	db := flag.String("db", "", "")
	name := flag.String("case", "scan_full", "")
	n := flag.Int("count", 4096, "")
	iterations := flag.Int("iterations", 64, "")
	warm := flag.Int("warm", 4, "")
	cold := flag.Int("cold", 0, "")
	segment := flag.Int64("segment", 0, "")
	cursorRow := flag.Int("row", 0, "")
	cursorLimit := flag.Int("limit", 127, "")
	out := flag.String("output", "", "")
	corpus := flag.String("corpus", "", "")
	flag.Parse()
	if *mode == "export" {
		f, e := os.Create(*out)
		check(e)
		enc := json.NewEncoder(f)
		for _, r := range fixture(*n, *corpus) {
			check(enc.Encode(portable(r)))
		}
		check(f.Close())
		emit(map[string]any{"count": *n})
		return
	}
	if strings.HasPrefix(*mode, "kernel") {
		runKernels(*mode, *name, *iterations)
		return
	}
	if *mode == "storage" {
		report, e := dbstat.Read(ctx, *db)
		check(e)
		emit(report)
		return
	}
	if *mode == "fixture" {
		a := open(*db)
		batch := fixture(*n, *corpus)
		for start := 0; start < len(batch); start += 1024 {
			check(a.s.Append(ctx, batch[start:min(start+1024, len(batch))]...))
		}
		if *name == "sealed" {
			clockNow = now + 1
			_, e := a.s.Maintain(ctx)
			check(e)
		}
		a.close()
		emit(map[string]any{"count": *n, "hash": canon(batch), "input": input(batch)})
		return
	}
	a := open(*db)
	defer a.close()
	if *mode == "cursor" {
		b, e := a.s.Follow(ctx, records.Cursor{Segment: *segment, Row: *cursorRow}, *cursorLimit)
		check(e)
		emit(map[string]any{"count": len(b.Records), "hash": canon(b.Records), "segment": b.Next.Segment, "row": b.Next.Row, "expired": b.Expired})
		return
	}
	if *mode == "verify" {
		for _, c := range []string{"scan_full", "scan_filtered", "scan_search", "scan_trace", "scan_page", "scan_newest", "scan_budget", "scan_none", "scan_no_context", "follow"} {
			r := result(a, c)
			r["case"] = c
			emit(r)
		}
		return
	}
	if *mode == "snapshot" {
		if *name == "follow_walk" {
			b := engine(a, *name, nil).(records.Batch)
			emit(map[string]any{"count": len(b.Records), "hash": canon(b.Records), "segment": b.Next.Segment, "row": b.Next.Row, "expired": 0})
		} else {
			emit(result(a, *name))
		}
		return
	}
	if *mode == "lifetime" {
		if strings.HasPrefix(*name, "follow") {
			b := engine(a, *name, nil).(records.Batch)
			before := canon(b.Records)
			a.close()
			if before != canon(b.Records) {
				panic("result lifetime")
			}
			emit(map[string]any{"lifetime": "engine and connections destroyed", "count": len(b.Records), "hash": before})
		} else {
			p := engine(a, *name, nil).(records.Page)
			before := canon(p.Records)
			a.close()
			if before != canon(p.Records) {
				panic("result lifetime")
			}
			emit(map[string]any{"lifetime": "engine and connections destroyed", "count": len(p.Records), "hash": before})
		}
		return
	}
	if *mode == "guards" {
		r := fixture(1, "")[0]
		r.At = time.Unix(0, now+601000000000)
		e := a.s.Append(ctx, r)
		if !errors.Is(e, tinystore.ErrTooNew) {
			panic("future guard")
		}
		r.At = time.Unix(0, epoch)
		r.Attrs[0].Value = "invalid"
		e = a.s.Append(ctx, r)
		if !errors.Is(e, tinystore.ErrInvalid) {
			panic("JSON guard")
		}
		_, e = a.s.Scan(ctx, records.Query{Limit: 10001})
		if !errors.Is(e, tinystore.ErrInvalid) {
			panic("limit guard")
		}
		emit(map[string]any{"guards": "ok"})
		return
	}
	if *mode == "late" {
		batch := fixture(4, "")
		for i := range batch {
			batch[i].At = time.Unix(0, epoch-20000000000+int64(i)*1000000).UTC()
		}
		check(a.s.Append(ctx, batch...))
		emit(map[string]any{"count": len(batch)})
		return
	}
	if *mode == "verify-walk" {
		for _, newest := range []bool{false, true} {
			q := query("scan_page")
			q.Newest = newest
			var all []records.Record
			pages := 0
			for {
				p, e := a.s.Scan(ctx, q)
				check(e)
				all = append(all, p.Records...)
				pages++
				if !p.More {
					break
				}
				if pages > 1000 {
					panic("pagination did not advance")
				}
				q = p.Next
			}
			emit(map[string]any{"walk": "scan", "newest": newest, "count": len(all), "pages": pages, "hash": canon(all)})
		}
		var all []records.Record
		cursor := records.Cursor{}
		batches := 0
		for {
			b, e := a.s.Follow(ctx, cursor, 127)
			check(e)
			all = append(all, b.Records...)
			batches++
			if len(b.Records) == 0 {
				break
			}
			if batches > 1000 {
				panic("follow did not advance")
			}
			cursor = b.Next
		}
		emit(map[string]any{"walk": "follow", "count": len(all), "batches": batches, "hash": canon(all), "segment": cursor.Segment, "row": cursor.Row})
		return
	}
	batch := fixture(256, *corpus)
	if *cold != 0 {
		var elapsed time.Duration
		var mallocs, allocated uint64
		for i := 0; i < *iterations; i++ {
			a.close()
			a = open(*db)
			var before, after runtime.MemStats
			runtime.ReadMemStats(&before)
			start := time.Now()
			engine(a, *name, batch)
			elapsed += time.Since(start)
			runtime.ReadMemStats(&after)
			mallocs += after.Mallocs - before.Mallocs
			allocated += after.TotalAlloc - before.TotalAlloc
		}
		a.close()
		emit(map[string]any{"language": "go", "case": *name, "iterations": *iterations, "cold": true, "setup_excluded": true, "ns_per_op": float64(elapsed.Nanoseconds()) / float64(*iterations), "allocations_per_op": float64(mallocs) / float64(*iterations), "allocated_bytes_per_op": float64(allocated) / float64(*iterations)})
		return
	}
	for i := 0; i < *warm && *name != "seal"; i++ {
		engine(a, *name, batch)
	}
	runtime.GC()
	var before, after runtime.MemStats
	runtime.ReadMemStats(&before)
	start := time.Now()
	for i := 0; i < *iterations; i++ {
		engine(a, *name, batch)
	}
	elapsed := time.Since(start)
	runtime.ReadMemStats(&after)
	retained := []any{}
	if *mode == "memory" {
		for i := 0; i < *iterations; i++ {
			retained = append(retained, engine(a, *name, batch))
		}
		runtime.GC()
	}
	emit(map[string]any{"language": "go", "case": *name, "iterations": *iterations, "ns_per_op": float64(elapsed.Nanoseconds()) / float64(*iterations), "allocations_per_op": float64(after.Mallocs-before.Mallocs) / float64(*iterations), "allocated_bytes_per_op": float64(after.TotalAlloc-before.TotalAlloc) / float64(*iterations), "rss_kib": rss(), "retained": len(retained), "heap_alloc": after.HeapAlloc})
	runtime.KeepAlive(retained)
}

type Payload struct {
	Bytes []byte
	Words []uint64
}
type Case struct {
	Name       string
	Units      int
	InputBytes int
	InputHash  uint64
	Run        func() Payload
}

func fingerprintBytes(b []byte, params ...uint64) uint64 {
	v := uint64(14695981039346656037)
	for _, p := range params {
		for i := 0; i < 8; i++ {
			v = (v ^ uint64(byte(p>>uint(8*i)))) * 1099511628211
		}
	}
	for _, x := range b {
		v = (v ^ uint64(x)) * 1099511628211
	}
	return v
}
func fingerprintWords(w []uint64, params ...uint64) uint64 {
	b := []byte{}
	for _, v := range w {
		b = binary.LittleEndian.AppendUint64(b, v)
	}
	return fingerprintBytes(b, params...)
}
func appendString(b []byte, s string) []byte {
	b = binary.AppendUvarint(b, uint64(len(s)))
	return append(b, s...)
}
func appendFields(b []byte, fs []records.Field) []byte {
	b = binary.AppendUvarint(b, uint64(len(fs)))
	for _, f := range fs {
		b = appendString(appendString(b, f.Key), f.Value)
	}
	return b
}
func appendHeadRecord(b []byte, r *records.Record) []byte {
	b = binary.AppendVarint(b, r.At.UnixNano())
	b = appendString(b, r.Name)
	p := byte(0)
	if r.Level != nil {
		p |= 1
	}
	if r.Body != nil {
		p |= 2
	}
	if r.TraceID != (records.TraceID{}) {
		p |= 4
	}
	if r.SpanID != (records.SpanID{}) {
		p |= 8
	}
	if len(r.Context) > 0 {
		p |= 16
	}
	b = append(b, p)
	if r.Level != nil {
		b = binary.AppendVarint(b, int64(*r.Level))
	}
	if r.Body != nil {
		b = appendString(b, *r.Body)
	}
	if p&4 != 0 {
		b = append(b, r.TraceID[:]...)
	}
	if p&8 != 0 {
		b = append(b, r.SpanID[:]...)
	}
	return appendFields(appendFields(b, r.Context), r.Attrs)
}
func allCases() []Case {
	cs := recordsCases()
	batch := fixture(1024, "")
	cs = append(cs, Case{Name: "head/serialize", Units: 1024, Run: func() Payload {
		b := []byte{}
		for i := range batch {
			b = appendHeadRecord(b, &batch[i])
		}
		return Payload{Bytes: b}
	}})
	for _, typed := range []bool{false, true} {
		name := "sort/stable"
		if typed {
			name = "sort/arrival_key"
		}
		cs = append(cs, Case{Name: name, Units: 1024, Run: func() Payload {
			ids := make([]uint64, len(batch))
			for i := range ids {
				ids[i] = uint64(i)
			}
			if typed {
				slices.SortFunc(ids, func(a, b uint64) int {
					return cmp.Or(cmp.Compare(batch[a].At.UnixNano(), batch[b].At.UnixNano()), cmp.Compare(a, b))
				})
			} else {
				sort.SliceStable(ids, func(i, j int) bool { return batch[ids[i]].At.Before(batch[ids[j]].At) })
			}
			return Payload{Words: ids}
		}})
	}
	return cs
}
func runKernels(mode, name string, iterations int) {
	for _, c := range allCases() {
		if name != "all" && name != c.Name {
			continue
		}
		p := c.Run()
		b := p.Bytes
		if b == nil {
			for _, v := range p.Words {
				b = binary.LittleEndian.AppendUint64(b, v)
			}
		}
		if mode == "kernel-verify" {
			emit(map[string]any{"case": c.Name, "units": c.Units, "hex": hex.EncodeToString(b)})
			continue
		}
		runtime.GC()
		var before, after runtime.MemStats
		runtime.ReadMemStats(&before)
		start := time.Now()
		for i := 0; i < iterations; i++ {
			payloadSink = c.Run()
		}
		elapsed := time.Since(start)
		runtime.ReadMemStats(&after)
		emit(map[string]any{"language": "go", "case": c.Name, "iterations": iterations, "ns_per_op": float64(elapsed.Nanoseconds()) / float64(iterations), "allocations_per_op": float64(after.Mallocs-before.Mallocs) / float64(iterations), "allocated_bytes_per_op": float64(after.TotalAlloc-before.TotalAlloc) / float64(iterations)})
	}
}
