// Public-API production fixture and independent IEEE-bit readback verifier.
package main

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"sort"
	"time"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/internal/dbstat"
	"github.com/tinyshed/tinystore/internal/sqlite"
	"github.com/tinyshed/tinystore/metrics"
)

var ctx = context.Background()

const epoch int64 = 1767225600000

type corpusLine struct {
	Metric     map[string]string `json:"metric"`
	Values     []float64         `json:"values"`
	Timestamps []int64           `json:"timestamps"`
}
type seriesInfo struct {
	ID     int64          `json:"id"`
	Name   string         `json:"name"`
	Kind   metrics.Kind   `json:"kind"`
	Labels metrics.Labels `json:"labels"`
	Count  int            `json:"count"`
	From   int64          `json:"from"`
	To     int64          `json:"to"`
	SHA256 string         `json:"sha256"`
}
type manifest struct {
	Format      string       `json:"format"`
	Source      string       `json:"source_commit"`
	Dataset     string       `json:"dataset"`
	CorpusSHA   string       `json:"corpus_sha256"`
	Now         int64        `json:"now"`
	Count       int64        `json:"sample_count"`
	Head        int64        `json:"head_sample_count"`
	Series      []seriesInfo `json:"series"`
	BuildNS     int64        `json:"build_ns"`
	VerifyNS    int64        `json:"verify_ns"`
	Maintenance int          `json:"maintenance_calls"`
}

func check(e error) {
	if e != nil {
		panic(e)
	}
}
func digest(p []metrics.Sample) string {
	h := sha256.New()
	var b [16]byte
	for _, x := range p {
		binary.LittleEndian.PutUint64(b[:8], uint64(x.At))
		binary.LittleEndian.PutUint64(b[8:], math.Float64bits(x.Value))
		h.Write(b[:])
	}
	return hex.EncodeToString(h.Sum(nil))
}
func options() metrics.Options {
	return metrics.Options{Retention: 365 * 24 * time.Hour, MaxSeries: 100000, MaxHeadSamples: 1 << 20, MaxHeadBytes: 16 << 20, MaxBatchSamples: 100000, MaxBatchBytes: 64 << 20, MaxReaders: 1, MaxConcurrentReads: 1, MaintenanceSeries: 64}
}
func open(dir string, now int64) (*tinystore.Store, *metrics.Store) {
	r, e := tinystore.Open(ctx, dir, tinystore.Options{Manual: true, Clock: func() time.Time { return time.UnixMilli(now) }})
	check(e)
	s, e := metrics.Open(ctx, r, options())
	check(e)
	return r, s
}
func emit(v any) { check(json.NewEncoder(os.Stdout).Encode(v)) }
func main() {
	mode := flag.String("mode", "build", "build or verify or physical")
	out := flag.String("out", "", "store directory")
	db := flag.String("db", "", "database file for physical mode")
	corpus := flag.String("corpus", "", "normalized public series JSONL")
	dataset := flag.String("dataset", "regular", "regular, irregular, nonsparse, edge, or corpus label")
	limit := flag.Int("series-limit", 0, "first N canonical corpus series, 0 all")
	n := flag.Int("samples", 15361, "samples per deterministic series")
	series := flag.Int("series", 32, "deterministic series count")
	source := flag.String("source", "e307c48a40126aad0e2873b6bf3aaedef8115483", "pinned production source")
	flag.Parse()
	if *out == "" && *db == "" {
		panic("--out required")
	}
	if *mode == "physical" {
		path := *db
		if path == "" {
			path = filepath.Join(*out, "metrics.db")
		}
		o, e := dbstat.Read(ctx, path)
		check(e)
		emit(o)
		return
	}
	if *mode == "verify" {
		b, e := os.ReadFile(filepath.Join(*out, "manifest.json"))
		check(e)
		var m manifest
		check(json.Unmarshal(b, &m))
		verify(*out, &m)
		emit(m)
		return
	}
	if _, e := os.Stat(filepath.Join(*out, "metrics.db")); e == nil {
		panic("refuse existing fixture")
	}
	check(os.MkdirAll(*out, 0755))
	m := manifest{Format: "tinystore-storage-input-v1", Source: *source, Dataset: *dataset}
	// Corpus is streamed twice: extrema and hash first, then one series at a time.
	m.Now = epoch + int64(*n)*13000 + 1000
	if *corpus != "" {
		m.Now = 0
		f, e := os.Open(*corpus)
		check(e)
		h := sha256.New()
		sc := bufio.NewScanner(f)
		sc.Buffer(make([]byte, 1<<20), 64<<20)
		rows := 0
		for sc.Scan() {
			h.Write(sc.Bytes())
			h.Write([]byte{'\n'})
			if *limit == 0 || rows < *limit {
				var c corpusLine
				check(json.Unmarshal(sc.Bytes(), &c))
				for _, at := range c.Timestamps {
					if at+1000 > m.Now {
						m.Now = at + 1000
					}
				}
			}
			rows++
		}
		check(sc.Err())
		check(f.Close())
		m.CorpusSHA = hex.EncodeToString(h.Sum(nil))
	}
	r, s := open(*out, m.Now)
	start := time.Now()
	add := func(b metrics.Batch) {
		sort.SliceStable(b.Samples, func(i, j int) bool { return b.Samples[i].At < b.Samples[j].At })
		if len(b.Samples) == 0 {
			return
		}
		info := seriesInfo{Name: b.Series.Name, Kind: b.Series.Kind, Labels: b.Series.Labels, Count: len(b.Samples), From: b.Samples[0].At, To: b.Samples[len(b.Samples)-1].At + 1, SHA256: digest(b.Samples)}
		for begin := 0; begin < len(b.Samples); {
			end := min(begin+7680, len(b.Samples))
			check(s.Ingest(ctx, []metrics.Batch{{Series: b.Series, Samples: b.Samples[begin:end]}}))
			_, e := s.Maintain(ctx)
			check(e)
			m.Maintenance++
			begin = end
		}
		m.Series = append(m.Series, info)
		m.Count += int64(len(b.Samples))
	}
	if *corpus != "" {
		f, e := os.Open(*corpus)
		check(e)
		sc := bufio.NewScanner(f)
		sc.Buffer(make([]byte, 1<<20), 64<<20)
		rows := 0
		for sc.Scan() {
			if *limit > 0 && rows >= *limit {
				break
			}
			rows++
			var c corpusLine
			check(json.Unmarshal(sc.Bytes(), &c))
			if len(c.Values) != len(c.Timestamps) {
				panic("corpus vector lengths")
			}
			b := metrics.Batch{Series: metrics.Series{Kind: metrics.Gauge, Labels: metrics.Labels{}}, Samples: make([]metrics.Sample, len(c.Values))}
			for k, v := range c.Metric {
				if k == "__name__" {
					b.Series.Name = v
				} else {
					b.Series.Labels[k] = v
				}
			}
			for i, v := range c.Values {
				b.Samples[i] = metrics.Sample{At: c.Timestamps[i], Value: v}
			}
			add(b)
		}
		check(sc.Err())
		check(f.Close())
	} else {
		for id := 0; id < *series; id++ {
			kind := metrics.Gauge
			if id%4 == 3 {
				kind = metrics.Counter
			}
			b := metrics.Batch{Series: metrics.Series{Name: "layout_" + *dataset, Kind: kind, Labels: metrics.Labels{"id": fmt.Sprint(id)}}, Samples: make([]metrics.Sample, *n)}
			at := epoch
			bits := uint64(id + 1)
			for i := range b.Samples {
				at += 1000
				if *dataset == "irregular" {
					at += int64((i*17+id)%13) * 1000
				}
				v := float64((i/240 + id) % 17)
				switch *dataset {
				case "nonsparse":
					bits ^= bits << 13
					bits ^= bits >> 7
					bits ^= bits << 17
					v = math.Float64frombits((bits & ((1 << 52) - 1)) | uint64(1023)<<52)
				case "irregular":
					v = float64((i*17+id)%997) / 10
				case "edge":
					edge := []uint64{0, 1 << 63, 0x7ff0000000000000, 0xfff0000000000000, 0x7ff8000000000042, 0x7ff0000000000001, 1, 0x7fefffffffffffff, 0x3ff0000000000000}
					v = math.Float64frombits(edge[(i+id)%len(edge)])
				}
				if *dataset == "edge" && kind == metrics.Counter {
					v = float64(i % 313)
				}
				b.Samples[i] = metrics.Sample{At: at, Value: v}
			}
			add(b)
		}
	}
	for range (len(m.Series)+63)/64 + 2 {
		_, e := s.Maintain(ctx)
		check(e)
		m.Maintenance++
	}
	check(r.Close(ctx))
	m.BuildNS = time.Since(start).Nanoseconds()
	f, e := sqlite.Open(ctx, filepath.Join(*out, "metrics.db"), sqlite.Config{Readers: 1})
	check(e)
	check(f.Lookup(ctx, func(rd sqlite.Reader) error {
		rows, e := rd.QueryContext(ctx, "select id from series order by id")
		if e != nil {
			return e
		}
		i := 0
		for rows.Next() {
			if i >= len(m.Series) {
				panic("registry count")
			}
			check(rows.Scan(&m.Series[i].ID))
			i++
		}
		check(rows.Err())
		check(rows.Close())
		if i != len(m.Series) {
			panic("registry count")
		}
		return sqlite.QueryRow(ctx, rd, "select coalesce(sum(head_count),0) from series_state").Scan(&m.Head)
	}))
	check(f.Close())
	verify(*out, &m)
	b, e := json.MarshalIndent(m, "", "  ")
	check(e)
	check(os.WriteFile(filepath.Join(*out, "manifest.json"), append(b, '\n'), 0644))
	emit(m)
}
func verify(dir string, m *manifest) {
	start := time.Now()
	r, s := open(dir, m.Now)
	for _, v := range m.Series {
		results, e := s.Read(ctx, metrics.Range{Name: v.Name, Match: v.Labels, From: v.From, To: v.To})
		check(e)
		if len(results) != 1 || len(results[0].Samples) != v.Count || digest(results[0].Samples) != v.SHA256 {
			panic(fmt.Sprintf("Go public readback mismatch %s %v", v.Name, v.Labels))
		}
	}
	check(r.Close(ctx))
	m.VerifyNS = time.Since(start).Nanoseconds()
}
