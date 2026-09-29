package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
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

var stackEngine = engine{
	order: []string{"tinystore", "services"},
	contenders: map[string]opener{
		"tinystore": openTinyStoreApp,
		"services":  openServicesApp,
	},
	measure: measureStack,
}

// measureStack seeds users and notes, serves requests from 8 and 64 clients,
// then takes a backup and times it
func measureStack(ctx context.Context, s subject, seconds float64) ([]stage, error) {
	app, ok := s.(appStore)
	if !ok {
		return nil, fmt.Errorf("%T is not an application's storage", s)
	}
	if err := app.seed(ctx); err != nil {
		return nil, fmt.Errorf("seed: %w", err)
	}
	var stages []stage
	for _, clients := range []int{8, 64} {
		stages = append(stages, timeStage(ctx, "request", clients, seconds, func(ctx context.Context, worker, n int) error {
			return app.serve(ctx, requestFor(worker, n))
		}))
	}
	backup, err := timeBackup(ctx, app)
	if err != nil {
		return nil, fmt.Errorf("backup: %w", err)
	}
	return append(stages, backup), nil
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

// timeBackup writes a backup beside the run's directory and reports its time,
// and its size as the stage's operation count, in bytes
func timeBackup(ctx context.Context, app appStore) (stage, error) {
	dir, err := os.MkdirTemp("", "compare-backup-")
	if err != nil {
		return stage{}, err
	}
	defer os.RemoveAll(dir)
	path := filepath.Join(dir, "backup")
	before := spent()
	began := time.Now()
	if err = app.backup(ctx, path); err != nil {
		return stage{}, err
	}
	elapsed := time.Since(began).Seconds()
	size, err := directoryBytes(dir)
	s := stage{Name: "backup-bytes", Goroutines: 1, Ops: size, Seconds: elapsed}
	s.addUsage(before, spent())
	return s, err
}
