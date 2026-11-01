// Command tinystore encodes a series file's blocks with TinyStore's adaptive
// codec, as the harness README says, and checks every value's bits back.
//
//	tinystore tinystore <series file> <out dir> <L1,L2,...>
//
// The codec takes 1 to codec.MaxSamples (240) values a call and keeps the
// first value and the count outside its stream, in the row around it. A body
// here holds them itself, so that it decodes alone as every other codec's
// does. A block of L values is cut into sub-blocks of at most 240, each
//
//	uvarint count | first value, 8 bytes | uvarint stream length | stream
//
// with the last sub-block's stream length left out, since the body ends
// there. A block of at most 240 values is one sub-block: count, first value
// and EncodeValues' stream, 9 or 10 bytes more than the stream alone.
package main

import (
	"bufio"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/tinyshed/tinystore/codec"
)

type series struct {
	name   string
	values []float64
}

func main() {
	if len(os.Args) != 5 || os.Args[1] != "tinystore" {
		fmt.Fprintln(os.Stderr, "usage: tinystore tinystore <series file> <out dir> <L1,L2,...>")
		os.Exit(2)
	}
	all, err := readSeries(os.Args[2])
	if err != nil {
		fail(err)
	}
	c, err := codec.New()
	if err != nil {
		fail(err)
	}
	defer c.Close()
	for _, field := range strings.Split(os.Args[4], ",") {
		length, err := strconv.Atoi(field)
		if err != nil || length < 1 {
			fail(fmt.Errorf("block length %q", field))
		}
		if err := encodeAll(c, all, length, os.Args[3]); err != nil {
			fail(err)
		}
	}
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}

func readSeries(path string) ([]series, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	r := bufio.NewReaderSize(f, 1<<20)
	magic := make([]byte, 4)
	if _, err := io.ReadFull(r, magic); err != nil || string(magic) != "CPS1" {
		return nil, errors.New("not a series file")
	}
	var count uint32
	if err := binary.Read(r, binary.LittleEndian, &count); err != nil {
		return nil, err
	}
	out := make([]series, 0, count)
	for range count {
		var nameLength uint32
		if err := binary.Read(r, binary.LittleEndian, &nameLength); err != nil {
			return nil, err
		}
		name := make([]byte, nameLength)
		if _, err := io.ReadFull(r, name); err != nil {
			return nil, err
		}
		var n uint64
		if err := binary.Read(r, binary.LittleEndian, &n); err != nil {
			return nil, err
		}
		raw := make([]byte, 8*n)
		if _, err := io.ReadFull(r, raw); err != nil {
			return nil, err
		}
		values := make([]float64, n)
		for i := range values {
			values[i] = math.Float64frombits(binary.LittleEndian.Uint64(raw[8*i:]))
		}
		out = append(out, series{name: string(name), values: values})
	}
	return out, nil
}

type record struct {
	series, count uint32
	status        byte
	body          []byte
}

func encodeAll(c *codec.Codec, all []series, length int, dir string) error {
	var records []record
	var payload, failed, values int
	for si, s := range all {
		for start := 0; start < len(s.values); start += length {
			block := s.values[start:min(start+length, len(s.values))]
			body, err := encodeBlock(c, block)
			if err != nil {
				return fmt.Errorf("series %d at %d: %w", si, start, err)
			}
			status := byte(0)
			if !roundTrips(c, body, block) {
				status = 1
				failed++
			}
			records = append(records, record{uint32(si), uint32(len(block)), status, body}) //nolint:gosec // counts of a file
			payload += len(body)
			values += len(block)
		}
	}
	path := filepath.Join(dir, fmt.Sprintf("tinystore.L%d.bodies", length))
	if err := writeBodies(path, records); err != nil {
		return err
	}
	fmt.Printf(`{"codec":"tinystore","block":%d,"blocks":%d,"values":%d,"payload_bytes":%d,"failed_blocks":%d}`+"\n",
		length, len(records), values, payload, failed)
	return nil
}

func encodeBlock(c *codec.Codec, block []float64) ([]byte, error) {
	var body []byte
	for start := 0; start < len(block); start += codec.MaxSamples {
		sub := block[start:min(start+codec.MaxSamples, len(block))]
		stream, err := c.EncodeValues(sub)
		if err != nil {
			return nil, err
		}
		body = binary.AppendUvarint(body, uint64(len(sub)))
		body = binary.LittleEndian.AppendUint64(body, math.Float64bits(sub[0]))
		if start+codec.MaxSamples < len(block) {
			body = binary.AppendUvarint(body, uint64(len(stream)))
		}
		body = append(body, stream...)
	}
	return body, nil
}

func roundTrips(c *codec.Codec, body []byte, block []float64) bool {
	got := 0
	for len(body) > 0 {
		count, n := binary.Uvarint(body)
		if n <= 0 || len(body) < n+8 {
			return false
		}
		first := math.Float64frombits(binary.LittleEndian.Uint64(body[n:]))
		body = body[n+8:]
		stream := body
		if got+int(count) < len(block) { //nolint:gosec // at most MaxSamples
			length, m := binary.Uvarint(body)
			if m <= 0 || uint64(len(body)-m) < length {
				return false
			}
			stream, body = body[m:m+int(length)], body[m+int(length):] //nolint:gosec // checked above
		} else {
			body = nil
		}
		it, err := c.DecodeValues(first, int(count), stream) //nolint:gosec // at most MaxSamples
		if err != nil {
			return false
		}
		for it.Next() {
			if got >= len(block) || math.Float64bits(it.Sample().Value) != math.Float64bits(block[got]) {
				return false
			}
			got++
		}
		if it.Err() != nil {
			return false
		}
	}
	return got == len(block)
}

func writeBodies(path string, records []record) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	w := bufio.NewWriterSize(f, 1<<20)
	header := binary.LittleEndian.AppendUint32([]byte("CPB1"), uint32(len(records))) //nolint:gosec // block count
	if _, err := w.Write(header); err != nil {
		return err
	}
	for _, r := range records {
		head := make([]byte, 0, 13)
		head = binary.LittleEndian.AppendUint32(head, r.series)
		head = binary.LittleEndian.AppendUint32(head, r.count)
		head = binary.LittleEndian.AppendUint32(head, uint32(len(r.body))) //nolint:gosec // a body is small
		head = append(head, r.status)
		if _, err := w.Write(head); err != nil {
			return err
		}
		if _, err := w.Write(r.body); err != nil {
			return err
		}
	}
	if err := w.Flush(); err != nil {
		return err
	}
	return f.Close()
}
