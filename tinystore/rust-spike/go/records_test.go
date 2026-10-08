package main

import (
	"math"
	"slices"
	"testing"
)

func TestRecordsWidthRoundTripAndTruncation(t *testing.T) {
	for _, width := range []uint{1, 7, 17, 64} {
		values := recordsFixtureWidth(width)
		encoded := recordsEncodeWidth(values, width)
		if len(encoded) != (len(values)*int(width)+7)/8 {
			t.Fatalf("width %d length %d", width, len(encoded))
		}
		for _, decoder := range []func([]byte, int, uint) ([]uint64, bool){recordsDecodeWidth, recordsDecodeWidthWord} {
			decoded, invalid := decoder(encoded, len(values), width)
			if invalid || !slices.Equal(decoded, values) {
				t.Fatalf("width %d round trip", width)
			}
			if _, invalid = decoder(encoded[:len(encoded)-1], len(values), width); !invalid {
				t.Fatalf("width %d truncation accepted", width)
			}
		}
	}
	if len(recordsEncodeWidth([]uint64{math.MaxUint64}, 0)) != 0 {
		t.Fatal("width zero emitted bytes")
	}
	if values, invalid := recordsDecodeWidth(nil, 1, 0); invalid || !slices.Equal(values, []uint64{0}) {
		t.Fatal("width zero decode")
	}
}

func TestRecordsRiceRoundTripEscapeAndTruncation(t *testing.T) {
	for _, escapes := range []bool{false, true} {
		values := recordsFixtureRice(escapes)
		encoded := recordsEncodeRice(values, 2)
		var total uint64
		for _, value := range values {
			total += riceBits(value, 2)
		}
		if uint64(len(encoded)) != (total+7)/8 {
			t.Fatal("rice code length")
		}
		for _, decoder := range []func([]byte, int, uint) ([]uint64, bool){recordsDecodeRice, recordsDecodeRiceWord} {
			decoded, invalid := decoder(encoded, len(values), 2)
			if invalid || !slices.Equal(decoded, values) {
				t.Fatalf("rice escapes %v round trip", escapes)
			}
			if _, invalid = decoder(encoded[:len(encoded)-1], len(values), 2); !invalid {
				t.Fatal("rice truncation accepted")
			}
			trailing := append(slices.Clone(encoded), 0)
			if _, invalid = decoder(trailing, len(values), 2); !invalid {
				t.Fatal("rice trailing byte accepted")
			}
		}
	}
	for _, check := range []struct{ value, bits uint64 }{{127, 34}, {128, 96}, {math.MaxUint64, 96}} {
		if riceBits(check.value, 2) != check.bits {
			t.Fatalf("rice bits for %d", check.value)
		}
	}
}

func TestRecordsShift64MixedAlignment(t *testing.T) {
	widths := []uint{1, 64, 7, 64, 17, 0, 64}
	values := []uint64{1, math.MaxUint64, 0x65, 0x0123456789abcdef, 0x1abcd, 0, 0xfedcba9876543210}
	writer := bitWriter{}
	for i, value := range values {
		writer.write(value, widths[i])
	}
	encoded := writer.finish()
	reader := bitReader{data: encoded}
	word := recordsWordReader{data: encoded}
	for i, value := range values {
		if reader.read(widths[i]) != value || word.read(widths[i]) != value || reader.at != word.at {
			t.Fatalf("mixed alignment item %d", i)
		}
	}
	if reader.short || word.short || reader.consumed() != word.consumed() {
		t.Fatal("mixed alignment status")
	}
}

func TestRecordsFailedReadPreservesConsumedBits(t *testing.T) {
	for length := 0; length < 16; length++ {
		data := make([]byte, length)
		for i := range data {
			data[i] = 0xa5
		}
		reader := bitReader{data: data}
		word := recordsWordReader{data: data}
		for _, width := range []uint{7, 64, 1, 17, 0, 64} {
			if reader.read(width) != word.read(width) || reader.at != word.at || reader.short != word.short ||
				reader.consumed() != word.consumed() {
				t.Fatalf("failed read status length %d width %d", length, width)
			}
		}
	}
}

func TestRecordsSignedRangeWrapping(t *testing.T) {
	limits := []int64{math.MinInt64, -1, 0, 1, math.MaxInt64}
	for _, start := range limits {
		for _, end := range limits {
			if advance(start, distance(start, end)) != end {
				t.Fatalf("wrapping start %d end %d", start, end)
			}
		}
	}
	if distance(math.MinInt64, math.MaxInt64) != math.MaxUint64 || advance(math.MaxInt64, 1) != math.MinInt64 {
		t.Fatal("full signed range wrapping")
	}
	if zigzag(math.MinInt64) != math.MaxUint64 || zigzag(math.MaxInt64) != math.MaxUint64-1 ||
		zigzag(-1) != 1 || zigzag(1) != 2 {
		t.Fatal("zigzag sign folding")
	}
}
