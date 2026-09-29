package main

import (
	"bufio"
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"time"

	"github.com/klauspost/compress/zstd"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/records"
)

// TinyStore: its records engine, with a retention longer than the corpus's
// fourteen days and heads sealed after a second, so that settling leaves the
// segments its background work would leave in time.

type tinyStoreRecords struct {
	noService
	store   *tinystore.Store
	records *records.Store
}

const tinyStoreSealAge = time.Second

func openTinyStoreRecords(ctx context.Context, dir string) (subject, error) {
	store, err := tinystore.Open(ctx, dir, tinystore.Options{})
	if err != nil {
		return nil, err
	}
	r, err := records.Open(ctx, store, records.Options{Retention: 30 * 24 * time.Hour, SealAge: tinyStoreSealAge})
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	return &tinyStoreRecords{store: store, records: r}, nil
}

func (t *tinyStoreRecords) append(ctx context.Context, lines []logLine) error {
	batch := make([]records.Record, len(lines))
	for i, line := range lines {
		body := line.Body
		batch[i] = records.Record{At: line.At, Stream: line.Stream, Name: "log", Body: &body}
	}
	return t.records.Append(ctx, batch...)
}

func (t *tinyStoreRecords) settle(ctx context.Context) error {
	time.Sleep(tinyStoreSealAge + 100*time.Millisecond)
	for range 10_000 {
		done, err := t.records.Maintain(ctx)
		if err != nil || done.SealedRecords == 0 && done.MergedSegments == 0 {
			return err
		}
	}
	return errors.New("maintenance still working after ten thousand passes")
}

func (t *tinyStoreRecords) count(ctx context.Context, stream string, from, to time.Time) (int, error) {
	query := records.Query{From: from, To: to, Streams: []string{stream}, Limit: 10_000}
	n := 0
	for {
		page, err := t.records.Read(ctx, query)
		if err != nil {
			return n, err
		}
		n += len(page.Records)
		if !page.More {
			return n, nil
		}
		query = page.Next
	}
}

func (t *tinyStoreRecords) close() error { return t.store.Close(context.Background()) }

// SQLite: one table with an index on stream and time, a batch one
// transaction, through a writer pool of one and a pool of readers.

type sqliteRecords struct {
	noService
	writer, reader *sql.DB
}

const (
	logsSchema = `create table if not exists logs (at integer not null, stream text not null, body text not null) strict;
create index if not exists logs_by_stream on logs (stream, at)`
	logsInsert = `insert into logs (at, stream, body) values (?, ?, ?)`
	logsCount  = `select count(*) from logs where stream = ? and at >= ? and at < ?`
)

func openSQLiteRecords(ctx context.Context, dir string) (subject, error) {
	dsn := "file:" + filepath.ToSlash(filepath.Join(dir, "logs.db")) +
		"?_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)&_pragma=busy_timeout(5000)"
	writer, err := sql.Open("sqlite", dsn+"&_txlock=immediate")
	if err != nil {
		return nil, err
	}
	writer.SetMaxOpenConns(1)
	if _, err = writer.ExecContext(ctx, logsSchema); err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	reader, err := sql.Open("sqlite", dsn+"&_pragma=query_only(1)")
	if err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	return &sqliteRecords{writer: writer, reader: reader}, nil
}

func (s *sqliteRecords) append(ctx context.Context, lines []logLine) error {
	tx, err := s.writer.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	insert, err := tx.PrepareContext(ctx, logsInsert)
	if err != nil {
		return errors.Join(err, tx.Rollback())
	}
	for _, line := range lines {
		if _, err = insert.ExecContext(ctx, line.At.UnixNano(), line.Stream, line.Body); err != nil {
			return errors.Join(err, tx.Rollback())
		}
	}
	return tx.Commit()
}

func (s *sqliteRecords) settle(ctx context.Context) error {
	_, err := s.writer.ExecContext(ctx, `pragma wal_checkpoint(truncate)`)
	return err
}

func (s *sqliteRecords) count(ctx context.Context, stream string, from, to time.Time) (int, error) {
	var n int
	err := s.reader.QueryRowContext(ctx, logsCount, stream, from.UnixNano(), to.UnixNano()).Scan(&n)
	return n, err
}

func (s *sqliteRecords) close() error { return errors.Join(s.reader.Close(), s.writer.Close()) }

// JSON lines: a file a stream, a line a record, as docker's json-file driver
// keeps them, each batch synced; compressed, each file zstd'd once written, as
// a log rotation leaves it. A read scans its stream's file.

type jsonlRecords struct {
	noService
	dir      string
	compress bool
	files    map[string]*os.File
}

func openJSONLRecords(compress bool) opener {
	return func(_ context.Context, dir string) (subject, error) {
		return &jsonlRecords{dir: dir, compress: compress, files: map[string]*os.File{}}, nil
	}
}

func (j *jsonlRecords) append(_ context.Context, lines []logLine) error {
	touched := map[string]*bytes.Buffer{}
	for _, line := range lines {
		buffer := touched[line.Stream]
		if buffer == nil {
			buffer = &bytes.Buffer{}
			touched[line.Stream] = buffer
		}
		body, err := json.Marshal(line.Body)
		if err != nil {
			return err
		}
		buffer.WriteString(`{"t":` + strconv.FormatInt(line.At.UnixNano(), 10) + `,"log":`)
		buffer.Write(body)
		buffer.WriteString("}\n")
	}
	for stream, buffer := range touched {
		f, err := j.file(stream)
		if err != nil {
			return err
		}
		if _, err = f.Write(buffer.Bytes()); err != nil {
			return err
		}
		if err = f.Sync(); err != nil {
			return err
		}
	}
	return nil
}

func (j *jsonlRecords) file(stream string) (*os.File, error) {
	if f, ok := j.files[stream]; ok {
		return f, nil
	}
	f, err := os.OpenFile(j.path(stream), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	j.files[stream] = f
	return f, err
}

func (j *jsonlRecords) path(stream string) string {
	return filepath.Join(j.dir, stream+".jsonl")
}

// settle closes the files and, compressed, replaces each with its zstd
func (j *jsonlRecords) settle(_ context.Context) error {
	for stream, f := range j.files {
		if err := f.Close(); err != nil {
			return err
		}
		if j.compress {
			if err := compressFile(j.path(stream)); err != nil {
				return err
			}
		}
	}
	j.files = map[string]*os.File{}
	return nil
}

func compressFile(path string) error {
	in, err := os.Open(path)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.Create(path + ".zst")
	if err != nil {
		return err
	}
	w, err := zstd.NewWriter(out)
	if err != nil {
		return errors.Join(err, out.Close())
	}
	if _, err = io.Copy(w, in); err != nil {
		return errors.Join(err, w.Close(), out.Close())
	}
	if err = errors.Join(w.Close(), out.Sync(), out.Close()); err != nil {
		return err
	}
	return os.Remove(path)
}

func (j *jsonlRecords) count(_ context.Context, stream string, from, to time.Time) (int, error) {
	var source io.Reader
	f, err := os.Open(j.path(stream))
	if errors.Is(err, os.ErrNotExist) && j.compress {
		if f, err = os.Open(j.path(stream) + ".zst"); err == nil {
			decoder, derr := zstd.NewReader(f, zstd.WithDecoderConcurrency(1))
			if derr != nil {
				return 0, errors.Join(derr, f.Close())
			}
			defer decoder.Close()
			source = decoder
		}
	} else {
		source = f
	}
	if err != nil {
		return 0, err
	}
	defer f.Close()
	return countJSONL(source, from.UnixNano(), to.UnixNano())
}

// countJSONL counts the lines whose time is in [from, to), reading each
// line's time as the text it starts with
func countJSONL(r io.Reader, from, to int64) (int, error) {
	n := 0
	lines := bufio.NewScanner(r)
	lines.Buffer(make([]byte, 1<<20), 16<<20)
	for lines.Scan() {
		line := lines.Bytes()
		end := bytes.IndexByte(line, ',')
		if end < 5 {
			continue
		}
		at, err := strconv.ParseInt(string(line[5:end]), 10, 64)
		if err != nil {
			return n, err
		}
		if at >= from && at < to {
			n++
		}
	}
	return n, lines.Err()
}

func (j *jsonlRecords) close() error {
	var err error
	for _, f := range j.files {
		err = errors.Join(err, f.Close())
	}
	return err
}
