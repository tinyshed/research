package main

import (
	"github.com/ncruces/go-sqlite3"
)

type raw struct {
	writer, reader   *sqlite3.Conn
	writers, readers map[string]*sqlite3.Stmt
}

func openRaw(path string) *raw {
	w, err := sqlite3.OpenContext(ctx, path)
	check(err)
	check(w.Exec(pragmaSQL))
	check(w.Exec(`PRAGMA journal_mode=WAL`))
	w.Limit(sqlite3.LIMIT_LENGTH, 1<<20)
	r, err := sqlite3.OpenFlags(path, sqlite3.OPEN_READWRITE)
	check(err)
	check(r.Exec(pragmaSQL))
	check(r.Exec(`PRAGMA query_only=1`))
	r.Limit(sqlite3.LIMIT_LENGTH, 1<<20)
	return &raw{w, r, make(map[string]*sqlite3.Stmt), make(map[string]*sqlite3.Stmt)}
}

func (r *raw) Close() {
	for _, s := range r.readers {
		check(s.Close())
	}
	for _, s := range r.writers {
		check(s.Close())
	}
	check(r.reader.Close())
	check(r.writer.Close())
}
func prepared(conn *sqlite3.Conn, cache map[string]*sqlite3.Stmt, sql string, params ...any) *sqlite3.Stmt {
	s, found := cache[sql]
	if !found {
		var err error
		s, _, err = conn.Prepare(sql)
		check(err)
		cache[sql] = s
	}
	for i, p := range params {
		switch v := p.(type) {
		case int64:
			check(s.BindInt64(i+1, v))
		case int:
			check(s.BindInt(i+1, v))
		case []byte:
			check(s.BindBlob(i+1, v))
		default:
			panic("unsupported bind")
		}
	}
	return s
}
func rawExec(conn *sqlite3.Conn, cache map[string]*sqlite3.Stmt, sql string, params ...any) {
	s := prepared(conn, cache, sql, params...)
	check(s.Exec())
	check(s.Reset())
}

func (r *raw) Run(name string, i uint64) (out result) {
	series, at := key(i)
	if name == "update1" || name == "update64" {
		check(r.writer.Exec(`BEGIN IMMEDIATE`))
		n := uint64(1)
		if name == "update64" {
			n = 64
		}
		for j := uint64(0); j < n; j++ {
			q := i
			if n == 64 {
				q = i*64 + j
				rawExec(r.writer, r.writers, `SAVEPOINT grouped`)
			}
			series, at = key(q)
			rawExec(r.writer, r.writers, updateSQL, body(q), series, at)
			out.Affected += r.writer.Changes()
			if n == 64 {
				rawExec(r.writer, r.writers, `RELEASE grouped`)
			}
		}
		check(r.writer.Exec(`COMMIT`))
		return out
	}
	if name == "point_txn" {
		check(r.reader.Exec(`BEGIN DEFERRED`))
	}
	var s *sqlite3.Stmt
	switch name {
	case "point", "point_txn":
		s = prepared(r.reader, r.readers, pointSQL, series, at)
		if !s.Step() {
			panic("point missing")
		}
		out.Blob = s.ColumnBlob(0, nil)
		out.Count = 1
	case "range240":
		start := at % 273
		s = prepared(r.reader, r.readers, rangeSQL, series, start, start+240, 240)
		for s.Step() {
			out.Rows = append(out.Rows, row{At: s.ColumnInt64(0), Body: s.ColumnBlob(1, nil)})
		}
		out.Count = int64(len(out.Rows))
	case "aggregate240":
		start := at % 273
		s = prepared(r.reader, r.readers, aggregateSQL, series, start, start+240)
		if !s.Step() {
			panic("aggregate missing")
		}
		out.Count = s.ColumnInt64(0)
		out.Sum = s.ColumnInt64(1)
	default:
		panic("unknown case")
	}
	check(s.Err())
	check(s.Reset())
	if name == "point_txn" {
		check(r.reader.Exec(`COMMIT`))
	}
	return out
}

func (r *raw) Digest() uint64 {
	h := uint64(14695981039346656037)
	s := prepared(r.reader, r.readers, digestSQL)
	for s.Step() {
		h = hashWord(h, s.ColumnInt64(0))
		h = hashWord(h, s.ColumnInt64(1))
		h = hashBytes(h, s.ColumnRawBlob(2))
	}
	check(s.Err())
	check(s.Reset())
	return h
}

func rawInfo(conn *sqlite3.Conn) map[string]any {
	out := map[string]any{}
	for _, name := range []string{"page_size", "foreign_keys", "busy_timeout", "synchronous", "fullfsync", "checkpoint_fullfsync", "cache_size", "query_only", "trusted_schema", "wal_autocheckpoint", "mmap_size"} {
		s, _, err := conn.Prepare("PRAGMA " + name)
		check(err)
		if !s.Step() {
			panic("pragma missing")
		}
		out[name] = s.ColumnInt64(0)
		check(s.Close())
	}
	for _, q := range []string{"SELECT sqlite_version()", "SELECT sqlite_source_id()", "PRAGMA journal_mode"} {
		s, _, err := conn.Prepare(q)
		check(err)
		if !s.Step() {
			panic("version missing")
		}
		value := s.ColumnText(0)
		name := "version"
		if q == "SELECT sqlite_source_id()" {
			name = "source_id"
		}
		if q == "PRAGMA journal_mode" {
			name = "journal_mode"
		}
		out[name] = value
		check(s.Close())
	}
	current, _, err := conn.Status(sqlite3.DBSTATUS_CACHE_USED, false)
	check(err)
	out["cache_used_bytes"] = current
	return out
}

func (r *raw) Inspect() map[string]any {
	var options []string
	s, _, err := r.reader.Prepare("PRAGMA compile_options")
	check(err)
	for s.Step() {
		options = append(options, s.ColumnText(0))
	}
	check(s.Err())
	check(s.Close())
	return map[string]any{"reader": rawInfo(r.reader), "writer": rawInfo(r.writer), "compile_options": options, "statement_cache_capacity": 32, "cache_note": "direct prepared statements; only measured programs, no eviction", "max_length": 1 << 20}
}
