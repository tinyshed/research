package main

import (
	"math"
	"testing"
)

func TestPrepareSamplesKeepsLastArrivalBits(t *testing.T) {
	input := []Sample{{At: 2, Value: 1}, {At: 1, Value: math.Float64frombits(0x7ff8000000000042)}, {At: 2, Value: math.Float64frombits(0x8000000000000000)}}
	for _, typed := range []bool{false, true} {
		out := prepareSamples(input, typed)
		if len(out) != 2 || out[0].At != 1 || out[1].At != 2 || math.Float64bits(out[1].Value) != 0x8000000000000000 {
			t.Fatal("last arrival or ordering lost")
		}
	}
}

func TestPrepareSamplesFixtures(t *testing.T) {
	for _, n := range []int{240, 4096} {
		for _, kind := range []string{"ordered", "shuffled", "duplicates"} {
			input := ingestFixture(n, kind)
			for _, typed := range []bool{false, true} {
				out := prepareSamples(input, typed)
				want := n
				if kind == "duplicates" {
					want -= n / 4
				}
				if len(out) != want {
					t.Fatal("unique count")
				}
				for i, p := range out {
					if i > 0 && out[i-1].At >= p.At {
						t.Fatal("output not strictly sorted")
					}
					for j := len(input) - 1; j >= 0; j-- {
						if input[j].At == p.At {
							if math.Float64bits(input[j].Value) != math.Float64bits(p.Value) {
								t.Fatal("last bits changed")
							}
							break
						}
					}
				}
			}
		}
	}
}
