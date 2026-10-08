package main

import (
	"fmt"
	"math"
	"slices"
	"sort"
)

// The add/extends/sorted kernel is from metrics/ingest_input.go at
// e307c48a40126aad0e2873b6bf3aaedef8115483. Validation and labels are excluded.
type pendingSamples struct {
	samples []Sample
	byTime  map[int64]Sample
}

func (p *pendingSamples) add(point Sample) {
	if p.byTime == nil && !p.extends(point.At) {
		p.byTime = make(map[int64]Sample, len(p.samples)+1)
		for _, earlier := range p.samples {
			p.byTime[earlier.At] = earlier
		}
		p.samples = nil
	}
	if p.byTime != nil {
		p.byTime[point.At] = point
		return
	}
	p.samples = append(p.samples, point)
}

func (p *pendingSamples) extends(at int64) bool {
	return len(p.samples) == 0 || at > p.samples[len(p.samples)-1].At
}

func prepareSamples(input []Sample, typedSort bool) []Sample {
	p := pendingSamples{}
	for _, point := range input {
		p.add(point)
	}
	if p.byTime == nil {
		return p.samples
	}
	for _, point := range p.byTime {
		p.samples = append(p.samples, point)
	}
	if typedSort {
		slices.SortFunc(p.samples, func(a, b Sample) int {
			if a.At < b.At {
				return -1
			}
			if a.At > b.At {
				return 1
			}
			return 0
		})
	} else {
		sort.Slice(p.samples, func(i, j int) bool { return p.samples[i].At < p.samples[j].At })
	}
	return p.samples
}

func ingestFixture(n int, kind string) []Sample {
	input := make([]Sample, n)
	for i := range input {
		bits := uint64(i+1) * 0x9e3779b97f4a7c15
		value := math.Float64frombits(0x3ff0000000000000 | bits&0xfffffffffffff)
		if i%17 == 0 {
			value = math.Float64frombits(0x8000000000000000)
		}
		if i%29 == 0 {
			value = math.Float64frombits(0x7ff8000000000042)
		}
		input[i] = Sample{At: 1000000 + int64(i)*10, Value: value}
	}
	if kind == "duplicates" {
		for i := 3; i < n; i += 4 {
			input[i].At = input[i-1].At
		}
	}
	if kind != "ordered" {
		state := uint64(0x123456789abcdef)
		for i := n - 1; i > 0; i-- {
			state = state*6364136223846793005 + 1442695040888963407
			j := int(state % uint64(i+1))
			input[i], input[j] = input[j], input[i]
		}
	}
	return input
}

func ingestCases() []Case {
	var out []Case
	for _, n := range []int{240, 4096} {
		for _, kind := range []string{"ordered", "shuffled", "duplicates"} {
			input := ingestFixture(n, kind)
			out = append(out, Case{Name: fmt.Sprintf("ingest/%s/%d", kind, n), Units: n, InputBytes: n * 16, InputHash: fingerprintSamples(input), Run: func() Payload { return Payload{Samples: prepareSamples(input, false)} }})
			if kind != "ordered" {
				out = append(out, Case{Name: fmt.Sprintf("ingest/%s/%d/typed_sort", kind, n), Units: n, InputBytes: n * 16, InputHash: fingerprintSamples(input), Run: func() Payload { return Payload{Samples: prepareSamples(input, true)} }})
			}
		}
	}
	return out
}
