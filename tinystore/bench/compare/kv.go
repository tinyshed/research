package main

import (
	"context"
	"encoding/binary"
	"fmt"
	"sync"
)

// kvStore is what every key-value contender does: a durable write of one key,
// committed and synced before it returns, and a read of one.
type kvStore interface {
	subject
	set(ctx context.Context, key string, value []byte) error
	get(ctx context.Context, key string) ([]byte, error)
}

const (
	kvKeys       = 100_000
	kvValueBytes = 128
)

var kvEngine = engine{
	order: []string{"tinystore", "sqlite", "bbolt", "badger", "pebble", "redis"},
	contenders: map[string]opener{
		"tinystore": openTinyStoreKV,
		"sqlite":    openSQLiteKV,
		"bbolt":     openBoltKV,
		"badger":    openBadgerKV,
		"pebble":    openPebbleKV,
		"redis":     openRedisKV,
	},
	measure: measureKV,
}

// measureKV fills the key space, then times reads, writes and a mix of nine
// reads to one write, each from 1, 8 and 64 goroutines.
func measureKV(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	store, ok := s.(kvStore)
	if !ok {
		return nil, fmt.Errorf("%T is not a key-value store", s)
	}
	if err := fillKV(ctx, store); err != nil {
		return nil, fmt.Errorf("fill: %w", err)
	}

	var stages []stage
	for _, load := range []struct {
		name string
		op   func(kvStore) operation
	}{
		{"get", kvGet},
		{"set", kvSet},
		{"mixed", kvMixed},
	} {
		for _, goroutines := range []int{1, 8, 64} {
			stages = append(stages, timeStage(ctx, load.name, goroutines, seconds, load.op(store)))
		}
	}
	return stages, nil
}

// fillKV writes every key once from 64 goroutines, untimed
func fillKV(ctx context.Context, store kvStore) error {
	const writers = 64
	errs := make([]error, writers)
	var wg sync.WaitGroup
	for w := range writers {
		wg.Go(func() {
			for i := w; i < kvKeys && errs[w] == nil; i += writers {
				errs[w] = store.set(ctx, kvKey(i), kvValue(i, 0))
			}
		})
	}
	wg.Wait()
	for _, err := range errs {
		if err != nil {
			return err
		}
	}
	return nil
}

func kvGet(store kvStore) operation {
	return func(ctx context.Context, worker, n int) error {
		i := pick(worker, n, 1)
		value, err := store.get(ctx, kvKey(i))
		if err == nil && len(value) != kvValueBytes {
			err = fmt.Errorf("%s: %d bytes, want %d", kvKey(i), len(value), kvValueBytes)
		}
		return err
	}
}

func kvSet(store kvStore) operation {
	return func(ctx context.Context, worker, n int) error {
		i := pick(worker, n, 2)
		return store.set(ctx, kvKey(i), kvValue(i, n))
	}
}

func kvMixed(store kvStore) operation {
	get, set := kvGet(store), kvSet(store)
	return func(ctx context.Context, worker, n int) error {
		if pick(worker, n, 3)%10 == 0 {
			return set(ctx, worker, n)
		}
		return get(ctx, worker, n)
	}
}

// pick is a key for a worker's nth call, the same on every run and without a
// generator shared between goroutines: splitmix64 over the three numbers.
func pick(worker, n, stream int) int {
	x := uint64(worker)<<40 ^ uint64(n)<<8 ^ uint64(stream)
	x += 0x9e3779b97f4a7c15
	x = (x ^ x>>30) * 0xbf58476d1ce4e5b9
	x = (x ^ x>>27) * 0x94d049bb133111eb
	x ^= x >> 31
	return int(x % kvKeys)
}

func kvKey(i int) string {
	return fmt.Sprintf("user:%08d", i)
}

// kvValue is a value that changes with each write of its key and does not
// compress to nothing, as a session's JSON would not
func kvValue(i, version int) []byte {
	value := make([]byte, kvValueBytes)
	x := uint64(i)*0x9e3779b97f4a7c15 + uint64(version)
	for at := 0; at < len(value); at += 8 {
		x ^= x << 13
		x ^= x >> 7
		x ^= x << 17
		binary.LittleEndian.PutUint64(value[at:], x)
	}
	return value
}
