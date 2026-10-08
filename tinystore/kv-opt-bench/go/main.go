// Copyright 2026 the TinyStore authors. Apache-2.0; see ../NOTICE.
package main

import (
	"bytes"
	"context"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"log/slog"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"

	sqlite3 "github.com/ncruces/go-sqlite3"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/internal/dbstat"
	"github.com/tinyshed/tinystore/internal/sqlite"
	"github.com/tinyshed/tinystore/kv"
)

const epoch int64 = 1800000000000

var ctx = context.Background()
var sink uint64

type Document struct {
	ID   int64    `json:"id"`
	Name string   `json:"name"`
	Tags []string `json:"tags"`
}
type command struct {
	Op, Bucket, Key, Kind   string
	Owners                  []string
	Value                   kv.Raw
	Version                 string
	Expiry, Now, DefaultTTL int64
	N                       int64
	Limit                   int
	After                   string
}

func must(err error) {
	if err != nil {
		panic(err)
	}
}
func emit(v any) { must(json.NewEncoder(os.Stdout).Encode(v)) }
func open(dir string, now *int64) (*tinystore.Store, *kv.Store) {
	s, e := tinystore.Open(ctx, dir, tinystore.Options{Manual: true, Clock: func() time.Time { return time.UnixMilli(*now) }, Logger: slog.New(slog.NewTextHandler(io.Discard, nil))})
	must(e)
	k, e := kv.Open(ctx, s, kv.Options{})
	must(e)
	return s, k
}
func bucket[V any](k *kv.Store, name string) *kv.Bucket[V] {
	b, e := kv.OpenBucket[V](ctx, k, name)
	must(e)
	return b
}
func branch(k *kv.Store, c command) *kv.Bucket[kv.Raw] {
	opts := []kv.BucketOption{}
	if c.DefaultTTL > 0 {
		opts = append(opts, kv.DefaultTTL(time.Duration(c.DefaultTTL)*time.Millisecond))
	}
	b, e := kv.OpenBucket[kv.Raw](ctx, k, c.Bucket, opts...)
	must(e)
	a := make([]any, len(c.Owners))
	for i, s := range c.Owners {
		a[i] = s
	}
	return b.Of(a...)
}
func errResult(e error) any {
	if e == nil {
		return nil
	}
	code := "other"
	for _, p := range []struct {
		s error
		n string
	}{{tinystore.ErrInvalid, "invalid"}, {tinystore.ErrConflict, "conflict"}, {tinystore.ErrCorrupt, "corrupt"}, {tinystore.ErrLimit, "limit"}} {
		if errors.Is(e, p.s) {
			code = p.n
		}
	}
	out := map[string]any{"code": code}
	var ke *kv.KeyError
	if errors.As(e, &ke) {
		out["bucket"] = ke.Bucket
		out["path"] = ke.Path
	}
	return out
}
func entry(e kv.Entry[kv.Raw]) any {
	ex := int64(0)
	if !e.ExpiresAt.IsZero() {
		ex = e.ExpiresAt.UnixMilli()
	}
	return map[string]any{"key": e.Key, "value": e.Value, "version": e.Version.String(), "expiry": ex}
}
func options(c command) []kv.Option {
	var a []kv.Option
	if c.Version != "" {
		var v kv.Version
		must(v.UnmarshalText([]byte(c.Version)))
		a = append(a, kv.IfVersion(v))
	}
	if c.Expiry != 0 {
		a = append(a, kv.ExpireAt(time.UnixMilli(c.Expiry)))
	}
	return a
}
func trace(dir, path string) {
	now := epoch
	s, k := open(dir, &now)
	defer func() { must(s.Close(ctx)) }()
	f, e := os.Open(path)
	must(e)
	defer f.Close()
	d := json.NewDecoder(f)
	for {
		var c command
		e = d.Decode(&c)
		if e == io.EOF {
			break
		}
		must(e)
		if c.Now != 0 {
			now = c.Now
		}
		r := map[string]any{"op": c.Op}
		var err error
		if c.Op == "maintain" {
			m, e := k.Maintain(ctx)
			err = e
			r["expired"] = m.Expired
			r["cleared"] = m.Cleared
		} else if strings.HasPrefix(c.Op, "counter_") {
			a := make([]any, len(c.Owners))
			for i, o := range c.Owners {
				a[i] = o
			}
			co, e := kv.OpenCounters(ctx, k, c.Bucket, kv.DefaultTTL(time.Second))
			must(e)
			co = co.Of(a...)
			var n int64
			switch c.Op {
			case "counter_add":
				n, err = co.Add(ctx, c.Key, c.N)
			case "counter_max":
				n, err = co.Max(ctx, c.Key, c.N)
			case "counter_get":
				n, err = co.Get(ctx, c.Key)
			}
			if err == nil {
				r["n"] = n
			}
		} else {
			b := branch(k, c)
			switch c.Op {
			case "set_void":
				err = b.Set(ctx, c.Key, c.Value, options(c)...)
			case "set":
				v, e := b.SetEntry(ctx, c.Key, c.Value, options(c)...)
				err = e
				if e == nil {
					r["entry"] = entry(v)
				}
			case "absent":
				v, created, e := b.SetEntryIfAbsent(ctx, c.Key, c.Value, options(c)...)
				err = e
				if e == nil {
					r["created"] = created
					r["entry"] = entry(v)
				}
			case "get":
				v, found, e := b.GetEntry(ctx, c.Key)
				err = e
				r["found"] = found
				if found {
					r["entry"] = entry(v)
				}
			case "has":
				found, e := b.Has(ctx, c.Key)
				err = e
				r["found"] = found
			case "take":
				v, found, e := b.Take(ctx, c.Key, options(c)...)
				err = e
				r["found"] = found
				if found {
					r["value"] = v
				}
			case "take_bad":
				a := make([]any, len(c.Owners))
				for i, o := range c.Owners {
					a[i] = o
				}
				_, found, e := bucket[int64](k, c.Bucket).Of(a...).Take(ctx, c.Key)
				err = e
				r["found"] = found
			case "delete":
				err = b.Delete(ctx, c.Key, options(c)...)
			case "touch":
				found, e := b.Touch(ctx, c.Key, options(c)...)
				err = e
				r["found"] = found
			case "clear":
				err = b.Clear(ctx)
			case "rollback_set":
				sentinel := errors.New("research rollback")
				e := k.Tx(ctx, func(tx *kv.Tx) error {
					if e := b.WithTx(tx).Set(ctx, c.Key, c.Value); e != nil {
						return e
					}
					return sentinel
				})
				if !errors.Is(e, sentinel) {
					err = e
				}
				r["rolled_back"] = errors.Is(e, sentinel)
			case "scan":
				p, e := b.Scan(ctx, kv.Query{After: c.After, Limit: c.Limit})
				err = e
				if e == nil {
					es := make([]any, 0, len(p.Entries))
					for _, v := range p.Entries {
						es = append(es, entry(v))
					}
					r["entries"] = es
					r["more"] = p.More
					r["after"] = p.Next.After
				}
			default:
				panic(c.Op)
			}
		}
		r["error"] = errResult(err)
		emit(r)
	}
}
func fixture(dir string) {
	now := epoch
	s, k := open(dir, &now)
	bb := bucket[[]byte](k, "bench").Of("tenant", 42)
	sp := bucket[[]byte](k, "spill").Of("tenant", 42)
	js := bucket[Document](k, "json").Of("tenant", 42)
	cl := bucket[[]byte](k, "clear").Of("parent")
	ex := bucket[[]byte](k, "expiry")
	small := bucket[[]byte](k, "small").Of("parent")
	must(k.Tx(ctx, func(tx *kv.Tx) error {
		for i := 0; i < 4096; i++ {
			if e := bb.WithTx(tx).Set(ctx, fmt.Sprintf("k%06d", i), bytes.Repeat([]byte{byte(i % 251)}, 256)); e != nil {
				return e
			}
		}
		for i := 0; i < 512; i++ {
			if e := sp.WithTx(tx).Set(ctx, fmt.Sprintf("k%06d", i), bytes.Repeat([]byte{byte(i % 251)}, 4096)); e != nil {
				return e
			}
		}
		for i := 0; i < 1024; i++ {
			if e := js.WithTx(tx).Set(ctx, fmt.Sprintf("k%06d", i), Document{int64(i), "document", []string{"one", "two", "three"}}); e != nil {
				return e
			}
		}
		for i := 0; i < 10001; i++ {
			if e := cl.WithTx(tx).Set(ctx, fmt.Sprintf("k%06d", i), []byte("clear")); e != nil {
				return e
			}
		}
		for i := 0; i < 256; i++ {
			if e := small.WithTx(tx).Set(ctx, fmt.Sprintf("k%06d", i), bytes.Repeat([]byte("s"), 600)); e != nil {
				return e
			}
		}
		for i := 0; i < 1200; i++ {
			if e := ex.WithTx(tx).Set(ctx, fmt.Sprintf("k%06d", i), bytes.Repeat([]byte("x"), 600), kv.TTL(time.Second)); e != nil {
				return e
			}
		}
		return nil
	}))
	by := bucket[[]byte](k, "shapes")
	for i, n := range []int{0, 1, 31, 512, 513, 4096, 1 << 20} {
		must(by.Set(ctx, fmt.Sprintf("bytes-%d", i), bytes.Repeat([]byte{byte(i)}, n)))
	}
	st := bucket[string](k, "strings")
	for i, v := range []string{"", "abc", "Русский\x00text", strings.Repeat("s", 4096)} {
		must(st.Set(ctx, fmt.Sprint(i), v))
	}
	si := bucket[int64](k, "signed")
	for i, v := range []int64{math.MinInt64, -1, 0, 1, math.MaxInt64} {
		must(si.Set(ctx, fmt.Sprint(i), v))
	}
	ui := bucket[uint64](k, "unsigned")
	for i, v := range []uint64{0, 1, 1 << 63, math.MaxUint64} {
		must(ui.Set(ctx, fmt.Sprint(i), v))
	}
	ff := bucket[float64](k, "floats")
	for i, v := range []uint64{0, 1 << 63, 0x7ff0000000000000, 0xfff0000000000000, 0x7ff0000000000001, 0x7ff8000000001234} {
		must(ff.Set(ctx, fmt.Sprint(i), math.Float64frombits(v)))
	}
	f32 := bucket[float32](k, "float32")
	for i, v := range []uint32{0, 1 << 31, 0x7f800001, 0x7fc01234} {
		must(f32.Set(ctx, fmt.Sprint(i), math.Float32frombits(v)))
	}
	must(bucket[struct{}](k, "nothing").Set(ctx, "empty", struct{}{}))
	must(by.Of("zero\x00owner", "深い", "a", "b", "c", "d", "e").Set(ctx, "key\x00x", []byte("deep")))
	must(by.Set(ctx, int64(42), []byte("integer identity")))
	must(bucket[kv.Raw](k, "trace").Set(ctx, "bad", kv.Raw{Kind: kv.RawBytes, Bytes: []byte("not an integer")}))
	_, e := kv.OpenCounters(ctx, k, "counters", kv.DefaultTTL(time.Second))
	must(e)
	must(s.Close(ctx))
	emit(map[string]any{"fixture": "deterministic-v1", "now": epoch, "db": "kv.db"})
}
func escapeLoop(dst []byte, s string) []byte {
	for i := 0; i < len(s); i++ {
		dst = append(dst, s[i])
		if s[i] == 0 {
			dst = append(dst, 255)
		}
	}
	return dst
}
func escapeSearch(dst []byte, s string) []byte {
	for {
		p := strings.IndexByte(s, 0)
		if p < 0 {
			return append(dst, s...)
		}
		dst = append(dst, s[:p+1]...)
		dst = append(dst, 255)
		s = s[p+1:]
	}
}
func kernel(name string, i int) uint64 {
	switch name {
	case "key_loop", "key_search":
		key := kernelKeys[i&255]
		p := make([]byte, 0, len(key)+20)
		p = append(p, 1)
		if name == "key_loop" {
			p = escapeLoop(p, "tenant")
			p = append(p, 0, 1)
			p = escapeLoop(p, "42")
			p = append(p, 0, 2)
			p = escapeLoop(p, key)
		} else {
			p = escapeSearch(p, "tenant")
			p = append(p, 0, 1)
			p = escapeSearch(p, "42")
			p = append(p, 0, 2)
			p = escapeSearch(p, key)
		}
		return consumePath(p)
	case "value_codec":
		bits := uint64(i) * 0x9e3779b97f4a7c15
		b := encode64(bits)
		return (binary.BigEndian.Uint64(b) ^ bits) + uint64(len(b))
	case "json_codec":
		d := Document{int64(i & 255), "document", []string{"one", "two", "three"}}
		b, e := json.Marshal(d)
		must(e)
		var got Document
		must(json.Unmarshal(b, &got))
		return uint64(len(b)) + uint64(got.ID)
	case "scan_materialize":
		es := make([]kv.Entry[kv.Raw], 0, 100)
		for j := 0; j < 100; j++ {
			es = append(es, kv.Entry[kv.Raw]{Key: strings.Clone(kernelKeys[j]), Value: kv.Raw{Kind: kv.RawBytes, Bytes: bytes.Clone(kernelValue)}})
		}
		return consumeEntries(es)
	case "ttl_guard":
		j := i & 7
		v := observedVersions[j]
		if v > 0 && v == expectedVersions[j] && expiries[j] > 100 && !hiddenRows[j] {
			return 1
		}
		return 0
	}
	panic(name)
}

var kernelKeys []string

//go:noinline
func consumePath(p []byte) uint64 { return uint64(len(p)) + uint64(p[len(p)-1]) }

//go:noinline
func consumeEntries(es []kv.Entry[kv.Raw]) uint64 {
	sum := uint64(0)
	for _, e := range es {
		sum += uint64(len(e.Key) + len(e.Value.Bytes))
	}
	return sum
}

var observedVersions = [8]int64{10, 10, 11, 10, 0, 10, 11, 10}
var expectedVersions = [8]int64{10, 11, 11, 10, 0, 10, 10, 10}
var expiries = [8]int64{200, 200, 99, 100, 200, 201, 200, 200}
var hiddenRows = [8]bool{false, false, false, false, false, true, false, false}

//go:noinline
func encode64(bits uint64) []byte { return binary.BigEndian.AppendUint64(nil, bits) }

var kernelValue = bytes.Repeat([]byte("k"), 256)

func init() {
	for i := 0; i < 256; i++ {
		s := fmt.Sprintf("key%06d-Русский-%s", i, strings.Repeat("x", i%80))
		if i%4 == 0 {
			s += "\x00tail"
		}
		kernelKeys = append(kernelKeys, s)
	}
}
func rss() map[string]int64 {
	out := map[string]int64{}
	b, e := os.ReadFile("/proc/self/status")
	if e == nil {
		for _, line := range strings.Split(string(b), "\n") {
			a := strings.Fields(line)
			if len(a) >= 2 && (a[0] == "VmRSS:" || a[0] == "VmHWM:") {
				n, _ := strconv.ParseInt(a[1], 10, 64)
				out[a[0]] = n * 1024
			}
		}
	}
	return out
}
func metadata(dir string) {
	db, e := sqlite.OpenDB(filepath.Join(dir, "kv.db"))
	must(e)
	defer db.Close()
	out := map[string]any{}
	for _, q := range []string{"sqlite_version()", "sqlite_source_id()"} {
		var v string
		must(db.QueryRow("select " + q).Scan(&v))
		out[q] = v
	}
	rows, e := db.Query("pragma compile_options")
	must(e)
	opts := []string{}
	for rows.Next() {
		var v string
		must(rows.Scan(&v))
		opts = append(opts, v)
	}
	must(rows.Err())
	must(rows.Close())
	out["compile_options"] = opts
	emit(out)
}
func sqliteCounters(cs []*sqlite3.Conn) any {
	out := []any{}
	for _, c := range cs {
		m := map[string]int64{}
		for _, p := range []struct {
			n  string
			op sqlite3.DBStatus
		}{{"cache_hits", sqlite3.DBSTATUS_CACHE_HIT}, {"cache_misses", sqlite3.DBSTATUS_CACHE_MISS}, {"cache_writes", sqlite3.DBSTATUS_CACHE_WRITE}, {"cache_spills", sqlite3.DBSTATUS_CACHE_SPILL}, {"cache_used", sqlite3.DBSTATUS_CACHE_USED}, {"stmt_used", sqlite3.DBSTATUS_STMT_USED}} {
			n, _, e := c.Status(p.op, false)
			must(e)
			m[p.n] = n
		}
		out = append(out, m)
	}
	return out
}

//go:noinline
func consumeWritten(v kv.Entry[[]byte]) uint64 {
	if !v.ExpiresAt.IsZero() {
		panic("expiry")
	}
	return uint64(len(v.Key) + len(v.Value) + len(v.Version.String()))
}
func bench(dir, caseName string, n int) {
	now := epoch
	s, k := open(dir, &now)
	defer func() { must(s.Close(ctx)) }()
	b := bucket[[]byte](k, "bench").Of("tenant", 42)
	raw := bucket[kv.Raw](k, "bench").Of("tenant", 42)
	sp := bucket[[]byte](k, "spill").Of("tenant", 42)
	js := bucket[Document](k, "json").Of("tenant", 42)
	small := bucket[[]byte](k, "small").Of("parent")
	clear := bucket[[]byte](k, "clear").Of("parent")
	co, e := kv.OpenCounters(ctx, k, "counters")
	must(e)
	keys := make([]string, 4096)
	for i := range keys {
		keys[i] = fmt.Sprintf("k%06d", i)
	}
	value := bytes.Repeat([]byte("v"), 256)
	large := bytes.Repeat([]byte("l"), 4096)
	huge := bytes.Repeat([]byte("h"), 65536)
	var sqlfile *sqlite.File
	var sqlConns []*sqlite3.Conn
	if caseName == "sql_get" {
		sqlfile, e = sqlite.Open(ctx, filepath.Join(dir, "kv.db"), sqlite.Config{Readers: 1, PageSize: 4096, WriterCache: 4 << 20, Connected: func(c *sqlite3.Conn) error { sqlConns = append(sqlConns, c); return nil }})
		must(e)
		defer sqlfile.Close()
	}
	op := func(i int) uint64 {
		key := keys[(i*997)&4095]
		switch caseName {
		case "get_bytes":
			v, f, e := b.Get(ctx, key)
			must(e)
			if !f {
				panic("missing")
			}
			return uint64(len(v))
		case "get_raw":
			v, f, e := raw.Get(ctx, key)
			must(e)
			if !f {
				panic("missing")
			}
			return uint64(len(v.Bytes))
		case "getentry":
			v, f, e := b.GetEntry(ctx, key)
			must(e)
			if !f {
				panic("missing")
			}
			return uint64(len(v.Value) + len(v.Version.String()))
		case "has":
			f, e := b.Has(ctx, key)
			must(e)
			if f {
				return 1
			}
			return 0
		case "get_spill":
			v, f, e := sp.Get(ctx, keys[(i*997)&511])
			must(e)
			if !f {
				panic("missing")
			}
			return uint64(len(v))
		case "get_json":
			v, f, e := js.Get(ctx, keys[(i*997)&1023])
			must(e)
			if !f {
				panic("missing")
			}
			return uint64(v.ID + int64(len(v.Tags)))
		case "scan100":
			p, e := b.Scan(ctx, kv.Query{After: keys[(i*97)&2047], Limit: 100})
			must(e)
			sum := uint64(0)
			for _, v := range p.Entries {
				sum += uint64(len(v.Value) + len(v.Key))
			}
			return sum
		case "set_bytes":
			must(b.Set(ctx, key, value))
			return 1
		case "set_spill":
			must(sp.Set(ctx, keys[(i*997)&511], large))
			return 1
		case "set_64k":
			must(sp.Set(ctx, keys[(i*997)&511], huge))
			return 1
		case "setentry_bytes":
			v, e := b.SetEntry(ctx, key, value)
			must(e)
			return consumeWritten(v)
		case "setentry_spill":
			v, e := sp.SetEntry(ctx, keys[(i*997)&511], large)
			must(e)
			return consumeWritten(v)
		case "cas":
			v, f, e := b.GetEntry(ctx, key)
			must(e)
			if !f {
				panic("missing")
			}
			must(b.Set(ctx, key, value, kv.IfVersion(v.Version)))
			return 1
		case "take_cycle":
			v, f, e := b.Take(ctx, key)
			must(e)
			if !f {
				panic("missing")
			}
			must(b.Set(ctx, key, value))
			return uint64(len(v))
		case "delete_cycle":
			must(b.Delete(ctx, key))
			must(b.Set(ctx, key, value))
			return 1
		case "counter_add":
			v, e := co.Add(ctx, keys[(i*997)&1023], 1)
			must(e)
			return uint64(v)
		case "clear_small":
			must(small.Clear(ctx))
			return 256
		case "clear_mark":
			must(clear.Clear(ctx))
			return 10001
		case "expire":
			now = epoch + 1000
			m, e := k.Maintain(ctx)
			must(e)
			return uint64(m.Expired)
		case "sql_get":
			path := append([]byte{1}, []byte("tenant")...)
			path = append(path, 0, 1, '4', '2', 0, 2)
			path = append(path, key...)
			var v []byte
			must(sqlfile.Lookup(ctx, func(r sqlite.Reader) error {
				return sqlite.QueryRowByKey(ctx, r, "select value from cells where bucket=1 and path=?1", path).Scan(&v)
			}))
			return uint64(len(v))
		}
		panic(caseName)
	}
	if !strings.HasPrefix(caseName, "clear_") && caseName != "expire" {
		for i := 0; i < 64; i++ {
			sink += op(i)
		}
	}
	runtime.GC()
	sqlBefore := sqliteCounters(sqlConns)
	var a, z runtime.MemStats
	before := rss()
	runtime.ReadMemStats(&a)
	start := time.Now()
	sum := uint64(0)
	for i := 0; i < n; i++ {
		sum += op(i)
	}
	elapsed := time.Since(start)
	runtime.ReadMemStats(&z)
	sink += sum
	emit(map[string]any{"language": "go", "case": caseName, "iterations": n, "elapsed_ns": elapsed.Nanoseconds(), "ns_op": float64(elapsed.Nanoseconds()) / float64(n), "alloc_calls": z.Mallocs - a.Mallocs, "alloc_bytes": z.TotalAlloc - a.TotalAlloc, "rss_before": before, "rss_after": rss(), "checksum": sum, "sqlite_counters": "public kv handle exposes none; SQL-control file only", "sqlite_control_before": sqlBefore, "sqlite_control_after": sqliteCounters(sqlConns), "go_gc": z.NumGC - a.NumGC})
}
func kernelBench(name string, n int) {
	for i := 0; i < 64; i++ {
		sink += kernel(name, i)
	}
	runtime.GC()
	var a, z runtime.MemStats
	runtime.ReadMemStats(&a)
	start := time.Now()
	sum := uint64(0)
	for i := 0; i < n; i++ {
		sum += kernel(name, i)
	}
	dt := time.Since(start)
	runtime.ReadMemStats(&z)
	sink += sum
	emit(map[string]any{"language": "go", "case": name, "iterations": n, "elapsed_ns": dt.Nanoseconds(), "ns_op": float64(dt.Nanoseconds()) / float64(n), "alloc_calls": z.Mallocs - a.Mallocs, "alloc_bytes": z.TotalAlloc - a.TotalAlloc, "checksum": sum, "rss_after": rss()})
}
func verifyTyped(dir string) {
	now := epoch
	s, k := open(dir, &now)
	defer s.Close(ctx)
	for i, v := range []uint64{0, 1 << 63, 0x7ff0000000000000, 0xfff0000000000000, 0x7ff0000000000001, 0x7ff8000000001234} {
		got, f, e := bucket[float64](k, "floats").Get(ctx, fmt.Sprint(i))
		must(e)
		if !f || math.Float64bits(got) != v {
			panic("float bits")
		}
	}
	for i, v := range []uint32{0, 1 << 31, 0x7f800001, 0x7fc01234} {
		got, f, e := bucket[float32](k, "float32").Get(ctx, fmt.Sprint(i))
		must(e)
		if !f || math.Float32bits(got) != v {
			panic("float32 bits")
		}
	}
	_, f, e := bucket[[]byte](k, "shapes").Get(ctx, "42")
	must(e)
	if !f {
		panic("key identity")
	}
	for i, want := range []int64{math.MinInt64, -1, 0, 1, math.MaxInt64} {
		got, found, e := bucket[int64](k, "signed").Get(ctx, fmt.Sprint(i))
		must(e)
		if !found || got != want {
			panic("signed cross-read")
		}
	}
	for i, want := range []uint64{0, 1, 1 << 63, math.MaxUint64} {
		got, found, e := bucket[uint64](k, "unsigned").Get(ctx, fmt.Sprint(i))
		must(e)
		if !found || got != want {
			panic("unsigned cross-read")
		}
	}
	for i, want := range []string{"", "abc", "Русский\x00text", strings.Repeat("s", 4096)} {
		got, found, e := bucket[string](k, "strings").Get(ctx, fmt.Sprint(i))
		must(e)
		if !found || got != want {
			panic("string cross-read")
		}
	}
	for i, n := range []int{0, 1, 31, 512, 513, 4096, 1 << 20} {
		got, found, e := bucket[[]byte](k, "shapes").Get(ctx, fmt.Sprintf("bytes-%d", i))
		must(e)
		if !found || !bytes.Equal(got, bytes.Repeat([]byte{byte(i)}, n)) {
			panic("bytes cross-read")
		}
	}
	emit(map[string]any{"typed_bits": "passed", "integer_identity": "passed"})
}
func main() {
	mode := flag.String("mode", "bench", "")
	dir := flag.String("dir", "", "")
	name := flag.String("case", "get_bytes", "")
	n := flag.Int("iterations", 10000, "")
	tr := flag.String("trace", "", "")
	flag.Parse()
	switch *mode {
	case "fixture":
		fixture(*dir)
	case "metadata":
		metadata(*dir)
	case "trace":
		trace(*dir, *tr)
	case "bench":
		bench(*dir, *name, *n)
	case "kernel":
		kernelBench(*name, *n)
	case "verify":
		verifyTyped(*dir)
	case "dbstat":
		objects, e := dbstat.Read(ctx, filepath.Join(*dir, "kv.db"))
		must(e)
		emit(objects)
	case "kernel-check":
		out := map[string]any{}
		for _, n := range []string{"key_loop", "key_search", "value_codec", "json_codec", "scan_materialize", "ttl_guard"} {
			sum := uint64(0)
			for i := 0; i < 256; i++ {
				sum += kernel(n, i)
			}
			out[n] = sum
		}
		out["escaped_hex"] = hex.EncodeToString(escapeLoop(nil, "a\x00b"))
		emit(out)
	}
}
