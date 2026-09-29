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
