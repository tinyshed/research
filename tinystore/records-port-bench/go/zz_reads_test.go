package records

import (
	"bufio"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"slices"
	"testing"
	"time"
)

// readsFixture is 65,536 requests of one stream, twenty milliseconds apart:
// a request id each, one of sixteen routes, a status, a duration, a host in
// their context, and a trace one in sixty-four
func readsFixture(base time.Time) []Record {
	routes := make([]string, 16)
	for i := range routes {
		routes[i] = fmt.Sprintf(`"/route/%d"`, i)
	}
	records := make([]Record, 65_536)
	for i := range records {
		x := uint64(i)*0x9e3779b97f4a7c15 + 1
		body := fmt.Sprintf("GET /notes/%d served", i%97)
		level := slog.LevelInfo
		if i%16 == 5 {
			level = slog.LevelWarn
		}
		name := "request"
		if i%8 == 3 {
			name = "cache"
		}
		r := Record{
			At: base.Add(time.Duration(i) * 20 * time.Millisecond), Stream: "api", Name: name, Level: &level, Body: &body,
			Attrs: []Field{
				{Key: "ms", Value: fmt.Sprint(i % 400)},
				{Key: "request", Value: fmt.Sprintf(`"%016x%016x"`, x, x^0x5bd1e995)},
				{Key: "route", Value: routes[(x>>7)%16]},
				{Key: "status", Value: []string{"200", "200", "404", "500"}[(x>>13)%4]},
			},
			Context: []Field{{Key: "host", Value: fmt.Sprintf(`"web-%d"`, i%4)}, {Key: "service", Value: `"api"`}},
		}
		if i%64 == 0 {
			for b := range r.TraceID {
				r.TraceID[b] = byte(x >> (b % 8 * 8))
			}
			r.TraceID[15] = byte(i >> 6)
			r.TraceID[14] = byte(i >> 14)
			r.SpanID = SpanID{1, 2, 3, 4, 5, 6, 7, 8}
		}
		records[i] = r
	}
	return records
}

// TestCompareReads seals the fixture and times three reads over it: an exact
// filter, a single trace and an absent context value. With TINYSTORE_READS it
// first writes the fixture there for the Rust port. A scratch measurement,
// never committed.
func TestCompareReads(t *testing.T) {
	if os.Getenv("TINYSTORE_SPIKE") != "1" {
		t.Skip("a measurement: set TINYSTORE_SPIKE=1")
	}
	s := openRecords(t)
	base := testNow.Add(-30 * time.Minute)
	records := readsFixture(base)
	if path := os.Getenv("TINYSTORE_READS"); path != "" {
		dumpReads(t, path, records, base)
	}
	s.append(t, records...)
	s.clock.advance(2 * time.Hour)
	for range 8 {
		if _, err := s.Maintain(t.Context()); err != nil {
			t.Fatal(err)
		}
	}
	trace := records[64*500].TraceID
	reads := []struct {
		name  string
		query Query
	}{
		{"exact", Query{Names: []string{"request"}, Attrs: []Field{{Key: "route", Value: `"/route/7"`}, {Key: "status", Value: "500"}}, Limit: 10000}},
		{"trace", Query{TraceID: trace, Limit: 10000}},
		{"absent", Query{Context: []Field{{Key: "host", Value: `"nowhere"`}}, Limit: 10000}},
	}
	for _, read := range reads {
		var found int
		run := func() {
			found = 0
			for _, err := range s.All(t.Context(), read.query) {
				if err != nil {
					t.Fatal(err)
				}
				found++
			}
		}
		for range 3 {
			run()
		}
		var runs []time.Duration
		for range 15 {
			started := time.Now()
			for range 10 {
				run()
			}
			runs = append(runs, time.Since(started)/10)
		}
		slices.Sort(runs)
		fmt.Printf("go   read=%-7s found=%-5d median=%.3fms min=%.3fms\n", read.name, found,
			float64(runs[7].Microseconds())/1000, float64(runs[0].Microseconds())/1000)
	}
}

func dumpReads(t *testing.T, path string, records []Record, base time.Time) {
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	w := bufio.NewWriterSize(file, 1<<20)
	enc := json.NewEncoder(w)
	enc.SetEscapeHTML(false)
	type dumped struct {
		At      int64       `json:"at"`
		Name    string      `json:"name"`
		Level   int         `json:"level"`
		Body    string      `json:"body"`
		Fields  [][2]string `json:"fields"`
		Context [][2]string `json:"context"`
		Trace   string      `json:"trace,omitempty"`
		Span    string      `json:"span,omitempty"`
	}
	for _, r := range records {
		d := dumped{At: r.At.Sub(base).Nanoseconds(), Name: r.Name, Level: int(*r.Level), Body: *r.Body}
		for _, field := range r.Attrs {
			d.Fields = append(d.Fields, [2]string{field.Key, field.Value})
		}
		for _, field := range r.Context {
			d.Context = append(d.Context, [2]string{field.Key, field.Value})
		}
		if r.TraceID != (TraceID{}) {
			d.Trace, d.Span = hex.EncodeToString(r.TraceID[:]), hex.EncodeToString(r.SpanID[:])
		}
		if err := enc.Encode(d); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.Flush(); err != nil {
		t.Fatal(err)
	}
	_ = file.Close()
}
