package main

import (
	"encoding/binary"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"runtime"
	"strconv"
	"strings"
	"time"
)

const (
	pointSQL     = `SELECT body FROM blocks WHERE series=?1 AND at=?2`
	rangeSQL     = `SELECT at,body FROM blocks WHERE series=?1 AND at>=?2 AND at<?3 ORDER BY at LIMIT CAST(?4 AS INTEGER)`
	aggregateSQL = `SELECT count(*),sum(at) FROM blocks WHERE series=?1 AND at>=?2 AND at<?3`
	updateSQL    = `UPDATE blocks SET body=?1 WHERE series=?2 AND at=?3`
	digestSQL    = `SELECT series,at,body FROM blocks ORDER BY series,at`
	insertSQL    = `INSERT INTO blocks VALUES(?1,?2,?3)`
	pragmaSQL    = `PRAGMA foreign_keys=1; PRAGMA busy_timeout=5000; PRAGMA synchronous=FULL; PRAGMA fullfsync=1; PRAGMA checkpoint_fullfsync=1; PRAGMA cache_size=-1024; PRAGMA trusted_schema=OFF;`
)

type row struct {
	At   int64
	Body []byte
}
type result struct {
	Blob                 []byte
	Rows                 []row
	Count, Sum, Affected int64
}
type runner interface {
	Run(string, uint64) result
	Digest() uint64
	Inspect() map[string]any
	Close()
}

var sink result
var ctx = backgroundContext()

func check(err error) {
	if err != nil {
		panic(err)
	}
}
func emit(v any)                  { check(json.NewEncoder(os.Stdout).Encode(v)) }
func key(i uint64) (int64, int64) { n := (i * 7919) % 32768; return int64(n / 512), int64(n % 512) }
func body(i uint64) []byte {
	b := make([]byte, 128)
	for k := range b {
		b[k] = byte((i + uint64(k)) % 251)
	}
	return b
}
func hashBytes(h uint64, b []byte) uint64 {
	for _, v := range b {
		h = (h ^ uint64(v)) * 1099511628211
	}
	return h
}
func hashWord(h uint64, n int64) uint64 {
	var b [8]byte
	binary.LittleEndian.PutUint64(b[:], uint64(n))
	return hashBytes(h, b[:])
}
func hashResult(h uint64, name string, r result) uint64 {
	switch name {
	case "point", "point_txn":
		return hashBytes(h, r.Blob)
	case "range240":
		for _, row := range r.Rows {
			h = hashWord(h, row.At)
			h = hashBytes(h, row.Body)
		}
	case "aggregate240":
		h = hashWord(h, r.Count)
		h = hashWord(h, r.Sum)
	case "update1", "update64":
		h = hashWord(h, r.Affected)
	default:
		panic("unknown case")
	}
	return h
}

func rssKiB() uint64 {
	data, err := os.ReadFile("/proc/self/status")
	check(err)
	for _, line := range strings.Split(string(data), "\n") {
		if strings.HasPrefix(line, "VmRSS:") {
			n, err := strconv.ParseUint(strings.Fields(line)[1], 10, 64)
			check(err)
			return n
		}
	}
	panic("RSS unavailable")
}

func main() {
	path := flag.String("db", "", "fixture database")
	name := flag.String("case", "point", "case")
	mode := flag.String("mode", "verify", "fixture,verify,bench,inspect")
	implementation := flag.String("implementation", "go_tinystore", "go_tinystore or go_raw")
	seconds := flag.Float64("seconds", 0.15, "minimum final interval")
	iterations := flag.Uint64("iterations", 16, "verification operations")
	page := flag.Int("page-size", 4096, "new fixture page size")
	flag.Parse()
	if *path == "" {
		panic("--db required")
	}
	if *mode == "fixture" {
		fixture(*path, *page)
		return
	}
	var r runner
	if *implementation == "go_tinystore" {
		r = openTiny(*path)
	} else if *implementation == "go_raw" {
		r = openRaw(*path)
	} else {
		panic("unknown implementation")
	}
	defer r.Close()
	switch *mode {
	case "verify":
		h := uint64(14695981039346656037)
		var rows, affected int64
		for i := uint64(0); i < *iterations; i++ {
			out := r.Run(*name, i)
			h = hashResult(h, *name, out)
			rows += out.Count
			affected += out.Affected
		}
		emit(map[string]any{"case": *name, "implementation": *implementation, "iterations": *iterations, "checksum": fmt.Sprintf("%016x", h), "rows": rows, "affected": affected, "final_digest": fmt.Sprintf("%016x", r.Digest())})
	case "bench":
		sequence := uint64(0)
		for range 32 {
			sink = r.Run(*name, sequence)
			sequence++
		}
		for n := uint64(1); ; n *= 2 {
			start := time.Now()
			for range n {
				sink = r.Run(*name, sequence)
				sequence++
			}
			elapsed := time.Since(start)
			if elapsed.Seconds() >= *seconds {
				emit(map[string]any{"case": *name, "implementation": *implementation, "iterations": n, "ns_per_op": float64(elapsed.Nanoseconds()) / float64(n)})
				break
			}
		}
	case "inspect":
		for i := uint64(0); i < 1000; i++ {
			sink = r.Run("point", i)
		}
		sink = result{}
		runtime.GC()
		info := r.Inspect()
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		info["implementation"] = *implementation
		info["rss_kib"] = rssKiB()
		info["go_heap_alloc_bytes"] = m.HeapAlloc
		info["go_heap_sys_bytes"] = m.HeapSys
		emit(info)
	default:
		panic("unknown mode")
	}
}
