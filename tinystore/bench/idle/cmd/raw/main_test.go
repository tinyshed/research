package main

import (
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"testing"
)

func TestRawKeepsEverySampleAndItsSeries(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "input.jsonl")
	row := series{Metric: map[string]string{"name": "sample"}, Values: []float64{math.Copysign(0, -1), 1.25},
		Timestamps: []int64{-1, 3}}
	encoded, err := json.Marshal(row)
	if err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(path, append(encoded, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
	result, err := writeRaw(path, filepath.Join(root, "raw"))
	if err != nil {
		t.Fatal(err)
	}
	if result.Samples != 2 || result.Series != 1 || result.DataBytes != 32 || result.IndexBytes == 0 {
		t.Fatalf("bad footprint: %+v", result)
	}
	index, err := os.ReadFile(filepath.Join(root, "raw", "series.json"))
	if err != nil {
		t.Fatal(err)
	}
	var entries []entry
	if err = json.Unmarshal(index, &entries); err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Labels["name"] != "sample" || entries[0].Count != 2 || entries[0].Offset != 0 {
		t.Fatalf("lost series: %+v", entries)
	}
}
