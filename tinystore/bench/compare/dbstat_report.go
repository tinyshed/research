//go:build ignore

// Run after the contender closes: go run dbstat_report.go <file> [<file>...].
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/tinyshed/tinystore/internal/dbstat"
)

type filePages struct {
	File       string
	Bytes      int64
	OtherBytes int64 // freelist, pointer maps and SQLite's lock-byte page
	Objects    []dbstat.Object
}

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: go run dbstat_report.go <closed-file> [<closed-file>...]")
		os.Exit(1)
	}
	for _, path := range os.Args[1:] {
		pages, err := readPages(context.Background(), path)
		if err == nil {
			err = json.NewEncoder(os.Stdout).Encode(pages)
		}
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
	}
}

func readPages(ctx context.Context, path string) (filePages, error) {
	objects, err := dbstat.Read(ctx, path)
	if err != nil {
		return filePages{}, err
	}
	info, err := os.Stat(path)
	if err != nil {
		return filePages{}, err
	}

	pages := filePages{File: filepath.Base(path), Bytes: info.Size(), Objects: objects}
	pages.OtherBytes = pages.Bytes
	for _, object := range objects {
		pages.OtherBytes -= object.Bytes
	}
	return pages, nil
}
