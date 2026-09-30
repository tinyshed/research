package main

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/blobs"
)

// blobStore is what every file contender does: put an object durably under a
// key and read one back whole.
type blobStore interface {
	subject
	put(ctx context.Context, key string, body []byte) error
	get(ctx context.Context, key string) (int, error)
}

var blobsEngine = engine{
	order: []string{"tinystore", "files", "sqlite"},
	contenders: map[string]opener{
		"tinystore": openTinyStoreBlobs,
		"files":     openFileBlobs,
		"sqlite":    openSQLiteBlobs,
	},
	measure: measureBlobs,
}

// blobSizes are a small file, which TinyStore keeps in its row, and a large
// one, which it keeps as a file of its own; how many of each are written
// before the reads
var blobSizes = []struct {
	name     string
	bytes    int
	preload  int
	parallel []int
}{
	{"4k", 4 << 10, 2000, []int{1, 64}},
	{"1m", 1 << 20, 100, []int{1, 8}},
}

func measureBlobs(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	store, ok := s.(blobStore)
	if !ok {
		return nil, fmt.Errorf("%T is not a file store", s)
	}
	var stages []stage
	for _, size := range blobSizes {
		body := kvValue(size.bytes, 0)
		for len(body) < size.bytes {
			body = append(body, body...)
		}
		body = body[:size.bytes]
		for i := range size.preload {
			if err := store.put(ctx, "preload/"+size.name+"/"+strconv.Itoa(i), body); err != nil {
				return nil, fmt.Errorf("preload: %w", err)
			}
		}
		for _, goroutines := range size.parallel {
			stages = append(stages, timeStage(ctx, "put-"+size.name, goroutines, seconds,
				func(ctx context.Context, worker, n int) error {
					return store.put(ctx, fmt.Sprintf("put/%s/%d/%d", size.name, worker, n), body)
				}))
			stages = append(stages, timeStage(ctx, "get-"+size.name, goroutines, seconds,
				func(ctx context.Context, worker, n int) error {
					key := "preload/" + size.name + "/" + strconv.Itoa(pick(worker, n, 41)%size.preload)
					got, err := store.get(ctx, key)
					if err == nil && got != size.bytes {
						err = fmt.Errorf("%s: %d bytes, want %d", key, got, size.bytes)
					}
					return err
				}))
		}
	}
	return stages, nil
}

// TinyStore: a blobs bucket, each whole read checked by its SHA-256.

type tinyStoreBlobs struct {
	noService
	store  *tinystore.Store
	bucket *blobs.Bucket
}

func openTinyStoreBlobs(ctx context.Context, dir string) (subject, error) {
	store, err := tinystore.Open(ctx, dir, tinystore.Options{})
	if err != nil {
		return nil, err
	}
	objects, err := blobs.Open(ctx, store, blobs.Options{})
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	bucket, err := blobs.OpenBucket(ctx, objects, "files")
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	return &tinyStoreBlobs{store: store, bucket: bucket}, nil
}

func (t *tinyStoreBlobs) put(ctx context.Context, key string, body []byte) error {
	_, err := t.bucket.Put(ctx, key, bytes.NewReader(body), blobs.Size(int64(len(body))))
	return err
}

func (t *tinyStoreBlobs) get(ctx context.Context, key string) (int, error) {
	r, found, err := t.bucket.Open(ctx, key)
	if err != nil || !found {
		return 0, errors.Join(err, errNotFound(key, found))
	}
	n, err := io.Copy(io.Discard, r)
	return int(n), errors.Join(err, r.Close())
}

func (t *tinyStoreBlobs) close() error { return t.store.Close(context.Background()) }

func errNotFound(key string, found bool) error {
	if found {
		return nil
	}
	return fmt.Errorf("%s: not found", key)
}

// Files: a file a key, written as a careful program writes one so that it is
// whole after a crash: a temporary file, synced, renamed over the key, and
// its directory synced.

type fileBlobs struct {
	noService
	dir string
}

func openFileBlobs(_ context.Context, dir string) (subject, error) {
	return &fileBlobs{dir: dir}, nil
}

func (f *fileBlobs) put(_ context.Context, key string, body []byte) error {
	path := filepath.Join(f.dir, "objects", filepath.FromSlash(key))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".upload-*")
	if err != nil {
		return err
	}
	_, err = tmp.Write(body)
	err = errors.Join(err, tmp.Sync(), tmp.Close())
	if err == nil {
		err = os.Rename(tmp.Name(), path)
	}
	if err != nil {
		return errors.Join(err, os.Remove(tmp.Name()))
	}
	return syncDir(filepath.Dir(path))
}

func syncDir(path string) error {
	d, err := os.Open(path)
	if err != nil {
		return err
	}
	return errors.Join(d.Sync(), d.Close())
}

func (f *fileBlobs) get(_ context.Context, key string) (int, error) {
	body, err := os.ReadFile(filepath.Join(f.dir, "objects", filepath.FromSlash(key)))
	return len(body), err
}

func (f *fileBlobs) close() error { return nil }

// SQLite: a blob column, through a writer pool of one and a pool of readers.

type sqliteBlobs struct {
	noService
	writer, reader *sql.DB
}

func openSQLiteBlobs(ctx context.Context, dir string) (subject, error) {
	dsn := "file:" + filepath.ToSlash(filepath.Join(dir, "blobs.db")) +
		"?_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)&_pragma=busy_timeout(5000)"
	writer, err := sql.Open("sqlite", dsn+"&_txlock=immediate")
	if err != nil {
		return nil, err
	}
	writer.SetMaxOpenConns(1)
	if _, err = writer.ExecContext(ctx,
		`create table if not exists objects (key text primary key, body blob not null) strict`); err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	reader, err := sql.Open("sqlite", dsn+"&_pragma=query_only(1)")
	if err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	keepReaders(reader)
	return &sqliteBlobs{writer: writer, reader: reader}, nil
}

func (s *sqliteBlobs) put(ctx context.Context, key string, body []byte) error {
	_, err := s.writer.ExecContext(ctx,
		`insert into objects (key, body) values (?, ?) on conflict (key) do update set body = excluded.body`, key, body)
	return err
}

func (s *sqliteBlobs) get(ctx context.Context, key string) (int, error) {
	var body []byte
	err := s.reader.QueryRowContext(ctx, `select body from objects where key = ?`, key).Scan(&body)
	return len(body), err
}

func (s *sqliteBlobs) close() error { return errors.Join(s.reader.Close(), s.writer.Close()) }
