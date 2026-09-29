package main

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"time"
)

// recordsStore is what every log contender does: keep a batch of lines durably,
// settle into the state it keeps on disk, and count one stream's lines in a
// range of time.
type recordsStore interface {
	subject
	append(ctx context.Context, lines []logLine) error
	settle(ctx context.Context) error
	count(ctx context.Context, stream string, from, to time.Time) (int, error)
}

// logLine is one line of a program's output, as a log collector sees it.
type logLine struct {
	At     time.Time
	Stream string
	Body   string
}

var recordsEngine = engine{
	order: []string{"tinystore", "sqlite", "jsonl", "jsonl-zstd"},
	contenders: map[string]opener{
		"tinystore":  openTinyStoreRecords,
		"sqlite":     openSQLiteRecords,
		"jsonl":      openJSONLRecords(false),
		"jsonl-zstd": openJSONLRecords(true),
	},
	measure: measureRecords,
}

// The corpus is a directory of logs: docker json-file logs, a container a
// directory, as bench/fetch-docker-logs.sh leaves them, or plain text files,
// as LogHub's. COMPARE_LOGS names it; the private production corpus is used
// when it is there, and its report gives aggregates only.
var defaultLogCorpora = []string{"../corpus/prod", "../corpus/loghub"}

// appendLines is how many lines a batch carries, as a collector flushes them
const appendLines = 1000

func measureRecords(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	store, ok := s.(recordsStore)
	if !ok {
		return nil, fmt.Errorf("%T is not a log store", s)
	}
	corpus, err := loadLogs()
	if err != nil {
		return nil, err
	}

	stages := []stage{appendCorpus(ctx, store, corpus)}
	if stages[0].Errors > 0 {
		return stages, fmt.Errorf("append: %s", stages[0].FirstError)
	}
	began := time.Now()
	if err = store.settle(ctx); err != nil {
		return nil, fmt.Errorf("settle: %w", err)
	}
	stages = append(stages, stage{Name: "settle", Goroutines: 1, Ops: 1, Seconds: time.Since(began).Seconds()})
	for _, goroutines := range []int{1, 8} {
		stages = append(stages, timeStage(ctx, "read-window", goroutines, seconds, readLogWindow(store, corpus)))
	}
	return stages, nil
}

// logCorpus is every line, in time order, shifted to end a minute ago so that
// a store with a retention keeps all of it.
type logCorpus struct {
	lines   []logLine
	streams []string
	counts  map[string][]time.Time // each stream's times, for what a read must find
}

var loadLogCorpus = sync.OnceValues(func() (*logCorpus, error) {
	dir := os.Getenv("COMPARE_LOGS")
	if dir == "" {
		for _, candidate := range defaultLogCorpora {
			if _, err := os.Stat(candidate); err == nil {
				dir = candidate
				break
			}
		}
	}
	c := &logCorpus{counts: map[string][]time.Time{}}
	err := filepath.WalkDir(dir, func(path string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() || !strings.HasSuffix(path, ".log") {
			return err
		}
		return c.read(path)
	})
	if err != nil {
		return nil, fmt.Errorf("the log corpus in %q: %w", dir, err)
	}
	c.settle()
	return c, nil
})

func loadLogs() (*logCorpus, error) { return loadLogCorpus() }

// read takes a file's lines: a docker json-file line is its log and time, and
// any other line is text whose time is the next millisecond of its file
func (c *logCorpus) read(path string) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close()
	stream := filepath.Base(filepath.Dir(path))
	if !strings.HasSuffix(path, "-json.log") {
		stream = strings.TrimSuffix(filepath.Base(path), ".log")
	}
	at := time.Unix(0, 0)
	lines := bufio.NewScanner(f)
	lines.Buffer(make([]byte, 1<<20), 16<<20)
	for lines.Scan() {
		var docker struct {
			Log  string    `json:"log"`
			Time time.Time `json:"time"`
		}
		line := logLine{Stream: stream, Body: lines.Text()}
		if json.Unmarshal(lines.Bytes(), &docker) == nil && !docker.Time.IsZero() {
			line.At, line.Body = docker.Time, strings.TrimSuffix(docker.Log, "\n")
		} else {
			at = at.Add(time.Millisecond)
			line.At = at
		}
		c.lines = append(c.lines, line)
	}
	return lines.Err()
}

func (c *logCorpus) settle() {
	slices.SortStableFunc(c.lines, func(a, b logLine) int { return a.At.Compare(b.At) })
	shift := time.Now().Add(-time.Minute).Sub(c.lines[len(c.lines)-1].At)
	for i := range c.lines {
		c.lines[i].At = c.lines[i].At.Add(shift).UTC()
		stream := c.lines[i].Stream
		if _, seen := c.counts[stream]; !seen {
			c.streams = append(c.streams, stream)
		}
		c.counts[stream] = append(c.counts[stream], c.lines[i].At)
	}
}

// appendCorpus sends every line in batches from one writer and times it all
func appendCorpus(ctx context.Context, store recordsStore, c *logCorpus) stage {
	var latencies histogram
	s := stage{Name: "append", Goroutines: 1, Ops: int64(len(c.lines))}
	before := spent()
	began := time.Now()
	for start := 0; start < len(c.lines); start += appendLines {
		sent := time.Now()
		if err := store.append(ctx, c.lines[start:min(start+appendLines, len(c.lines))]); err != nil {
			s.Errors++
			if s.FirstError == "" {
				s.FirstError = err.Error()
			}
		}
		latencies.add(time.Since(sent))
	}
	s.Seconds = time.Since(began).Seconds()
	s.PerSecond = float64(s.Ops) / s.Seconds
	s.P50Micros, s.P99Micros = latencies.quantile(0.5), latencies.quantile(0.99)
	s.addUsage(before, spent())
	return s
}

// readLogWindow counts ten minutes of one stream, a different one each call,
// and checks the count against the corpus
func readLogWindow(store recordsStore, c *logCorpus) operation {
	return func(ctx context.Context, worker, n int) error {
		index := pick(worker, n, 31) % len(c.streams)
		stream := c.streams[index]
		times := c.counts[stream]
		from := times[pick(worker, n, 32)%len(times)]
		to := from.Add(10 * time.Minute)
		first, _ := slices.BinarySearchFunc(times, from, func(a, b time.Time) int { return a.Compare(b) })
		last, _ := slices.BinarySearchFunc(times, to, func(a, b time.Time) int { return a.Compare(b) })
		got, err := store.count(ctx, stream, from, to)
		if err == nil && got != last-first {
			// a stream of the private corpus is a container, named by number only
			err = fmt.Errorf("stream %d from %s: %d lines, want %d", index, from, got, last-first)
		}
		return err
	}
}
