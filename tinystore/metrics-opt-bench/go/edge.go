package main

import (
	"math"
	"strconv"

	"github.com/tinyshed/tinystore/metrics"
)

func edgeBatches() []metrics.Batch {
	var out []metrics.Batch
	for id := range 12 {
		s := makeSeries("edge", id, metrics.Gauge)
		s.Labels["group"] = "edge"
		if id == 10 {
			s.Name = "edge_counter"
			s.Kind = metrics.Counter
		}
		p := make([]metrics.Sample, 481)
		at := epoch
		for i := range p {
			v := float64((i*17+id)%997) / 10
			switch id {
			case 0:
				v = 42
			case 1:
				v = float64(i/20%7) * 10
			case 3:
				n := (uint64(i) + 1) * 0x9e3779b97f4a7c15
				v = math.Float64frombits(0x3ff0000000000000 | n&0x000fffffffffffff)
			case 4:
				v = float64(i % 31)
			case 5:
				v = math.Float64frombits(uint64(i%2) << 63)
			case 6:
				n := uint64(i%7 + 1)
				if i%2 == 1 {
					n |= 1 << 63
				}
				v = math.Float64frombits(n)
			case 7:
				v = []float64{math.Float64frombits(0x7ff8000000000042), math.Inf(1), math.Inf(-1), 0, math.Copysign(0, -1), 1}[i%6]
			case 8:
				v = []float64{math.MaxFloat64, math.MaxFloat64, -math.MaxFloat64}[i%3]
			case 9:
				v = []float64{1e300, 1, -1e300}[i%3]
			case 10:
				v = float64(i % 64)
			case 11:
				v = float64(i) / 1000
			}
			p[i] = metrics.Sample{At: at, Value: v}
			step := int64(1)
			if id == 4 {
				step = []int64{1, 2, 1, 5}[i%4]
			}
			at += step
		}
		s.Labels["id"] = strconv.Itoa(id)
		out = append(out, metrics.Batch{Series: s, Samples: p})
	}
	return out
}
