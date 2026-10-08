package main

import (
	"testing"
	"time"
)

func TestTinyStoreRecordAdapterKeepsStreamAndTimeBounds(t *testing.T) {
	opened, err := openTinyStoreRecords(t.Context(), t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	store := opened.(*tinyStoreRecords)
	t.Cleanup(func() {
		if err := store.close(); err != nil {
			t.Error(err)
		}
	})
	at := time.Now().Add(-time.Minute).UTC()
	lines := []logLine{
		{At: at.Add(-time.Nanosecond), Stream: "api", Body: "before"},
		{At: at, Stream: "api", Body: "first"},
		{At: at.Add(time.Nanosecond), Stream: "api", Body: "second"},
		{At: at.Add(2 * time.Nanosecond), Stream: "api", Body: "end"},
		{At: at, Stream: "worker", Body: "another stream"},
	}
	if err := store.append(t.Context(), lines); err != nil {
		t.Fatal(err)
	}
	for _, query := range []struct {
		stream string
		want   int
	}{
		{"api", 2},
		{"worker", 1},
		{"missing", 0},
	} {
		if got, err := store.count(t.Context(), query.stream, at, at.Add(2*time.Nanosecond)); err != nil || got != query.want {
			t.Errorf("stream %s in [from,to): got %d, %v; want %d", query.stream, got, err, query.want)
		}
	}
}
