package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// appStore is one small application's storage: the same requests served by
// TinyStore alone and by the services such an application runs otherwise.
//
// A request checks its user's session, reads a note, logs a line and counts
// itself; one in ten also updates the note and enqueues a job to index it, and
// one in fifty uploads a 16 KiB attachment. A worker in the same process runs
// the jobs as they come. What must survive a crash is committed and synced
// before a request returns: the note, the job, the attachment. The log line
// and the counters are not, as an application's logger and metrics are not.
type appStore interface {
	subject
	seed(ctx context.Context) error
	serve(ctx context.Context, r appRequest) error
}

// backs up is an application's storage whose backup is measured: the
// embedded store and the services; a store served over the socket takes its
// backup on the server's side, which this round does not measure.
type backsUp interface {
	backup(ctx context.Context, path string) error
}

type appRequest struct {
	user, note int
	write      bool
	upload     bool
	n          int
}

const (
	appUsers           = 10_000
	appNotes           = 10_000
	appAttachmentBytes = 16 << 10
)

// The settings where an application's defaults would keep less of its work
// than the services keep, set so that both keep all of it: every log line
// stored, and the queue handled as fast as it fills. A stage's app counts say
// what each kept.
const (
	stackJobBatch   = 256                    // jobs one write settles and claims, on either side
	stackJobWorkers = stackJobBatch / 2      // TinyStore's Work claims two jobs ahead a worker
	stackLogBuffer  = 16 << 10               // log lines TinyStore's handler holds between flushes
	stackLogFlush   = 100 * time.Millisecond // how often it writes them: 160,000 lines a second at most
)

// appCounts is what an application kept of a stage's work, its warm-up
// included: the log lines it stored and dropped, the jobs enqueued and
// handled, and at the stage's end the jobs waiting since it opened. A queue
// that falls behind and a logger that drops show here, where a rate hides them.
type appCounts struct {
	LogsStored   int64 `json:"logs_stored"`
	LogsDropped  int64 `json:"logs_dropped"`
	JobsEnqueued int64 `json:"jobs_enqueued"`
	JobsHandled  int64 `json:"jobs_handled"`
	JobsWaiting  int64 `json:"jobs_waiting"`
}

// counting is an application that counts what it kept since it opened
type counting interface {
	counts(ctx context.Context) appCounts
}

// counted runs a stage of app's and gives it what app kept while it ran
func counted(ctx context.Context, app any, run func() stage) stage {
	c, ok := app.(counting)
	if !ok {
		return run()
	}
	before := c.counts(ctx)
	s := run()
	after := c.counts(ctx)
	s.App = &appCounts{
		LogsStored: after.LogsStored - before.LogsStored, LogsDropped: after.LogsDropped - before.LogsDropped,
		JobsEnqueued: after.JobsEnqueued - before.JobsEnqueued, JobsHandled: after.JobsHandled - before.JobsHandled,
		JobsWaiting: after.JobsEnqueued - after.JobsHandled,
	}
	return s
}

var stackEngine = engine{
	order: []string{"tinystore", "tinystore-sidecar", "tinystore-server", "services"},
	contenders: map[string]opener{
		"tinystore":         openTinyStoreApp,
		"tinystore-sidecar": openSidecarApp,
		"tinystore-server":  openServerApp,
		"services":          openServicesApp,
	},
	measure: measureStack,
}

// measureStack seeds users and notes, serves requests from 8, 64 and 256
// clients, then takes a backup and times it
func measureStack(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	app, ok := s.(appStore)
	if !ok {
		return nil, fmt.Errorf("%T is not an application's storage", s)
	}
	if err := app.seed(ctx); err != nil {
		return nil, fmt.Errorf("seed: %w", err)
	}
	var stages []stage
	for _, clients := range []int{8, 64, 256} {
		stages = append(stages, counted(ctx, app, func() stage {
			return timeStage(ctx, "request", clients, seconds, appRequests(app))
		}))
	}
	if _, ok := app.(backsUp); !ok {
		return stages, nil
	}
	dir, err := os.MkdirTemp("", "compare-backup-")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(dir)
	archive := filepath.Join(dir, "backup.zip")
	backup, err := timeBackup(ctx, app, archive)
	if err != nil {
		return nil, fmt.Errorf("backup: %w", err)
	}
	restore, err := timeRestore(ctx, app, archive)
	if err != nil {
		return nil, fmt.Errorf("restore: %w", err)
	}
	return append(stages, backup, restore), nil
}

func requestFor(worker, n int) appRequest {
	roll := pick(worker, n, 51) % 100
	return appRequest{
		user:   pick(worker, n, 52) % appUsers,
		note:   pick(worker, n, 53)%appNotes + 1,
		write:  roll < 10,
		upload: roll >= 10 && roll < 12,
		n:      n,
	}
}

// timeBackup writes a backup to archive and reports its time, and its size as
// the stage's operation count, in bytes
func timeBackup(ctx context.Context, app appStore, archive string) (stage, error) {
	before := spent()
	began := time.Now()
	if err := app.(backsUp).backup(ctx, strings.TrimSuffix(archive, ".zip")); err != nil {
		return stage{}, err
	}
	elapsed := time.Since(began).Seconds()
	size, err := directoryBytes(filepath.Dir(archive))
	s := stage{Name: "backup-bytes", Goroutines: 1, Ops: size, Seconds: elapsed}
	s.addUsage(before, spent())
	return s, err
}
