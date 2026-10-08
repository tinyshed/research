package main

import (
	"slices"
	"testing"
)

func TestReadersKeepExactWordsAndRejectTruncation(t *testing.T) {
	for _, width := range []uint{0, 1, 7, 17, 64} {
		values := recordsFixtureWidth(width)
		encoded := recordsEncodeWidth(values, width)
		for _, decode := range []func([]byte, int, uint) ([]uint64, bool){recordsDecodeWidth, recordsDecodeWidthWord} {
			got, bad := decode(encoded, len(values), width)
			if bad || !slices.Equal(got, values) {
				t.Fatalf("width %d round trip", width)
			}
			if len(encoded) > 0 {
				_, bad = decode(encoded[:len(encoded)-1], len(values), width)
				if !bad {
					t.Fatal("truncation accepted")
				}
			}
		}
	}
	for _, escapes := range []bool{false, true} {
		values := recordsFixtureRice(escapes)
		encoded := recordsEncodeRice(values, 2)
		for _, decode := range []func([]byte, int, uint) ([]uint64, bool){recordsDecodeRice, recordsDecodeRiceWord} {
			got, bad := decode(encoded, len(values), 2)
			if bad || !slices.Equal(got, values) {
				t.Fatal("rice round trip")
			}
			_, bad = decode(encoded[:len(encoded)-1], len(values), 2)
			if !bad {
				t.Fatal("rice truncation accepted")
			}
			_, bad = decode(append(slices.Clone(encoded), 0), len(values), 2)
			if !bad {
				t.Fatal("rice trailing bytes accepted")
			}
		}
	}
}
func TestSortCandidatesKeepArrivalAtEqualTimes(t *testing.T) {
	var baseline []uint64
	for _, c := range allCases() {
		if c.Name == "sort/stable" {
			baseline = c.Run().Words
		}
		if c.Name == "sort/arrival_key" && !slices.Equal(baseline, c.Run().Words) {
			t.Fatal("arrival ordering differs")
		}
	}
}
