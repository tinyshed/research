package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// weighed is what linking one contender costs a program: a program that opens
// it, writes a key and reads it back, against an empty one, both built
// without cgo for linux/amd64 and stripped, as a release is.
type weighed struct {
	Program string `json:"program"`
	Bytes   int64  `json:"bytes"`
	Added   int64  `json:"added_bytes"`
	Error   string `json:"error,omitempty"`
}

func weighAll(ctx context.Context, args []string) error {
	flags := flag.NewFlagSet("weight", flag.ContinueOnError)
	out := flags.String("out", "", "the JSON file the weights are written to")
	if err := flags.Parse(args); err != nil {
		return err
	}
	programs, err := os.ReadDir("weight")
	if err != nil {
		return fmt.Errorf("run it from bench/compare: %w", err)
	}
	built, err := os.MkdirTemp("", "compare-weight-")
	if err != nil {
		return err
	}
	defer os.RemoveAll(built)

	baseline, err := weigh(ctx, built, "empty")
	if err != nil {
		return err
	}
	var weights []weighed
	for _, p := range programs {
		w := weighed{Program: p.Name()}
		if w.Bytes, err = weigh(ctx, built, p.Name()); err != nil {
			w.Error = err.Error()
		} else {
			w.Added = w.Bytes - baseline
		}
		fmt.Fprintf(os.Stderr, "%-10s %6.1f MiB  +%.1f MiB %s\n", w.Program, float64(w.Bytes)/(1<<20),
			float64(w.Added)/(1<<20), w.Error)
		weights = append(weights, w)
	}
	return writeJSON(*out, weights)
}

func weigh(ctx context.Context, dir, program string) (int64, error) {
	binary := filepath.Join(dir, program)
	build := exec.CommandContext(ctx, "go", "build", "-trimpath", "-ldflags=-s -w", "-o", binary,
		"./weight/"+program)
	build.Env = append(os.Environ(), "CGO_ENABLED=0", "GOOS=linux", "GOARCH=amd64", "GOWORK=off")
	if text, err := build.CombinedOutput(); err != nil {
		return 0, fmt.Errorf("does not build without cgo: %s", strings.TrimSpace(string(text)))
	}
	info, err := os.Stat(binary)
	if err != nil {
		return 0, err
	}
	return info.Size(), nil
}
