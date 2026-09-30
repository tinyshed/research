package main

import (
	"archive/zip"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/tinyshed/tinystore/backup"
)

// restorable is an application's storage that can be brought back from the
// backup it took, into an empty directory, and opened there.
type restorable interface {
	restore(ctx context.Context, archive, dir string) (appStore, error)
}

const restoreChecks = 1000

// timeRestore restores the backup at archive into a directory of its own and
// times it until the restored application has answered restoreChecks reads
func timeRestore(ctx context.Context, app appStore, archive string) (stage, error) {
	r, ok := app.(restorable)
	if !ok {
		return stage{}, fmt.Errorf("%T cannot restore", app)
	}
	dir, err := os.MkdirTemp("", "compare-restore-")
	if err != nil {
		return stage{}, err
	}
	defer os.RemoveAll(dir)
	if err = os.Chmod(dir, 0o755); err != nil {
		return stage{}, err
	}
	before := spent()
	began := time.Now()
	restored, err := r.restore(ctx, archive, dir)
	if err != nil {
		return stage{}, err
	}
	s := stage{Name: "restore", Goroutines: 1, Ops: restoreChecks}
	for n := range restoreChecks {
		request := requestFor(0, n)
		request.write, request.upload = false, false
		if err = restored.serve(ctx, request); err != nil {
			s.Errors++
			if s.FirstError == "" {
				s.FirstError = err.Error()
			}
		}
	}
	s.Seconds = time.Since(began).Seconds()
	s.addUsage(before, spent())
	return s, restored.close()
}

// TinyStore: backup.Restore into the empty directory, then Open.
func (a *tinyStoreApp) restore(ctx context.Context, archive, dir string) (appStore, error) {
	f, err := os.Open(archive)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil {
		return nil, err
	}
	if err = backup.Restore(ctx, dir, f, info.Size()); err != nil {
		return nil, err
	}
	return openTinyStoreAppWith(ctx, dir, a.schema, a.jobsIn)
}

// The services, as their operator brings them back: the attachments and the
// log unpacked, the VictoriaMetrics snapshot copied where its data lives, a
// new Postgres cluster given pg_restore, and Redis started on its RDB file,
// its append-only file turned on once it has loaded, as Redis documents.
func (a *servicesApp) restore(ctx context.Context, archive, dir string) (appStore, error) {
	if err := unzipInto(archive, dir, map[string]string{
		"files/":        "files/",
		"app.log":       "app.log",
		"metrics/":      "metrics/vm/",
		"dump.rdb":      "redis/dump.rdb",
		"postgres.dump": "postgres.dump",
	}); err != nil {
		return nil, err
	}
	b := &servicesApp{dir: dir, files: &fileBlobs{dir: filepath.Join(dir, "files")}}
	if err := b.startRestored(ctx); err != nil {
		return nil, errors.Join(err, b.close())
	}
	background, stop := context.WithCancel(context.Background())
	b.stop = stop
	b.running.Go(func() { b.work(background) })
	b.running.Go(func() { b.pushMetrics(background) })
	return b, nil
}

func (a *servicesApp) startRestored(ctx context.Context) error {
	if err := os.MkdirAll(filepath.Join(a.dir, "postgres"), 0o755); err != nil {
		return err
	}
	var err error
	if a.redis, a.redisCmd, err = startRedisWith(ctx, filepath.Join(a.dir, "redis"), false, false); err != nil {
		return err
	}
	if err = a.redis.ConfigSet(ctx, "appendonly", "yes").Err(); err != nil {
		return fmt.Errorf("redis appendonly after the load: %w", err)
	}
	pg, err := openPostgresSQL(ctx, filepath.Join(a.dir, "postgres"))
	if err != nil {
		return err
	}
	a.pg = pg.(*postgresSQL)
	restore := exec.CommandContext(ctx, postgresBinary("pg_restore"), "--clean", "--if-exists", "-h",
		filepath.Join(a.dir, "postgres"), "-U", "postgres", "-d", "postgres", filepath.Join(a.dir, "postgres.dump"))
	if text, err := restore.CombinedOutput(); err != nil {
		return fmt.Errorf("pg_restore: %w\n%s", err, text)
	}
	vm, err := openVictoriaMetrics(ctx, filepath.Join(a.dir, "metrics"))
	if err != nil {
		return err
	}
	a.vm = vm.(*victoriaMetrics)
	a.logFile, err = os.OpenFile(filepath.Join(a.dir, "app.log"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return err
	}
	a.log = newJSONLogger(a.logFile)
	return nil
}

// unzipInto writes each archive entry under dir, its name's first matching
// prefix replaced by the destination's
func unzipInto(archive, dir string, prefixes map[string]string) error {
	r, err := zip.OpenReader(archive)
	if err != nil {
		return err
	}
	defer r.Close()
	for _, f := range r.File {
		target := ""
		for from, to := range prefixes {
			if rest, ok := strings.CutPrefix(f.Name, from); ok {
				target = to + rest
				break
			}
		}
		if target == "" {
			continue
		}
		if err = unzipFile(f, filepath.Join(dir, filepath.FromSlash(target))); err != nil {
			return err
		}
	}
	return nil
}

func unzipFile(f *zip.File, path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	in, err := f.Open()
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.Create(path)
	if err != nil {
		return err
	}
	if _, err = io.Copy(out, in); err != nil {
		return errors.Join(err, out.Close())
	}
	return errors.Join(out.Sync(), out.Close())
}
