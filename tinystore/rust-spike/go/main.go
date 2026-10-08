package main

import (
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"math"
	"os"
	"runtime"
	"strconv"
	"strings"
	"time"
	"unsafe"
)

type Sample struct {
	At    int64
	Value float64
}
type Payload struct {
	Bytes   []byte
	Words   []uint64
	Samples []Sample
}
type Case struct {
	Name              string
	Units, InputBytes int
	InputHash         uint64
	Run               func() Payload
}

func fingerprintBytes(data []byte, parameters ...uint64) uint64 {
	h := uint64(14695981039346656037)
	for _, b := range data {
		h = (h ^ uint64(b)) * 1099511628211
	}
	for _, value := range parameters {
		for i := range 8 {
			h = (h ^ uint64(byte(value>>uint(i*8)))) * 1099511628211
		}
	}
	return h
}
func fingerprintWords(words []uint64, parameters ...uint64) uint64 {
	h := uint64(14695981039346656037)
	for _, run := range [][]uint64{words, parameters} {
		for _, value := range run {
			for i := range 8 {
				h = (h ^ uint64(byte(value>>uint(i*8)))) * 1099511628211
			}
		}
	}
	return h
}
func fingerprintSamples(points []Sample) uint64 {
	h := uint64(14695981039346656037)
	for _, p := range points {
		for _, value := range []uint64{uint64(p.At), math.Float64bits(p.Value)} {
			for i := range 8 {
				h = (h ^ uint64(byte(value>>uint(i*8)))) * 1099511628211
			}
		}
	}
	return h
}

var sink Payload

func cases() []Case {
	out := metricsCases()
	out = append(out, recordsCases()...)
	return append(out, ingestCases()...)
}

func emit(value any) {
	if err := json.NewEncoder(os.Stdout).Encode(value); err != nil {
		panic(err)
	}
}

func payloadBytes(p Payload) []byte {
	if p.Words != nil {
		out := make([]byte, 0, len(p.Words)*8)
		for _, value := range p.Words {
			out = binary.LittleEndian.AppendUint64(out, value)
		}
		return out
	}
	if p.Samples != nil {
		out := make([]byte, 0, len(p.Samples)*16)
		for _, point := range p.Samples {
			out = binary.LittleEndian.AppendUint64(out, uint64(point.At))
			out = binary.LittleEndian.AppendUint64(out, math.Float64bits(point.Value))
		}
		return out
	}
	return p.Bytes
}

func payloadSize(p Payload) int     { return len(p.Bytes) + 8*len(p.Words) + 16*len(p.Samples) }
func payloadCapacity(p Payload) int { return cap(p.Bytes) + 8*cap(p.Words) + 16*cap(p.Samples) }

func rssKiB() uint64 {
	data, err := os.ReadFile("/proc/self/status")
	if err != nil {
		panic(err)
	}
	for _, line := range strings.Split(string(data), "\n") {
		if strings.HasPrefix(line, "VmRSS:") {
			n, err := strconv.ParseUint(strings.Fields(line)[1], 10, 64)
			if err != nil {
				panic(err)
			}
			return n
		}
	}
	panic("RSS unavailable")
}

func runBench(c Case, seconds float64) {
	// Calibrate before reporting so timer and process startup do not dominate.
	iterations := 1
	var elapsed time.Duration
	for {
		start := time.Now()
		for range iterations {
			sink = c.Run()
		}
		elapsed = time.Since(start)
		if elapsed.Seconds() >= seconds {
			break
		}
		iterations *= 2
	}
	emit(map[string]any{"mode": "bench", "language": "go", "case": c.Name, "units": c.Units, "iterations": iterations, "ns_per_op": float64(elapsed.Nanoseconds()) / float64(iterations), "output_bytes": payloadSize(sink)})
}

func runAlloc(c Case, n int) {
	for range 8 {
		sink = c.Run()
	}
	runtime.GC()
	var before, after runtime.MemStats
	runtime.ReadMemStats(&before)
	for range n {
		sink = c.Run()
	}
	runtime.ReadMemStats(&after)
	emit(map[string]any{"mode": "alloc", "language": "go", "case": c.Name, "iterations": n, "bytes_per_op": float64(after.TotalAlloc-before.TotalAlloc) / float64(n), "allocs_per_op": float64(after.Mallocs-before.Mallocs) / float64(n), "output_bytes": payloadSize(sink), "output_capacity_bytes": payloadCapacity(sink)})
}

func runMemory(c Case, n int) {
	for range 8 {
		sink = c.Run()
	}
	sink = Payload{}
	runtime.GC()
	baseline := rssKiB()
	retained := make([]Payload, 0, n)
	logical, capacity := 0, 0
	for range n {
		p := c.Run()
		logical += payloadSize(p)
		capacity += payloadCapacity(p)
		retained = append(retained, p)
	}
	runtime.GC()
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	emit(map[string]any{"mode": "memory", "language": "go", "case": c.Name, "retained": n, "logical_output_bytes": logical, "capacity_output_bytes": capacity, "retained_harness_metadata_bytes": n * int(unsafe.Sizeof(Payload{})), "baseline_rss_kib": baseline, "rss_kib": rssKiB(), "heap_alloc_bytes": m.HeapAlloc, "heap_sys_bytes": m.HeapSys, "gc_cycles": m.NumGC})
	runtime.KeepAlive(retained)
}

func main() {
	mode := flag.String("mode", "verify", "verify, bench, alloc or memory")
	filter := flag.String("case", "", "exact case name")
	seconds := flag.Float64("seconds", 0.15, "minimum seconds for final calibrated timing")
	n := flag.Int("iterations", 256, "allocation or retained output count")
	flag.Parse()
	found := false
	selected := cases()
	if *filter != "" {
		var kept []Case
		for _, c := range selected {
			if c.Name == *filter {
				kept = append(kept, c)
			}
		}
		selected = kept
	}
	for _, c := range selected {
		found = true
		switch *mode {
		case "verify":
			p := c.Run()
			emit(map[string]any{"mode": "verify", "case": c.Name, "units": c.Units, "input_bytes": c.InputBytes, "input_fingerprint": fmt.Sprintf("%016x", c.InputHash), "hex": hex.EncodeToString(payloadBytes(p))})
		case "bench":
			runBench(c, *seconds)
		case "alloc":
			runAlloc(c, *n)
		case "memory":
			runMemory(c, *n)
		default:
			panic(fmt.Sprintf("unknown mode %s", *mode))
		}
	}
	if !found {
		panic("case not found")
	}
}
