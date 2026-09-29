package main

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"sync"
	"time"
)

// metricsStore is what every metrics contender does: take a window of samples
// for many series, bring what it took to the state it keeps on disk, and read
// back one series or every series one label matches.
type metricsStore interface {
	subject
	ingest(ctx context.Context, window []seriesSamples) error
	settle(ctx context.Context) error
	readSeries(ctx context.Context, labels map[string]string, from, to int64) (int, error)
	readMatching(ctx context.Context, name, value string, from, to int64) (int, error)
}

// seriesSamples is one series' samples inside one window, times in unix ms.
type seriesSamples struct {
	Labels map[string]string
	Times  []int64
	Values []float64
}

var metricsEngine = engine{
	order: []string{"tinystore", "prometheus", "victoria"},
	contenders: map[string]opener{
		"tinystore":  openTinyStoreMetrics,
		"prometheus": openPrometheusMetrics,
		"victoria":   openVictoriaMetrics,
	},
	measure: measureMetrics,
}

// The corpus is TSBS DevOps as bench/tsbs writes it, a series a line in
// VictoriaMetrics' import format; COMPARE_METRICS names another file.
const defaultMetricsCorpus = "../corpus/tsbs-packed/series.jsonl"

// wideLabel is what the wide read matches: 303 of TSBS's 2,020 series.
const wideName, wideValue = "region", "eu-west-1"

const metricsWindow = 10 * time.Minute

func measureMetrics(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	store, ok := s.(metricsStore)
	if !ok {
		return nil, fmt.Errorf("%T is not a metrics store", s)
	}
	corpus, err := loadMetricsCorpus()
	if err != nil {
		return nil, err
	}

	stages := []stage{ingestCorpus(ctx, store, corpus)}
	if stages[0].Errors > 0 {
		return stages, fmt.Errorf("ingest: %s", stages[0].FirstError)
	}
	began := time.Now()
	if err = store.settle(ctx); err != nil {
		return nil, fmt.Errorf("settle: %w", err)
	}
	stages = append(stages, stage{Name: "settle", Goroutines: 1, Ops: 1, Seconds: time.Since(began).Seconds()})
	for _, goroutines := range []int{1, 8} {
		stages = append(stages, timeStage(ctx, "read-series", goroutines, seconds, readOneSeries(store, corpus)))
	}
	stages = append(stages, timeStage(ctx, "read-wide", 1, seconds, readWide(store, corpus)))
	return stages, nil
}

// metricsCorpus is the corpus shifted to end a minute ago, keeping every
// interval, since TinyStore refuses a sample outside its retention and a
// service has the same window: TSBS's own times are from 2016.
type metricsCorpus struct {
	series   []seriesSamples // whole, each in time order
	from, to int64
	samples  int
}

var loadCorpus = sync.OnceValues(func() (*metricsCorpus, error) {
	path := os.Getenv("COMPARE_METRICS")
	if path == "" {
		path = defaultMetricsCorpus
	}
	f, err := os.Open(filepath.Clean(path))
	if err != nil {
		return nil, fmt.Errorf("the metrics corpus: %w; bench/run-tsbs fetches it", err)
	}
	defer f.Close()
	c := &metricsCorpus{from: 1<<63 - 1, to: -1 << 63}
	lines := bufio.NewScanner(f)
	lines.Buffer(make([]byte, 1<<20), 64<<20)
	for lines.Scan() {
		var line struct {
			Metric     map[string]string `json:"metric"`
			Values     []float64         `json:"values"`
			Timestamps []int64           `json:"timestamps"`
		}
		if err = json.Unmarshal(lines.Bytes(), &line); err != nil {
			return nil, err
		}
		c.series = append(c.series, seriesSamples{Labels: line.Metric, Times: line.Timestamps, Values: line.Values})
		c.from, c.to = min(c.from, line.Timestamps[0]), max(c.to, line.Timestamps[len(line.Timestamps)-1])
		c.samples += len(line.Values)
	}
	shift := time.Now().Add(-time.Minute).UnixMilli() - c.to
	for i := range c.series {
		for j := range c.series[i].Times {
			c.series[i].Times[j] += shift
		}
	}
	c.from, c.to = c.from+shift, c.to+shift
	return c, lines.Err()
})

func loadMetricsCorpus() (*metricsCorpus, error) { return loadCorpus() }

// ingestCorpus sends the corpus a window at a time, every series' samples in
// that window together, as scrapes arrive, and times the whole of it
func ingestCorpus(ctx context.Context, store metricsStore, c *metricsCorpus) stage {
	var latencies histogram
	s := stage{Name: "ingest", Goroutines: 1}
	before := spent()
	began := time.Now()
	window := metricsWindow.Milliseconds()
	for start := c.from; start <= c.to; start += window {
		batch := c.window(start, start+window)
		sent := time.Now()
		if err := store.ingest(ctx, batch); err != nil {
			s.Errors++
			if s.FirstError == "" {
				s.FirstError = err.Error()
			}
		}
		latencies.add(time.Since(sent))
	}
	s.Seconds = time.Since(began).Seconds()
	s.Ops = int64(c.samples)
	s.PerSecond = float64(c.samples) / s.Seconds
	s.P50Micros, s.P99Micros = latencies.quantile(0.5), latencies.quantile(0.99)
	s.addUsage(before, spent())
	return s
}

// window is every series' samples in [from, to)
func (c *metricsCorpus) window(from, to int64) []seriesSamples {
	var batch []seriesSamples
	for _, series := range c.series {
		first, _ := slices.BinarySearch(series.Times, from)
		last, _ := slices.BinarySearch(series.Times, to)
		if first < last {
			batch = append(batch, seriesSamples{Labels: series.Labels, Times: series.Times[first:last],
				Values: series.Values[first:last]})
		}
	}
	return batch
}

// readOneSeries reads an hour of one series, a different one each call
func readOneSeries(store metricsStore, c *metricsCorpus) operation {
	hour := time.Hour.Milliseconds()
	return func(ctx context.Context, worker, n int) error {
		series := c.series[pick(worker, n, 21)%len(c.series)]
		from := c.from + int64(pick(worker, n, 22))%max(1, c.to-c.from-hour)
		got, err := store.readSeries(ctx, series.Labels, from, from+hour)
		if err == nil && got == 0 {
			err = fmt.Errorf("an hour from %d of %v came back empty", from, series.Labels)
		}
		return err
	}
}

// readWide reads every series of one region over the whole corpus
func readWide(store metricsStore, c *metricsCorpus) operation {
	want := 0
	for _, series := range c.series {
		if series.Labels[wideName] == wideValue {
			want += len(series.Values)
		}
	}
	return func(ctx context.Context, _, _ int) error {
		got, err := store.readMatching(ctx, wideName, wideValue, c.from, c.to+1)
		if err == nil && got != want {
			err = fmt.Errorf("the wide read returned %d samples, want %d", got, want)
		}
		return err
	}
}
