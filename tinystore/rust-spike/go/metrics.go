package main

import (
	"encoding/binary"
	"errors"
	"fmt"
	"math"
)

// Baseline bodies come from tinyshed/tinystore at
// e307c48a40126aad0e2873b6bf3aaedef8115483. Dependency names are localized.
// metrics/values_changes.go SHA256:
// 9c09a1e34ae5399f9c6f1bdd8fa35d4c8dab54eb9808abaa251fdaa990766e0f
// metrics/residuals.go SHA256:
// fac785233af3dc4a684127afb51723928e280e27430464f666f320171ee1152a
// metrics/binary.go SHA256:
// 7207b097f924a2922a8270b4b850f280437b400ea1755bd2a5b189e5f808ec19
// metrics/head.go SHA256:
// 11b7e38b636b71b5e0cb7d8e0497915a7993017bd2b02d6d45bf94dac7f0f616

const (
	valuesChanges       byte = 1
	metricsBlockSamples      = 240
)

var metricsErrCorrupt = errors.New("corrupt metrics payload")

type metricsChangeHead struct {
	First float64
	Count int
}

// changeValues writes where a block's value changes and by how much, keeping
// the shortest of whole numbers, hundredths and raw bits:
//
//	5 5 5 7 7 9    whole numbers    at 3 by +2, 2 later by +2    01 00 03 04 02 04
func changeValues(points []Sample) []byte {
	if constantValues(points) {
		return nil
	}
	var best []byte
	for kind := range byte(3) {
		candidate := changeCandidate(points, kind)
		if candidate != nil && (best == nil || len(candidate) < len(best)) {
			best = candidate
		}
	}
	return best
}

func constantValues(points []Sample) bool {
	for _, point := range points[1:] {
		if math.Float64bits(point.Value) != math.Float64bits(points[0].Value) {
			return false
		}
	}
	return true
}

func changeCandidate(points []Sample, kind byte) []byte {
	integers, ok := changeIntegers(points, kind)
	if !ok {
		return nil
	}
	out := []byte{valuesChanges, kind}
	previous := 0
	for i := 1; i < len(points); i++ {
		if math.Float64bits(points[i].Value) == math.Float64bits(points[i-1].Value) {
			continue
		}
		out = metricsAppendCount(out, i-previous)
		previous = i
		if kind == 2 {
			out = binary.LittleEndian.AppendUint64(out, math.Float64bits(points[i].Value))
		} else {
			out = binary.AppendVarint(out, integers[i]-integers[i-1])
		}
	}
	return out
}

// changeIntegers are the values counted in the kind's unit, when every one
// divides back to its exact bits; raw bits need none.
func changeIntegers(points []Sample, kind byte) ([]int64, bool) {
	integers := make([]int64, len(points))
	if kind == 2 {
		return integers, true
	}
	factor := changeFactor(kind)
	for i, point := range points {
		q := math.Round(point.Value * factor)
		if math.IsNaN(q) || math.IsInf(q, 0) || math.Abs(q) >= 0x1p60 {
			return nil, false
		}
		integers[i] = int64(q)
		if math.Float64bits(float64(integers[i])/factor) != math.Float64bits(point.Value) {
			return nil, false
		}
	}
	return integers, true
}

func changeFactor(kind byte) float64 {
	if kind == 1 {
		return 100
	}
	return 1
}

func readChanges(head metricsChangeHead, body []byte) ([]Sample, error) {
	r := metricsBinaryReader{data: body}
	kind := r.byte()
	if kind > 2 {
		return nil, fmt.Errorf("%w: change representation", metricsErrCorrupt)
	}
	value, integer, err := firstChange(head.First, kind)
	if err != nil {
		return nil, err
	}

	out := make([]Sample, head.Count)
	next := r.size(head.Count - 1)
	if next == 0 || r.err != nil {
		return nil, fmt.Errorf("%w: first change position", metricsErrCorrupt)
	}
	for i := range out {
		if i == next {
			if value, integer, err = readChange(&r, kind, integer); err != nil {
				return nil, err
			}
			if next, err = nextChange(&r, head.Count, i); err != nil {
				return nil, err
			}
		}
		out[i] = Sample{At: int64(i), Value: value}
	}
	if err = r.finish(); err != nil {
		return nil, err
	}
	return out, nil
}

// firstChange checks that the first value is one the kind could have written.
func firstChange(first float64, kind byte) (float64, int64, error) {
	if kind == 2 {
		return first, 0, nil
	}
	factor := changeFactor(kind)
	q := math.Round(first * factor)
	if math.IsNaN(q) || math.IsInf(q, 0) || math.Abs(q) >= 0x1p60 ||
		math.Float64bits(float64(int64(q))/factor) != math.Float64bits(first) {
		return 0, 0, fmt.Errorf("%w: first change value", metricsErrCorrupt)
	}
	return first, int64(q), nil
}

// readChange reads the next value: raw bits, or a delta to the running integer
// that must stay inside the range the encoder accepts.
func readChange(r *metricsBinaryReader, kind byte, integer int64) (float64, int64, error) {
	if kind == 2 {
		return math.Float64frombits(r.word()), integer, nil
	}
	delta := metricsUnfoldSigned(r.unsigned())
	if (delta > 0 && integer > math.MaxInt64-delta) || (delta < 0 && integer < math.MinInt64-delta) {
		return 0, 0, fmt.Errorf("%w: change integer overflow", metricsErrCorrupt)
	}
	integer += delta
	if integer <= -(1<<60) || integer >= 1<<60 {
		return 0, 0, fmt.Errorf("%w: change integer range", metricsErrCorrupt)
	}
	return float64(integer) / changeFactor(kind), integer, nil
}

// nextChange is the position of the change after i, or the end of the block
// when the body holds no more.
func nextChange(r *metricsBinaryReader, count, i int) (int, error) {
	if len(r.data) == 0 {
		return count, nil
	}
	gap := r.size(count - 1 - i)
	if gap == 0 || r.err != nil {
		return 0, fmt.Errorf("%w: change position", metricsErrCorrupt)
	}
	return i + gap, nil
}

func packedBits(values []uint64, width int) []byte {
	out := make([]byte, (len(values)*width+7)/8)
	for i, value := range values {
		for bit := range width {
			position := i*width + bit
			out[position/8] |= byte(value>>uint(bit)&1) << uint(position%8)
		}
	}
	return out
}

func readPackedBits(data []byte, count, width int) ([]uint64, error) {
	if width < 0 || width > 64 || count < 0 || count > metricsBlockSamples || len(data) != (count*width+7)/8 {
		return nil, fmt.Errorf("%w: packed residual size", metricsErrCorrupt)
	}
	if used := count * width % 8; used != 0 && data[len(data)-1]>>uint(used) != 0 {
		return nil, fmt.Errorf("%w: residual padding", metricsErrCorrupt)
	}
	out := make([]uint64, count)
	for i := range out {
		for bit := range width {
			position := i*width + bit
			out[i] |= uint64(data[position/8]>>uint(position%8)&1) << uint(bit)
		}
	}
	return out, nil
}

// metricsBinaryReader turns malformed input into one error before callers allocate from its lengths
type metricsBinaryReader struct {
	data []byte
	err  error
}

func (r *metricsBinaryReader) take(size int) []byte {
	if r.err != nil {
		return nil
	}
	if size < 0 || size > len(r.data) {
		r.err = fmt.Errorf("%w: truncated binary field", metricsErrCorrupt)
		return nil
	}
	value := r.data[:size]
	r.data = r.data[size:]
	return value
}

func (r *metricsBinaryReader) byte() byte {
	data := r.take(1)
	if len(data) == 0 {
		return 0
	}
	return data[0]
}

func (r *metricsBinaryReader) unsigned() uint64 {
	if r.err != nil {
		return 0
	}
	value, n := binary.Uvarint(r.data)
	if n <= 0 {
		r.err = fmt.Errorf("%w: invalid varint", metricsErrCorrupt)
		return 0
	}
	r.data = r.data[n:]
	return value
}

func (r *metricsBinaryReader) size(maximum int) int {
	value := r.unsigned()
	if maximum < 0 || value > uint64(maximum) {
		r.err = fmt.Errorf("%w: binary size exceeds limit", metricsErrCorrupt)
		return 0
	}
	return int(value) //nolint:gosec // checked against a nonnegative int bound
}

func (r *metricsBinaryReader) word() uint64 {
	data := r.take(8)
	if len(data) != 8 {
		return 0
	}
	return binary.LittleEndian.Uint64(data)
}

func (r *metricsBinaryReader) finish() error {
	if r.err != nil {
		return r.err
	}
	if len(r.data) != 0 {
		return fmt.Errorf("%w: trailing binary fields", metricsErrCorrupt)
	}
	return nil
}

// metricsFoldSigned is zigzag: small magnitudes of either sign become small numbers.
func metricsFoldSigned(value int64) uint64 {
	return uint64(value)<<1 ^ uint64(value>>63) //nolint:gosec // a bit transform, not a value converted
}

func metricsUnfoldSigned(value uint64) int64 {
	return int64(value>>1) ^ -int64(value&1)
}

func metricsAppendCount(out []byte, count int) []byte {
	return binary.AppendUvarint(out, uint64(count)) //nolint:gosec // counts are never negative
}

// The reservoir consumes complete words; only the last word uses byte writes.
func packedBitsChunked(values []uint64, width int) []byte {
	out := make([]byte, (len(values)*width+7)/8)
	if width == 0 {
		return out
	}
	mask := ^uint64(0)
	if width < 64 {
		mask = uint64(1)<<uint(width) - 1
	}
	var pending uint64
	used, position := 0, 0
	for _, value := range values {
		value &= mask
		pending |= value << uint(used)
		if used+width < 64 {
			used += width
			continue
		}
		binary.LittleEndian.PutUint64(out[position:position+8], pending)
		position += 8
		consumed := 64 - used
		pending = value >> uint(consumed)
		used = width - consumed
	}
	for used > 0 {
		out[position] = byte(pending)
		position++
		pending >>= 8
		used -= 8
	}
	return out
}

func readPackedBitsChunked(data []byte, count, width int) ([]uint64, error) {
	if width < 0 || width > 64 || count < 0 || count > metricsBlockSamples || len(data) != (count*width+7)/8 {
		return nil, fmt.Errorf("%w: packed residual size", metricsErrCorrupt)
	}
	if used := count * width % 8; used != 0 && data[len(data)-1]>>uint(used) != 0 {
		return nil, fmt.Errorf("%w: residual padding", metricsErrCorrupt)
	}
	out := make([]uint64, count)
	if width == 0 {
		return out, nil
	}
	mask := ^uint64(0)
	if width < 64 {
		mask = uint64(1)<<uint(width) - 1
	}
	var pending uint64
	used, position := 0, 0
	for i := range out {
		if used >= width {
			out[i] = pending & mask
			pending >>= uint(width)
			used -= width
			continue
		}
		take := min(8, len(data)-position)
		var word uint64
		if take == 8 {
			word = binary.LittleEndian.Uint64(data[position : position+8])
		} else {
			for j := range take {
				word |= uint64(data[position+j]) << uint(j*8)
			}
		}
		out[i] = (pending | word<<uint(used)) & mask
		needed := width - used
		pending = word >> uint(needed)
		used = take*8 - needed
		position += take
	}
	return out, nil
}

func metricsChangeFixture(shape string) []Sample {
	out := make([]Sample, metricsBlockSamples)
	raw := []uint64{
		0, 0x8000000000000000, 0x7ff8000000000001, 0x7ff8000000000042,
		0x7ff0000000000000, 0xfff0000000000000, 0x3ff0000000000001, 0xbff0000000000001,
	}
	for i := range out {
		out[i].At = int64(i)
		switch shape {
		case "constant":
			out[i].Value = math.Float64frombits(0x8000000000000000)
		case "sparse":
			out[i].Value = float64(5 + (i/48)*2)
		case "many":
			out[i].Value = float64((i*37)%1000 - 500)
		case "decimal":
			out[i].Value = float64(1200+(i/3)%31) / 100
		case "raw":
			out[i].Value = math.Float64frombits(raw[(i/2)%len(raw)])
		default:
			panic("unknown metric shape")
		}
	}
	return out
}

func metricsPackedFixture(width int) []uint64 {
	values := make([]uint64, metricsBlockSamples)
	x := uint64(0x9e3779b97f4a7c15)
	mask := ^uint64(0)
	if width < 64 {
		mask = uint64(1)<<uint(width) - 1
	}
	for i := range values {
		x ^= x << 13
		x ^= x >> 7
		x ^= x << 17
		values[i] = x & mask
	}
	return values
}

// The block's timestamp clock is outside the value-change representation.
func metricsReadValueBody(first float64, count int, body []byte) []Sample {
	if len(body) == 0 {
		out := make([]Sample, count)
		for i := range out {
			out[i] = Sample{At: int64(i), Value: first}
		}
		return out
	}
	out, err := readChanges(metricsChangeHead{First: first, Count: count}, body[1:])
	if err != nil {
		panic(err)
	}
	return out
}

func metricsCases() []Case {
	var cases []Case
	for _, shape := range []string{"constant", "sparse", "many", "decimal", "raw"} {
		points := metricsChangeFixture(shape)
		body := changeValues(points)
		first, count := points[0].Value, len(points)
		cases = append(cases,
			Case{Name: "metrics/change/" + shape + "/encode", Units: count, InputBytes: count * 16,
				InputHash: fingerprintSamples(points),
				Run:       func() Payload { return Payload{Bytes: changeValues(points)} }},
			Case{Name: "metrics/change/" + shape + "/decode", Units: count, InputBytes: len(body) + 8,
				InputHash: fingerprintBytes(body, math.Float64bits(first), uint64(count)),
				Run:       func() Payload { return Payload{Samples: metricsReadValueBody(first, count, body)} }},
		)
	}
	for _, width := range []int{1, 7, 17, 64} {
		values := metricsPackedFixture(width)
		body := packedBits(values, width)
		count := len(values)
		for _, variant := range []string{"baseline", "chunked"} {
			encode, decode := packedBits, readPackedBits
			if variant == "chunked" {
				encode, decode = packedBitsChunked, readPackedBitsChunked
			}
			name := fmt.Sprintf("metrics/packed/%d/%s/", width, variant)
			cases = append(cases,
				Case{Name: name + "encode", Units: count, InputBytes: count * 8,
					InputHash: fingerprintWords(values, uint64(width)),
					Run:       func() Payload { return Payload{Bytes: encode(values, width)} }},
				Case{Name: name + "decode", Units: count, InputBytes: len(body),
					InputHash: fingerprintBytes(body, uint64(count), uint64(width)), Run: func() Payload {
						out, err := decode(body, count, width)
						if err != nil {
							panic(err)
						}
						return Payload{Words: out}
					}},
			)
		}
	}
	return cases
}
