package main

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/cockroachdb/pebble/v2"
	"github.com/dgraph-io/badger/v4"
	"github.com/redis/go-redis/v9"
	bolt "go.etcd.io/bbolt"
	_ "modernc.org/sqlite"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/kv"
)

// Every contender writes durably, one commit synced to the disk a call, as
// TinyStore's files do (WAL, synchronous=FULL). Where a library groups the
// commits of concurrent callers itself, that is its idiomatic write and is
// used: bbolt's Batch, Badger's and Pebble's commit pipelines.

type noService struct{}

func (noService) servicePID() int { return 0 }

// TinyStore: a kv bucket of bytes.

type tinyStoreKV struct {
	noService
	store  *tinystore.Store
	bucket *kv.Bucket[[]byte]
}

func openTinyStoreKV(ctx context.Context, dir string) (subject, error) {
	store, err := tinystore.Open(ctx, dir, tinystore.Options{})
	if err != nil {
		return nil, err
	}
	state, err := kv.Open(ctx, store, kv.Options{})
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	bucket, err := kv.OpenBucket[[]byte](ctx, state, "sessions")
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	return &tinyStoreKV{store: store, bucket: bucket}, nil
}

func (t *tinyStoreKV) set(ctx context.Context, key string, value []byte) error {
	return t.bucket.Set(ctx, key, value)
}

func (t *tinyStoreKV) get(ctx context.Context, key string) ([]byte, error) {
	value, found, err := t.bucket.Get(ctx, key)
	if err == nil && !found {
		err = fmt.Errorf("%s: not found", key)
	}
	return value, err
}

func (t *tinyStoreKV) close() error { return t.store.Close(context.Background()) }

// SQLite: one table through database/sql and modernc, as a careful
// application keeps one without TinyStore: TinyStore's pragmas, a writer pool
// of one connection, since SQLite has one writer and a pool of many fails its
// writers with SQLITE_BUSY, and a pool of readers beside it.

type sqliteKV struct {
	noService
	writer, reader *sql.DB
}

const (
	sqliteKVSchema = `create table if not exists kv (key text primary key, value blob not null) strict`
	sqliteKVSet    = `insert into kv (key, value) values (?, ?) on conflict (key) do update set value = excluded.value`
	sqliteKVGet    = `select value from kv where key = ?`
)

func openSQLiteKV(ctx context.Context, dir string) (subject, error) {
	dsn := "file:" + filepath.ToSlash(filepath.Join(dir, "kv.db")) +
		"?_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)&_pragma=busy_timeout(5000)"
	writer, err := sql.Open("sqlite", dsn+"&_txlock=immediate")
	if err != nil {
		return nil, err
	}
	writer.SetMaxOpenConns(1)
	if _, err = writer.ExecContext(ctx, sqliteKVSchema); err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	reader, err := sql.Open("sqlite", dsn+"&_pragma=query_only(1)")
	if err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	return &sqliteKV{writer: writer, reader: reader}, nil
}

func (s *sqliteKV) set(ctx context.Context, key string, value []byte) error {
	_, err := s.writer.ExecContext(ctx, sqliteKVSet, key, value)
	return err
}

func (s *sqliteKV) get(ctx context.Context, key string) ([]byte, error) {
	var value []byte
	err := s.reader.QueryRowContext(ctx, sqliteKVGet, key).Scan(&value)
	return value, err
}

func (s *sqliteKV) close() error { return errors.Join(s.reader.Close(), s.writer.Close()) }

// bbolt: one bucket, writes through Batch, which joins concurrent callers
// into one synced transaction.

type boltKV struct {
	noService
	db *bolt.DB
}

var boltBucket = []byte("kv")

func openBoltKV(_ context.Context, dir string) (subject, error) {
	db, err := bolt.Open(filepath.Join(dir, "kv.bolt"), 0o600, &bolt.Options{Timeout: time.Second})
	if err != nil {
		return nil, err
	}
	err = db.Update(func(tx *bolt.Tx) error {
		_, err := tx.CreateBucketIfNotExists(boltBucket)
		return err
	})
	if err != nil {
		return nil, errors.Join(err, db.Close())
	}
	return &boltKV{db: db}, nil
}

func (b *boltKV) set(_ context.Context, key string, value []byte) error {
	return b.db.Batch(func(tx *bolt.Tx) error {
		return tx.Bucket(boltBucket).Put([]byte(key), value)
	})
}

func (b *boltKV) get(_ context.Context, key string) ([]byte, error) {
	var value []byte
	err := b.db.View(func(tx *bolt.Tx) error {
		found := tx.Bucket(boltBucket).Get([]byte(key))
		if found == nil {
			return fmt.Errorf("%s: not found", key)
		}
		value = append([]byte(nil), found...) // valid only inside the transaction
		return nil
	})
	return value, err
}

func (b *boltKV) close() error { return b.db.Close() }

// Badger: SyncWrites, which Badger leaves off unless asked.

type badgerKV struct {
	noService
	db *badger.DB
}

func openBadgerKV(_ context.Context, dir string) (subject, error) {
	db, err := badger.Open(badger.DefaultOptions(dir).WithSyncWrites(true).WithLogger(nil))
	if err != nil {
		return nil, err
	}
	return &badgerKV{db: db}, nil
}

func (b *badgerKV) set(_ context.Context, key string, value []byte) error {
	return b.db.Update(func(txn *badger.Txn) error {
		return txn.Set([]byte(key), value)
	})
}

func (b *badgerKV) get(_ context.Context, key string) ([]byte, error) {
	var value []byte
	err := b.db.View(func(txn *badger.Txn) error {
		item, err := txn.Get([]byte(key))
		if err != nil {
			return err
		}
		value, err = item.ValueCopy(nil)
		return err
	})
	return value, err
}

func (b *badgerKV) close() error { return b.db.Close() }

// Pebble: each Set synced, which its commit pipeline groups.

type pebbleKV struct {
	noService
	db *pebble.DB
}

func openPebbleKV(_ context.Context, dir string) (subject, error) {
	db, err := pebble.Open(dir, &pebble.Options{})
	if err != nil {
		return nil, err
	}
	return &pebbleKV{db: db}, nil
}

func (p *pebbleKV) set(_ context.Context, key string, value []byte) error {
	return p.db.Set([]byte(key), value, pebble.Sync)
}

func (p *pebbleKV) get(_ context.Context, key string) ([]byte, error) {
	found, closer, err := p.db.Get([]byte(key))
	if err != nil {
		return nil, err
	}
	value := append([]byte(nil), found...) // valid only until closer is closed
	return value, closer.Close()
}

func (p *pebbleKV) close() error { return p.db.Close() }

// Redis: a server of its own on a Unix socket in the run's directory, with an
// append-only file synced on every write.

type redisKV struct {
	client *redis.Client
	server *exec.Cmd
}

func openRedisKV(ctx context.Context, dir string) (subject, error) {
	client, server, err := startRedis(ctx, dir)
	if err != nil {
		return nil, err
	}
	return &redisKV{client: client, server: server}, nil
}

// startRedis starts a server of its own on a Unix socket in dir, with an
// append-only file synced on every write, and waits until it answers
func startRedis(ctx context.Context, dir string) (*redis.Client, *exec.Cmd, error) {
	socket := filepath.Join(dir, "redis.sock")
	server := exec.Command("redis-server", "--port", "0", "--unixsocket", socket, "--dir", dir,
		"--appendonly", "yes", "--appendfsync", "always", "--save", "", "--daemonize", "no")
	server.Stdout, server.Stderr = os.Stderr, os.Stderr
	if err := server.Start(); err != nil {
		return nil, nil, fmt.Errorf("redis-server: %w", err)
	}
	client := redis.NewClient(&redis.Options{Network: "unix", Addr: socket})
	for deadline := time.Now().Add(10 * time.Second); ; time.Sleep(20 * time.Millisecond) {
		if client.Ping(ctx).Err() == nil {
			return client, server, nil
		}
		if time.Now().After(deadline) {
			err := errors.Join(errors.New("redis-server did not answer within ten seconds"),
				(&redisKV{client: client, server: server}).close())
			return nil, nil, err
		}
	}
}

func (r *redisKV) set(ctx context.Context, key string, value []byte) error {
	return r.client.Set(ctx, key, value, 0).Err()
}

func (r *redisKV) get(ctx context.Context, key string) ([]byte, error) {
	return r.client.Get(ctx, key).Bytes()
}

func (r *redisKV) servicePID() int { return r.server.Process.Pid }

// close shuts the server down as an operator would, SHUTDOWN, so the append
// file is complete when its size is read
func (r *redisKV) close() error {
	_ = r.client.Shutdown(context.Background()).Err()
	err := r.client.Close()
	return errors.Join(err, r.server.Wait())
}
