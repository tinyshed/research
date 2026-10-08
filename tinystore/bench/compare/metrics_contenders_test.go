package main

import (
	"testing"
	"time"
)

func TestTinyStoreMetricAdapterKeepsNamesAndExactLabels(t *testing.T) {
	opened, err := openTinyStoreMetrics(t.Context(), t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	store := opened.(*tinyStoreMetrics)
	t.Cleanup(func() {
		if err := store.close(); err != nil {
			t.Error(err)
		}
	})
	at := time.Now().Add(-time.Minute).UnixMilli()
	cpu := map[string]string{"__name__": "cpu", "host": "web-1", "region": "east"}
	window := []seriesSamples{
		{Labels: cpu, Times: []int64{at, at + 1, at + 2}, Values: []float64{1, 2, 3}},
		{Labels: map[string]string{"__name__": "memory", "host": "web-1", "region": "east"},
			Times: []int64{at, at + 1, at + 2}, Values: []float64{10, 20, 30}},
		{Labels: map[string]string{"__name__": "cpu", "host": "web-2", "region": "west"},
			Times: []int64{at, at + 1}, Values: []float64{4, 5}},
	}
	if err := store.ingest(t.Context(), window); err != nil {
		t.Fatal(err)
	}
	if got, err := store.readSeries(t.Context(), cpu, at, at+2); err != nil || got != 2 {
		t.Fatalf("one cpu series in [from,to): got %d, %v; want 2", got, err)
	}
	for _, query := range []struct {
		name, value string
		want        int
	}{
		{"__name__", "cpu", 5},
		{"host", "web-1", 6},
		{"region", "east", 6},
		{"host", "missing", 0},
	} {
		if got, err := store.readMatching(t.Context(), query.name, query.value, at, at+3); err != nil || got != query.want {
			t.Errorf("%s=%s: got %d, %v; want %d", query.name, query.value, got, err, query.want)
		}
	}
	if cpu["__name__"] != "cpu" || len(cpu) != 3 {
		t.Fatalf("ingest or read changed corpus labels: %v", cpu)
	}
}
