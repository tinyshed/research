// Research baseline: records/bits.go at e307c48a40126aad0e2873b6bf3aaedef8115483.
// The bit helpers below are copied verbatim apart from the package name.
// Each operation allocates a fresh output; Rice encoder scratch reuse is excluded.
// Kernel loops use records/ints.go writePlan(packWidth), readWidth and readRiceCode.
package main

import "encoding/binary"

import "fmt"

// bitWriter appends values least significant bit first, 64 bits at a time
type bitWriter struct {
	out  []byte
	acc  uint64
	used uint
}

func (w *bitWriter) write(value uint64, width uint) {
	for width > 0 {
		take := min(width, 64-w.used)
		part := value
		if take < 64 {
			part &= 1<<take - 1
		}
		w.acc |= part << w.used
		w.used += take
		value >>= take
		width -= take
		if w.used == 64 {
			w.out = binary.LittleEndian.AppendUint64(w.out, w.acc)
			w.acc, w.used = 0, 0
		}
	}
}

// finish writes the bits still held, padded to a whole byte
func (w *bitWriter) finish() []byte {
	for w.used > 0 {
		w.out = append(w.out, byte(w.acc)) //nolint:gosec // the accumulator's low byte, the next to write
		w.acc >>= 8
		w.used -= min(w.used, 8)
	}
	return w.out
}

// bitReader reads what bitWriter wrote; a read past the end sets short
type bitReader struct {
	data  []byte
	at    uint64
	short bool
}

func (r *bitReader) read(width uint) uint64 {
	var value uint64
	for got := uint(0); got < width; {
		index := r.at / 8
		if index >= uint64(len(r.data)) {
			r.short = true
			return 0
		}
		shift := uint(r.at % 8)
		take := min(8-shift, width-got)
		value |= (uint64(r.data[index]>>shift) & (1<<take - 1)) << got
		got += take
		r.at += uint64(take)
	}
	return value
}

// consumed is how many whole bytes the reads so far have touched
func (r *bitReader) consumed() uint64 {
	return (r.at + 7) / 8
}

// a rice code writes value>>k in unary and the low k bits as they are; a
// quotient of riceEscape or more is written as the escape and 64 raw bits
//
//	k=2   5 → 1 0 01     0 → 0 00     13 → 111 0 01
const riceEscape = 32

func writeRice(w *bitWriter, value uint64, k uint) {
	if quotient := value >> k; quotient < riceEscape {
		w.write(1<<quotient-1, uint(quotient))
		w.write(0, 1)
		w.write(value, k)
		return
	}
	w.write(1<<riceEscape-1, riceEscape)
	w.write(value, 64)
}

func readRice(r *bitReader, k uint) uint64 {
	quotient := uint64(0)
	for quotient < riceEscape && r.read(1) == 1 {
		quotient++
	}
	if quotient == riceEscape {
		return r.read(64)
	}
	return quotient<<k | r.read(k)
}

func riceBits(value uint64, k uint) uint64 {
	if quotient := value >> k; quotient < riceEscape {
		return quotient + 1 + uint64(k)
	}
	return riceEscape + 64
}

// distance is end - start as an unsigned number, exact across the whole signed range
func distance(start, end int64) uint64 {
	return uint64(end) - uint64(start) //nolint:gosec // modular subtraction is the exact distance
}

// advance is start + span, wrapping exactly as distance does, so it inverts it
func advance(start int64, span uint64) int64 {
	return int64(uint64(start) + span) //nolint:gosec // the inverse of distance, wrapping alike
}

// unsigned is a count or a length, which is never negative
func unsigned(n int) uint64 {
	return uint64(n) //nolint:gosec // counts and lengths are never negative
}

// parameter is a packer's width, group or rice parameter, each at most 64
func parameter(value uint) byte {
	return byte(min(value, 64))
}

// zigzag folds the sign into the lowest bit, as a varint does: small
// magnitudes of either sign become small numbers
func zigzag(value int64) uint64 {
	return uint64(value<<1) ^ uint64(value>>63) //nolint:gosec // a bit transform, not a value converted
}

// This experimental reader changes only bit extraction; its matching Rust
// implementation makes the algorithm change measurable in both languages.
type recordsWordReader struct {
	data      []byte
	next      int
	acc       uint64
	available uint
	at        uint64
	short     bool
}

func (r *recordsWordReader) refill() {
	remaining := len(r.data) - r.next
	if remaining >= 8 {
		r.acc = binary.LittleEndian.Uint64(r.data[r.next : r.next+8])
		r.next += 8
		r.available = 64
		return
	}
	r.acc = 0
	for i := 0; i < remaining; i++ {
		r.acc |= uint64(r.data[r.next+i]) << (8 * i)
	}
	r.next += remaining
	r.available = uint(remaining) * 8
}

func (r *recordsWordReader) read(width uint) uint64 {
	var value uint64
	for got := uint(0); got < width; {
		if r.available == 0 {
			r.refill()
			if r.available == 0 {
				r.short = true
				return 0
			}
		}
		take := min(width-got, r.available)
		part := r.acc
		if take < 64 {
			part &= 1<<take - 1
		}
		value |= part << got
		r.acc >>= take
		r.available -= take
		r.at += uint64(take)
		got += take
	}
	return value
}

func (r *recordsWordReader) consumed() uint64 {
	return (r.at + 7) / 8
}

func recordsReadRiceWord(r *recordsWordReader, k uint) uint64 {
	quotient := uint64(0)
	for quotient < riceEscape && r.read(1) == 1 {
		quotient++
	}
	if quotient == riceEscape {
		return r.read(64)
	}
	return quotient<<k | r.read(k)
}

func recordsEncodeWidth(values []uint64, width uint) []byte {
	writer := bitWriter{}
	for _, value := range values {
		writer.write(value, width)
	}
	return writer.finish()
}

func recordsDecodeWidth(data []byte, count int, width uint) ([]uint64, bool) {
	reader := bitReader{data: data}
	residuals := make([]uint64, count)
	for i := range residuals {
		residuals[i] = reader.read(width)
	}
	return residuals, reader.short
}

func recordsDecodeWidthWord(data []byte, count int, width uint) ([]uint64, bool) {
	reader := recordsWordReader{data: data}
	residuals := make([]uint64, count)
	for i := range residuals {
		residuals[i] = reader.read(width)
	}
	return residuals, reader.short
}

func recordsEncodeRice(values []uint64, k uint) []byte {
	writer := bitWriter{}
	for _, value := range values {
		writeRice(&writer, value, k)
	}
	return writer.finish()
}

func recordsDecodeRice(data []byte, count int, k uint) ([]uint64, bool) {
	reader := bitReader{data: data}
	residuals := make([]uint64, count)
	for i := range residuals {
		residuals[i] = readRice(&reader, k)
	}
	return residuals, reader.short || reader.consumed() != uint64(len(data))
}

func recordsDecodeRiceWord(data []byte, count int, k uint) ([]uint64, bool) {
	reader := recordsWordReader{data: data}
	residuals := make([]uint64, count)
	for i := range residuals {
		residuals[i] = recordsReadRiceWord(&reader, k)
	}
	return residuals, reader.short || reader.consumed() != uint64(len(data))
}

func recordsFixtureWidth(width uint) []uint64 {
	values := make([]uint64, 1024)
	state := uint64(0x243f6a8885a308d3)
	mask := ^uint64(0)
	if width < 64 {
		mask = 1<<width - 1
	}
	for i := range values {
		state += 0x9e3779b97f4a7c15
		value := state
		value = (value ^ value>>30) * 0xbf58476d1ce4e5b9
		value = (value ^ value>>27) * 0x94d049bb133111eb
		value ^= value >> 31
		values[i] = value & mask
		if i%97 == 0 {
			values[i] = 0
		} else if i%97 == 1 {
			values[i] = mask
		}
	}
	return values
}

func recordsFixtureRice(escapes bool) []uint64 {
	small := [...]uint64{0, 1, 0, 2, 3, 5, 8, 13, 4, 0, 31, 7, 63, 15, 127, 0}
	wide := [...]uint64{0, 1, 5, 127, 128, 129, 255, 256, 1024, 65536, 1 << 32, 1 << 63, ^uint64(0)}
	values := make([]uint64, 1024)
	for i := range values {
		if escapes {
			values[i] = wide[i%len(wide)]
		} else {
			values[i] = small[i%len(small)]
		}
		if i%97 == 0 {
			values[i] = 0
		}
	}
	return values
}

func recordsWords(values []uint64, invalid bool) Payload {
	if invalid {
		panic("records benchmark input truncated")
	}
	return Payload{Words: values}
}

func recordsCases() []Case {
	var cases []Case
	for _, width := range []uint{1, 7, 17, 64} {
		values := recordsFixtureWidth(width)
		packed := recordsEncodeWidth(values, width)
		count := len(values)
		encodeHash := fingerprintWords(values, uint64(width))
		decodeHash := fingerprintBytes(packed, uint64(count), uint64(width))
		prefix := fmt.Sprintf("records/width_%d/", width)
		cases = append(cases,
			Case{Name: prefix + "encode", Units: count, InputBytes: count * 8, InputHash: encodeHash,
				Run: func() Payload { return Payload{Bytes: recordsEncodeWidth(values, width)} }},
			Case{Name: prefix + "decode", Units: count, InputBytes: len(packed), InputHash: decodeHash,
				Run: func() Payload { return recordsWords(recordsDecodeWidth(packed, count, width)) }},
			Case{Name: prefix + "decode_word", Units: count, InputBytes: len(packed), InputHash: decodeHash,
				Run: func() Payload { return recordsWords(recordsDecodeWidthWord(packed, count, width)) }},
		)
	}
	for _, escapes := range []bool{false, true} {
		values := recordsFixtureRice(escapes)
		packed := recordsEncodeRice(values, 2)
		count := len(values)
		encodeHash := fingerprintWords(values, 2)
		decodeHash := fingerprintBytes(packed, uint64(count), 2)
		prefix := "records/rice_small/"
		if escapes {
			prefix = "records/rice_escapes/"
		}
		cases = append(cases,
			Case{Name: prefix + "encode", Units: count, InputBytes: count * 8, InputHash: encodeHash,
				Run: func() Payload { return Payload{Bytes: recordsEncodeRice(values, 2)} }},
			Case{Name: prefix + "decode", Units: count, InputBytes: len(packed), InputHash: decodeHash,
				Run: func() Payload { return recordsWords(recordsDecodeRice(packed, count, 2)) }},
			Case{Name: prefix + "decode_word", Units: count, InputBytes: len(packed), InputHash: decodeHash,
				Run: func() Payload { return recordsWords(recordsDecodeRiceWord(packed, count, 2)) }},
		)
	}
	return cases
}
