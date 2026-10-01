package probe

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"runtime"
	"runtime/debug"
)

type Open func(context.Context, string) (func(context.Context) error, error)

type sample struct {
	Phase        string
	HeapAlloc    uint64
	HeapInuse    uint64
	HeapIdle     uint64
	HeapReleased uint64
	StackInuse   uint64
	Sys          uint64
	NumGC        uint32
	Goroutines   int
}

func Main(open Open) {
	if err := run(open); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(open Open) (err error) {
	if len(os.Args) != 2 {
		return errors.New("one empty store directory is required")
	}
	ctx := context.Background()
	closeStore := func(context.Context) error { return nil }
	defer func() { err = errors.Join(err, closeStore(ctx)) }()
	if err := emit("linked"); err != nil {
		return err
	}

	var command [1]byte
	for {
		if _, err := io.ReadFull(os.Stdin, command[:]); err != nil {
			return err
		}
		switch command[0] {
		case 'o':
			if open != nil {
				opened, openErr := open(ctx, os.Args[1])
				if openErr != nil {
					return openErr
				}
				closeStore = opened
			}
			if err := emit("opened"); err != nil {
				return err
			}
		case 'g':
			runtime.GC()
			if err := emit("gc"); err != nil {
				return err
			}
		case 'r':
			debug.FreeOSMemory()
			if err := emit("released"); err != nil {
				return err
			}
		case 'q':
			return nil
		default:
			return fmt.Errorf("unknown command %q", command[0])
		}
	}
}

func emit(phase string) error {
	var memory runtime.MemStats
	runtime.ReadMemStats(&memory)
	return json.NewEncoder(os.Stdout).Encode(sample{
		Phase: phase, HeapAlloc: memory.HeapAlloc, HeapInuse: memory.HeapInuse,
		HeapIdle: memory.HeapIdle, HeapReleased: memory.HeapReleased,
		StackInuse: memory.StackInuse, Sys: memory.Sys,
		NumGC: memory.NumGC, Goroutines: runtime.NumGoroutine(),
	})
}
