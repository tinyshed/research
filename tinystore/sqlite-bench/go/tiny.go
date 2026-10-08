package main

import (
	"context"
	"database/sql"
	"os"

	"github.com/tinyshed/tinystore/internal/sqlite"
)

func backgroundContext() context.Context { return context.Background() }

type tiny struct{ file *sqlite.File }

func openTiny(path string) *tiny {
	f, err := sqlite.Open(ctx, path, sqlite.Config{Readers: 1, Statements: 32, MaxLength: 1 << 20})
	check(err)
	return &tiny{f}
}
func (r *tiny) Close() { check(r.file.Close()) }

func fixture(path string, page int) {
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		panic("fixture path already exists")
	}
	f, err := sqlite.Open(ctx, path, sqlite.Config{Readers: 1, PageSize: page, Statements: 32, MaxLength: 1 << 20})
	check(err)
	check(f.UpdatePrepared(ctx, func(w sqlite.Writer) error {
		if _, err := w.ExecContext(ctx, `CREATE TABLE blocks(series INTEGER NOT NULL,at INTEGER NOT NULL,body BLOB NOT NULL,PRIMARY KEY(series,at)) WITHOUT ROWID`); err != nil {
			return err
		}
		for i := uint64(0); i < 32768; i++ {
			if _, err := w.ExecContext(ctx, insertSQL, int64(i/512), int64(i%512), body(i)); err != nil {
				return err
			}
		}
		return nil
	}))
	check(f.Close())
	emit(map[string]any{"fixture_rows": 32768, "blob_bytes": 128, "page_size": page})
}

func (r *tiny) read(reader sqlite.Reader, name string, i uint64) (out result) {
	series, at := key(i)
	switch name {
	case "point", "point_txn":
		check(sqlite.QueryRowByKey(ctx, reader, pointSQL, series, at).Scan(&out.Blob))
		out.Count = 1
	case "range240":
		start := at % 273
		rows, err := reader.QueryContext(ctx, rangeSQL, series, start, start+240, 240)
		check(err)
		check(sqlite.EachRow(rows, "benchmark range", func(rows *sql.Rows) error {
			var value row
			if err := rows.Scan(&value.At, &value.Body); err != nil {
				return err
			}
			out.Rows = append(out.Rows, value)
			return nil
		}))
		out.Count = int64(len(out.Rows))
	case "aggregate240":
		start := at % 273
		check(sqlite.QueryRow(ctx, reader, aggregateSQL, series, start, start+240).Scan(&out.Count, &out.Sum))
	default:
		panic("unknown read")
	}
	return out
}

func (r *tiny) Run(name string, i uint64) (out result) {
	if name == "update1" || name == "update64" {
		check(r.file.UpdatePrepared(ctx, func(w sqlite.Writer) error {
			n := uint64(1)
			if name == "update64" {
				n = 64
			}
			for j := uint64(0); j < n; j++ {
				q := i
				if n == 64 {
					q = i*64 + j
					if _, err := w.ExecContext(ctx, `SAVEPOINT grouped`); err != nil {
						return err
					}
				}
				series, at := key(q)
				changed, err := w.ExecContext(ctx, updateSQL, body(q), series, at)
				if err != nil {
					return err
				}
				count, err := changed.RowsAffected()
				if err != nil {
					return err
				}
				out.Affected += count
				if n == 64 {
					if _, err := w.ExecContext(ctx, `RELEASE grouped`); err != nil {
						return err
					}
				}
			}
			return nil
		}))
		return out
	}
	read := func(reader sqlite.Reader) error { out = r.read(reader, name, i); return nil }
	if name == "point_txn" {
		check(r.file.ViewPrepared(ctx, read))
	} else {
		check(r.file.Lookup(ctx, read))
	}
	return out
}

func (r *tiny) Digest() uint64 {
	h := uint64(14695981039346656037)
	check(r.file.Lookup(ctx, func(reader sqlite.Reader) error {
		rows, err := reader.QueryContext(ctx, digestSQL)
		if err != nil {
			return err
		}
		return sqlite.EachRow(rows, "digest", func(rows *sql.Rows) error {
			var series, at int64
			var b []byte
			if err := rows.Scan(&series, &at, &b); err != nil {
				return err
			}
			h = hashWord(h, series)
			h = hashWord(h, at)
			h = hashBytes(h, b)
			return nil
		})
	}))
	return h
}

func readerInfo(reader sqlite.Reader) map[string]any {
	info := map[string]any{}
	for _, name := range []string{"page_size", "foreign_keys", "busy_timeout", "synchronous", "fullfsync", "checkpoint_fullfsync", "cache_size", "query_only", "trusted_schema", "wal_autocheckpoint", "mmap_size"} {
		var v int64
		check(sqlite.QueryRow(ctx, reader, "PRAGMA "+name).Scan(&v))
		info[name] = v
	}
	var version, source, journal string
	check(sqlite.QueryRow(ctx, reader, "SELECT sqlite_version(),sqlite_source_id()").Scan(&version, &source))
	check(sqlite.QueryRow(ctx, reader, "PRAGMA journal_mode").Scan(&journal))
	info["version"] = version
	info["source_id"] = source
	info["journal_mode"] = journal
	return info
}

func (r *tiny) Inspect() map[string]any {
	out := map[string]any{"statement_cache_capacity": 32, "max_length": 1 << 20}
	check(r.file.Lookup(ctx, func(reader sqlite.Reader) error {
		out["reader"] = readerInfo(reader)
		var options []string
		rows, err := reader.QueryContext(ctx, "PRAGMA compile_options")
		if err != nil {
			return err
		}
		err = sqlite.EachRow(rows, "compile options", func(rows *sql.Rows) error {
			var v string
			if err := rows.Scan(&v); err != nil {
				return err
			}
			options = append(options, v)
			return nil
		})
		out["compile_options"] = options
		return err
	}))
	check(r.file.UpdatePrepared(ctx, func(w sqlite.Writer) error { out["writer"] = readerInfo(w); return nil }))
	counts, err := r.file.WriterCounters(ctx)
	check(err)
	out["writer_counters"] = counts
	return out
}
