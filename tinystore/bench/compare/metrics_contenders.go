package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"time"

	"github.com/prometheus/prometheus/model/labels"
	"github.com/prometheus/prometheus/storage"
	"github.com/prometheus/prometheus/tsdb"
	"github.com/prometheus/prometheus/tsdb/chunkenc"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/metrics"
)

// TinyStore: default ingest bounds and an explicit output budget for the
// TSBS wide read's 763,560 samples. The default query budget is 100,000.

type tinyStoreMetrics struct {
	noService
	store   *tinystore.Store
	metrics *metrics.Store
}

const tinyStoreBatchSamples = 10_000

func openTinyStoreMetrics(ctx context.Context, dir string) (subject, error) {
	store, err := tinystore.Open(ctx, dir, tinystore.Options{})
	if err != nil {
		return nil, err
	}
	m, err := metrics.Open(ctx, store, metrics.Options{
		Limits: metrics.Limits{OutputSamples: 1 << 20},
	})
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	return &tinyStoreMetrics{store: store, metrics: m}, nil
}

func (t *tinyStoreMetrics) ingest(ctx context.Context, window []seriesSamples) error {
	var batches []metrics.Batch
	held := 0
	for _, s := range window {
		batch := metrics.Batch{Series: metrics.Series{Labels: tinyStoreLabels(s.Labels), Kind: metrics.Gauge}}
		for i := range s.Times {
			batch.Samples = append(batch.Samples, metrics.Sample{At: s.Times[i], Value: s.Values[i]})
		}
		if held+len(batch.Samples) > tinyStoreBatchSamples && len(batches) > 0 {
			if err := t.metrics.Ingest(ctx, batches); err != nil {
				return err
			}
			batches, held = nil, 0
		}
		batches, held = append(batches, batch), held+len(batch.Samples)
	}
	if len(batches) == 0 {
		return nil
	}
	return t.metrics.Ingest(ctx, batches)
}

// settle runs maintenance until it seals nothing more, as its background
// work would in time
func (t *tinyStoreMetrics) settle(ctx context.Context) error {
	for range 1000 {
		done, err := t.metrics.Maintain(ctx)
		if err != nil || done.SealedBlocks == 0 {
			return err
		}
	}
	return errors.New("maintenance still sealing after a thousand passes")
}

func (t *tinyStoreMetrics) readSeries(ctx context.Context, lbls map[string]string, from, to int64) (int, error) {
	results, err := t.metrics.Read(ctx, metrics.Range{Matchers: tinyStoreLabels(lbls), From: from, To: to})
	n := 0
	for _, r := range results {
		n += len(r.Samples)
	}
	return n, err
}

func (t *tinyStoreMetrics) readMatching(ctx context.Context, name, value string, from, to int64) (int, error) {
	n := 0
	err := t.metrics.Stream(ctx, metrics.Range{Matchers: []metrics.Label{{Name: name, Value: value}}, From: from, To: to},
		func(r metrics.Result) error {
			n += len(r.Samples)
			return nil
		})
	return n, err
}

func (t *tinyStoreMetrics) close() error { return t.store.Close(context.Background()) }

func tinyStoreLabels(m map[string]string) []metrics.Label {
	out := make([]metrics.Label, 0, len(m))
	for name, value := range m {
		out = append(out, metrics.Label{Name: name, Value: value})
	}
	return out
}

// Prometheus: its storage, tsdb, as a library in this process, with its
// default options; a window is one appender committed. Its WAL is synced as
// a segment fills, not at each commit.

type prometheusMetrics struct {
	noService
	db *tsdb.DB
}

func openPrometheusMetrics(_ context.Context, dir string) (subject, error) {
	db, err := tsdb.Open(dir, nil, nil, tsdb.DefaultOptions(), nil)
	if err != nil {
		return nil, err
	}
	return &prometheusMetrics{db: db}, nil
}

func (p *prometheusMetrics) ingest(ctx context.Context, window []seriesSamples) error {
	app := p.db.Appender(ctx)
	for _, s := range window {
		lbls := labels.FromMap(s.Labels)
		var ref storage.SeriesRef
		for i := range s.Times {
			var err error
			if ref, err = app.Append(ref, lbls, s.Times[i], s.Values[i]); err != nil {
				return errors.Join(err, app.Rollback())
			}
		}
	}
	return app.Commit()
}

func (p *prometheusMetrics) settle(ctx context.Context) error { return p.db.Compact(ctx) }

func (p *prometheusMetrics) readSeries(ctx context.Context, lbls map[string]string, from, to int64) (int, error) {
	var matchers []*labels.Matcher
	for name, value := range lbls {
		matchers = append(matchers, labels.MustNewMatcher(labels.MatchEqual, name, value))
	}
	return p.count(ctx, from, to, matchers)
}

func (p *prometheusMetrics) readMatching(ctx context.Context, name, value string, from, to int64) (int, error) {
	return p.count(ctx, from, to, []*labels.Matcher{labels.MustNewMatcher(labels.MatchEqual, name, value)})
}

// count reads [from, to) as TinyStore does; a querier's range is inclusive
func (p *prometheusMetrics) count(ctx context.Context, from, to int64, matchers []*labels.Matcher) (int, error) {
	q, err := p.db.Querier(from, to-1)
	if err != nil {
		return 0, err
	}
	defer q.Close()
	set := q.Select(ctx, false, nil, matchers...)
	n := 0
	for set.Next() {
		it := set.At().Iterator(nil)
		for it.Next() == chunkenc.ValFloat {
			n++
		}
		if err = it.Err(); err != nil {
			return n, err
		}
	}
	return n, set.Err()
}

func (p *prometheusMetrics) close() error { return p.db.Close() }

// VictoriaMetrics: its single-node server on loopback, with a data directory
// in the run's, fed through its JSON import and read through its export.
// It acknowledges an import before the samples reach the disk, flushing a
// second later, and says so in its documentation.

type victoriaMetrics struct {
	server *exec.Cmd
	base   string
	client *http.Client
}

func openVictoriaMetrics(_ context.Context, dir string) (subject, error) {
	port, err := freePort()
	if err != nil {
		return nil, err
	}
	address := "127.0.0.1:" + strconv.Itoa(port)
	server := exec.Command("victoria-metrics-prod", "-storageDataPath="+filepath.Join(dir, "vm"),
		"-httpListenAddr="+address, "-retentionPeriod=100y", "-loggerLevel=ERROR")
	server.Stdout, server.Stderr = os.Stderr, os.Stderr
	if err = server.Start(); err != nil {
		return nil, fmt.Errorf("victoria-metrics-prod: %w", err)
	}
	v := &victoriaMetrics{server: server, base: "http://" + address, client: &http.Client{}}
	for deadline := time.Now().Add(30 * time.Second); ; time.Sleep(50 * time.Millisecond) {
		if resp, err := v.client.Get(v.base + "/health"); err == nil {
			_ = resp.Body.Close()
			if resp.StatusCode == http.StatusOK {
				return v, nil
			}
		}
		if time.Now().After(deadline) {
			return nil, errors.Join(errors.New("victoria-metrics did not answer within thirty seconds"), v.close())
		}
	}
}

func (v *victoriaMetrics) ingest(ctx context.Context, window []seriesSamples) error {
	var body bytes.Buffer
	encoder := json.NewEncoder(&body)
	for _, s := range window {
		line := struct {
			Metric     map[string]string `json:"metric"`
			Values     []float64         `json:"values"`
			Timestamps []int64           `json:"timestamps"`
		}{s.Labels, s.Values, s.Times}
		if err := encoder.Encode(line); err != nil {
			return err
		}
	}
	return v.post(ctx, "/api/v1/import", &body)
}

// settle flushes what VictoriaMetrics holds in memory and merges its parts,
// as it would given time
func (v *victoriaMetrics) settle(ctx context.Context) error {
	if err := v.post(ctx, "/internal/force_flush", nil); err != nil {
		return err
	}
	return v.post(ctx, "/internal/force_merge", nil)
}

func (v *victoriaMetrics) readSeries(ctx context.Context, lbls map[string]string, from, to int64) (int, error) {
	selector := "{"
	for name, value := range lbls {
		if len(selector) > 1 {
			selector += ","
		}
		selector += name + "=" + strconv.Quote(value)
	}
	return v.export(ctx, selector+"}", from, to)
}

func (v *victoriaMetrics) readMatching(ctx context.Context, name, value string, from, to int64) (int, error) {
	return v.export(ctx, "{"+name+"="+strconv.Quote(value)+"}", from, to)
}

// export counts the samples /api/v1/export streams, a series a line; its
// range is inclusive, and its times are seconds
func (v *victoriaMetrics) export(ctx context.Context, selector string, from, to int64) (int, error) {
	query := url.Values{"match[]": {selector}, "start": {millisToSeconds(from)}, "end": {millisToSeconds(to - 1)}}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, v.base+"/api/v1/export?"+query.Encode(), nil)
	if err != nil {
		return 0, err
	}
	resp, err := v.client.Do(req)
	if err != nil {
		return 0, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		text, _ := io.ReadAll(resp.Body)
		return 0, fmt.Errorf("export: %s: %s", resp.Status, text)
	}
	n := 0
	lines := bufio.NewScanner(resp.Body)
	lines.Buffer(make([]byte, 1<<20), 64<<20)
	for lines.Scan() {
		var line struct {
			Timestamps []int64 `json:"timestamps"`
		}
		if err = json.Unmarshal(lines.Bytes(), &line); err != nil {
			return n, err
		}
		n += len(line.Timestamps)
	}
	return n, lines.Err()
}

func (v *victoriaMetrics) post(ctx context.Context, path string, body io.Reader) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, v.base+path, body)
	if err != nil {
		return err
	}
	resp, err := v.client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	text, _ := io.ReadAll(resp.Body)
	if resp.StatusCode/100 != 2 {
		return fmt.Errorf("%s: %s: %s", path, resp.Status, text)
	}
	return nil
}

func (v *victoriaMetrics) servicePID() int { return v.server.Process.Pid }

// close stops the server as its operator would, SIGINT, which flushes first
func (v *victoriaMetrics) close() error {
	_ = v.server.Process.Signal(os.Interrupt)
	return v.server.Wait()
}

func millisToSeconds(ms int64) string {
	return strconv.FormatFloat(float64(ms)/1000, 'f', 3, 64)
}

func freePort() (int, error) {
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return 0, err
	}
	defer l.Close()
	return l.Addr().(*net.TCPAddr).Port, nil
}
