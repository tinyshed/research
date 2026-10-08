package main

import (
	"bytes"
	"encoding/binary"
	"math"
	"slices"
	"testing"
)

func TestMetricsChangesPreserveBits(t *testing.T) {
	for shape, kind := range map[string]byte{"sparse": 0, "many": 0, "decimal": 1, "raw": 2} {
		points := metricsChangeFixture(shape)
		body := changeValues(points)
		if body[1] != kind {
			t.Fatalf("%s: kind %d, want %d", shape, body[1], kind)
		}
		out := metricsReadValueBody(points[0].Value, len(points), body)
		for i := range points {
			if out[i].At != points[i].At || math.Float64bits(out[i].Value) != math.Float64bits(points[i].Value) {
				t.Fatalf("%s: sample %d lost bits", shape, i)
			}
		}
	}
	points := metricsChangeFixture("constant")
	if len(changeValues(points)) != 0 {
		t.Fatal("constant block has a body")
	}
	for _, point := range metricsReadValueBody(points[0].Value, len(points), nil) {
		if math.Float64bits(point.Value) != 0x8000000000000000 {
			t.Fatal("constant negative zero lost its sign")
		}
	}
}

func TestMetricsChangeExampleAndLimits(t *testing.T) {
	points := []Sample{{Value: 5}, {Value: 5}, {Value: 5}, {Value: 7}, {Value: 7}, {Value: 9}}
	if !bytes.Equal(changeValues(points), []byte{1, 0, 3, 4, 2, 4}) {
		t.Fatal("worked example differs")
	}
	for _, value := range []float64{math.Copysign(0, -1), 0x1p60, math.NaN(), math.Inf(1)} {
		if _, _, err := firstChange(value, 0); err == nil {
			t.Fatalf("firstChange accepted %x", math.Float64bits(value))
		}
	}
	reader := metricsBinaryReader{data: []byte{2}}
	if _, _, err := readChange(&reader, 0, (1<<60)-1); err == nil {
		t.Fatal("accepted integer outside encoding range")
	}
	reader = metricsBinaryReader{data: binary.AppendVarint(nil, math.MaxInt64)}
	if _, _, err := readChange(&reader, 0, 1); err == nil {
		t.Fatal("accepted overflowing integer")
	}
}

func TestMetricsChangesRejectMalformedData(t *testing.T) {
	for _, body := range [][]byte{nil, {3}, {0, 0}, {0, 3}, {0, 1}, {2, 1, 0}, {0, 1, 2, 0}, {0, 1, 2, 0x80}} {
		if _, err := readChanges(metricsChangeHead{Count: 3}, body); err == nil {
			t.Fatalf("accepted malformed body %x", body)
		}
	}
}

func TestMetricsPackedWidthsAndTails(t *testing.T) {
	for width := 0; width <= 64; width++ {
		for _, count := range []int{0, 1, 2, 7, 9, 63, 64, 65, 239, 240} {
			values := metricsPackedFixture(width)[:count]
			baseline := packedBits(values, width)
			if !bytes.Equal(packedBitsChunked(values, width), baseline) {
				t.Fatalf("packing width=%d count=%d differs", width, count)
			}
			for _, decode := range []func([]byte, int, int) ([]uint64, error){readPackedBits, readPackedBitsChunked} {
				out, err := decode(baseline, count, width)
				if err != nil || !slices.Equal(out, values) {
					t.Fatalf("reading width=%d count=%d differs: %v", width, count, err)
				}
			}
		}
	}
}

func TestMetricsPackedRejectMalformedData(t *testing.T) {
	for _, decode := range []func([]byte, int, int) ([]uint64, error){readPackedBits, readPackedBitsChunked} {
		for _, test := range []struct {
			data         []byte
			count, width int
		}{{nil, 1, 1}, {[]byte{0, 0}, 1, 1}, {[]byte{2}, 1, 1}, {nil, 0, 65}, {nil, 241, 0}, {nil, -1, 0}, {nil, 0, -1}} {
			if _, err := decode(test.data, test.count, test.width); err == nil {
				t.Fatalf("accepted malformed packed data: %+v", test)
			}
		}
		if out, err := decode(nil, 240, 0); err != nil || !slices.Equal(out, make([]uint64, 240)) {
			t.Fatalf("zero-width decoding differs: %v", err)
		}
	}
}
