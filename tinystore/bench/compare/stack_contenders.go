package main

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"log/slog"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"sync"
	"sync/atomic"
	"testing/fstest"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/backup"
	"github.com/tinyshed/tinystore/blobs"
	"github.com/tinyshed/tinystore/jobs"
	"github.com/tinyshed/tinystore/kv"
	"github.com/tinyshed/tinystore/metrics"
	"github.com/tinyshed/tinystore/records"
	"github.com/tinyshed/tinystore/sqldb"
)

var attachment = bytes.Repeat(kvValue(7, 0), appAttachmentBytes/kvValueBytes)

func sessionKey(user int) string { return "session:" + strconv.Itoa(user) }

// seedApp writes every user's session and every note from 64 goroutines
func seedApp(ctx context.Context, session func(context.Context, int) error, insert func(context.Context, note) error) error {
	const writers = 64
	errs := make([]error, writers)
	var wg sync.WaitGroup
	for w := range writers {
		wg.Go(func() {
			for i := w; i < max(appUsers, appNotes) && errs[w] == nil; i += writers {
				if i < appUsers {
					errs[w] = session(ctx, i)
				}
				if errs[w] == nil && i < appNotes {
					errs[w] = insert(ctx, makeNote(int64(i+1), 0))
				}
			}
		})
	}
	wg.Wait()
	return errors.Join(errs...)
}

// TinyStore: one store, every engine opened against it, its jobs run by the
// queue's own Work loop and its log lines through records' slog handler.

type tinyStoreApp struct {
	noService
	store    *tinystore.Store
	sessions *kv.Bucket[[]byte]
	db       *sqldb.DB
	queue    *jobs.Queue[job]
	files    *blobs.Bucket
	log      *slog.Logger
	requests metrics.CounterInstrument
	writes   metrics.CounterInstrument
	stop     context.CancelFunc
	worker   chan error
}

func openTinyStoreApp(ctx context.Context, dir string) (subject, error) {
	store, err := tinystore.Open(ctx, dir, tinystore.Options{})
	if err != nil {
		return nil, err
	}
	app := &tinyStoreApp{store: store, worker: make(chan error, 1)}
	if err = app.openEngines(ctx); err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	work, stop := context.WithCancel(context.Background())
	app.stop = stop
	go func() {
		app.worker <- app.queue.Work(work, func(context.Context, jobs.Job[job]) error { return nil })
	}()
	return app, nil
}

func (a *tinyStoreApp) openEngines(ctx context.Context) error {
	state, err := kv.Open(ctx, a.store, kv.Options{})
	if err != nil {
		return err
	}
	if a.sessions, err = kv.OpenBucket[[]byte](ctx, state, "sessions"); err != nil {
		return err
	}
	migrations := fstest.MapFS{"0001_notes.sql": {Data: []byte(notesSchema + ";\n")}}
	if a.db, err = sqldb.Open(ctx, a.store, "app", migrations, nil); err != nil {
		return err
	}
	queues, err := jobs.Open(ctx, a.store, jobs.Options{})
	if err != nil {
		return err
	}
	if a.queue, err = jobs.OpenQueue[job](ctx, queues, "index"); err != nil {
		return err
	}
	objects, err := blobs.Open(ctx, a.store, blobs.Options{})
	if err != nil {
		return err
	}
	if a.files, err = blobs.OpenBucket(ctx, objects, "attachments"); err != nil {
		return err
	}
	logs, err := records.Open(ctx, a.store, records.Options{})
	if err != nil {
		return err
	}
	a.log = slog.New(logs.Handler("app"))
	counters, err := metrics.Open(ctx, a.store, metrics.Options{})
	if err != nil {
		return err
	}
	a.requests, a.writes = counters.Counter("requests"), counters.Counter("writes")
	return nil
}

func (a *tinyStoreApp) seed(ctx context.Context) error {
	return seedApp(ctx,
		func(ctx context.Context, user int) error {
			return a.sessions.Set(ctx, sessionKey(user), kvValue(user, 0))
		},
		func(ctx context.Context, n note) error {
			_, err := a.db.Exec(ctx, notesInsert, n.ID, n.Title, n.Body)
			return err
		})
}

func (a *tinyStoreApp) serve(ctx context.Context, r appRequest) error {
	if _, found, err := a.sessions.Get(ctx, sessionKey(r.user)); err != nil || !found {
		return errors.Join(err, errNotFound(sessionKey(r.user), found))
	}
	if _, found, err := sqldb.One[note](ctx, a.db, notesGet, r.note); err != nil || !found {
		return errors.Join(err, errNotFound("note "+strconv.Itoa(r.note), found))
	}
	if r.write {
		if _, err := a.db.Exec(ctx, notesUpdate, makeNote(int64(r.note), r.n).Body, r.note); err != nil {
			return err
		}
		if err := a.queue.Enqueue(ctx, job{N: r.note, Text: "index the note"}); err != nil {
			return err
		}
		a.writes.Inc()
	}
	if r.upload {
		key := fmt.Sprintf("%d/%d-%d", r.user, r.note, r.n)
		if _, err := a.files.Put(ctx, key, bytes.NewReader(attachment), blobs.Size(int64(len(attachment)))); err != nil {
			return err
		}
	}
	a.log.InfoContext(ctx, "request", "user", r.user, "note", r.note, "write", r.write)
	a.requests.Inc()
	return nil
}

func (a *tinyStoreApp) backup(ctx context.Context, path string) error {
	f, err := os.Create(path + ".zip")
	if err != nil {
		return err
	}
	if err = backup.Write(ctx, a.store, f); err != nil {
		return errors.Join(err, f.Close())
	}
	return errors.Join(f.Sync(), f.Close())
}

func (a *tinyStoreApp) close() error {
	a.stop()
	err := <-a.worker
	if errors.Is(err, context.Canceled) {
		err = nil
	}
	return errors.Join(err, a.store.Close(context.Background()))
}

// The services an application runs otherwise: Postgres for its notes and
// its job queue, Redis for sessions, files on the disk for attachments,
// VictoriaMetrics for its counters, pushed every ten seconds, and a JSON log
// file. A worker in the process claims jobs with FOR UPDATE SKIP LOCKED.

type servicesApp struct {
	dir      string
	redis    *redis.Client
	redisCmd *exec.Cmd
	pg       *postgresSQL
	vm       *victoriaMetrics
	files    *fileBlobs
	logFile  *os.File
	log      *slog.Logger
	requests atomic.Int64
	writes   atomic.Int64
	stop     context.CancelFunc
	running  sync.WaitGroup
}

const (
	jobsSchema = `create table if not exists jobs (id bigserial primary key, body jsonb not null)`
	jobsInsert = `insert into jobs (body) values ($1)`
	jobsClaim  = `delete from jobs where id = (select id from jobs order by id for update skip locked limit 1) returning id`
)

func openServicesApp(ctx context.Context, dir string) (subject, error) {
	app := &servicesApp{dir: dir, files: &fileBlobs{dir: filepath.Join(dir, "files")}}
	if err := app.start(ctx); err != nil {
		return nil, errors.Join(err, app.close())
	}
	background, stop := context.WithCancel(context.Background())
	app.stop = stop
	app.running.Go(func() { app.work(background) })
	app.running.Go(func() { app.pushMetrics(background) })
	return app, nil
}

func (a *servicesApp) start(ctx context.Context) error {
	for _, sub := range []string{"redis", "postgres", "metrics", "files"} {
		if err := os.MkdirAll(filepath.Join(a.dir, sub), 0o755); err != nil {
			return err
		}
	}
	var err error
	if a.redis, a.redisCmd, err = startRedis(ctx, filepath.Join(a.dir, "redis"), false); err != nil {
		return err
	}
	pg, err := openPostgresSQL(ctx, filepath.Join(a.dir, "postgres"))
	if err != nil {
		return err
	}
	a.pg = pg.(*postgresSQL)
	if _, err = a.pg.db.ExecContext(ctx, jobsSchema); err != nil {
		return err
	}
	vm, err := openVictoriaMetrics(ctx, filepath.Join(a.dir, "metrics"))
	if err != nil {
		return err
	}
	a.vm = vm.(*victoriaMetrics)
	if a.logFile, err = os.OpenFile(filepath.Join(a.dir, "app.log"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644); err != nil {
		return err
	}
	a.log = newJSONLogger(a.logFile)
	return nil
}

// work claims a job at a time, and waits a little when none is waiting
func (a *servicesApp) work(ctx context.Context) {
	for ctx.Err() == nil {
		var id int64
		err := a.pg.db.QueryRowContext(ctx, jobsClaim).Scan(&id)
		if err != nil {
			select {
			case <-ctx.Done():
			case <-time.After(20 * time.Millisecond):
			}
		}
	}
}

// pushMetrics sends the counters to VictoriaMetrics every ten seconds, as an
// application's metrics client pushes them
func (a *servicesApp) pushMetrics(ctx context.Context) {
	tick := time.NewTicker(10 * time.Second)
	defer tick.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case now := <-tick.C:
			at := now.UnixMilli()
			_ = a.vm.ingest(ctx, []seriesSamples{
				{Labels: map[string]string{"__name__": "requests"}, Times: []int64{at}, Values: []float64{float64(a.requests.Load())}},
				{Labels: map[string]string{"__name__": "writes"}, Times: []int64{at}, Values: []float64{float64(a.writes.Load())}},
			})
		}
	}
}

func (a *servicesApp) seed(ctx context.Context) error {
	return seedApp(ctx,
		func(ctx context.Context, user int) error {
			return a.redis.Set(ctx, sessionKey(user), kvValue(user, 0), 0).Err()
		},
		func(ctx context.Context, n note) error { return a.pg.insertNote(ctx, n) })
}

func (a *servicesApp) serve(ctx context.Context, r appRequest) error {
	if err := a.redis.Get(ctx, sessionKey(r.user)).Err(); err != nil {
		return fmt.Errorf("%s: %w", sessionKey(r.user), err)
	}
	if _, err := a.pg.getNote(ctx, int64(r.note)); err != nil {
		return err
	}
	if r.write {
		if err := a.pg.updateNote(ctx, int64(r.note), makeNote(int64(r.note), r.n).Body); err != nil {
			return err
		}
		body, err := json.Marshal(job{N: r.note, Text: "index the note"})
		if err != nil {
			return err
		}
		if _, err = a.pg.db.ExecContext(ctx, jobsInsert, body); err != nil {
			return err
		}
		a.writes.Add(1)
	}
	if r.upload {
		if err := a.files.put(ctx, fmt.Sprintf("%d/%d-%d", r.user, r.note, r.n), attachment); err != nil {
			return err
		}
	}
	a.log.InfoContext(ctx, "request", "user", r.user, "note", r.note, "write", r.write)
	a.requests.Add(1)
	return nil
}

// backup takes each service's own backup, as an operator of these services
// does, and puts them in one zip: pg_dump, Redis's SAVE, a VictoriaMetrics
// snapshot, the attachments and the log
func (a *servicesApp) backup(ctx context.Context, path string) error {
	dump := filepath.Join(a.dir, "postgres", "backup.dump")
	pgDump := exec.CommandContext(ctx, postgresBinary("pg_dump"), "-Fc", "-h", filepath.Join(a.dir, "postgres"),
		"-U", "postgres", "-f", dump, "postgres")
	if text, err := pgDump.CombinedOutput(); err != nil {
		return fmt.Errorf("pg_dump: %w\n%s", err, text)
	}
	defer os.Remove(dump)
	if err := a.redis.Save(ctx).Err(); err != nil {
		return fmt.Errorf("redis SAVE: %w", err)
	}
	snapshot, err := a.vm.snapshot(ctx)
	if err != nil {
		return err
	}
	defer a.vm.deleteSnapshot(ctx, snapshot)
	return zipTrees(path+".zip", map[string]string{
		"postgres.dump": dump,
		"dump.rdb":      filepath.Join(a.dir, "redis", "dump.rdb"),
		"metrics":       filepath.Join(a.dir, "metrics", "vm", "snapshots", snapshot),
		"files":         filepath.Join(a.dir, "files"),
		"app.log":       filepath.Join(a.dir, "app.log"),
	})
}

// zipTrees writes each file, or each file under each directory, deflated
func zipTrees(path string, sources map[string]string) error {
	out, err := os.Create(path)
	if err != nil {
		return err
	}
	archive := zip.NewWriter(out)
	for name, source := range sources {
		if err = zipTree(archive, name, source); err != nil {
			return errors.Join(err, out.Close())
		}
	}
	return errors.Join(archive.Close(), out.Sync(), out.Close())
}

// zipTree follows a link to a directory, since a VictoriaMetrics snapshot is
// links to the parts it holds
func zipTree(archive *zip.Writer, name, source string) error {
	return filepath.WalkDir(source, func(file string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() {
			return err
		}
		relative, err := filepath.Rel(source, file)
		if err != nil {
			return err
		}
		inside := filepath.Join(name, relative)
		if entry.Type()&fs.ModeSymlink != 0 {
			if info, err := os.Stat(file); err == nil && info.IsDir() {
				return zipTree(archive, inside, file+string(filepath.Separator))
			}
		}
		return addFile(archive, filepath.ToSlash(inside), file)
	})
}

func addFile(archive *zip.Writer, name, path string) error {
	in, err := os.Open(path)
	if err != nil {
		return err
	}
	defer in.Close()
	w, err := archive.CreateHeader(&zip.FileHeader{Name: name, Method: zip.Deflate})
	if err != nil {
		return err
	}
	_, err = io.Copy(w, in)
	return err
}

func (a *servicesApp) servicePID() int { return 0 }

func (a *servicesApp) servicePIDs() []int {
	var pids []int
	for _, cmd := range []*exec.Cmd{a.redisCmd, a.pgServer(), a.vmServer()} {
		if cmd != nil && cmd.Process != nil {
			pids = append(pids, cmd.Process.Pid)
		}
	}
	return pids
}

func (a *servicesApp) pgServer() *exec.Cmd {
	if a.pg == nil {
		return nil
	}
	return a.pg.server
}

func (a *servicesApp) vmServer() *exec.Cmd {
	if a.vm == nil {
		return nil
	}
	return a.vm.server
}

func (a *servicesApp) close() error {
	if a.stop != nil {
		a.stop()
	}
	a.running.Wait()
	var err error
	if a.logFile != nil {
		err = a.logFile.Close()
	}
	if a.redis != nil {
		err = errors.Join(err, (&redisKV{client: a.redis, server: a.redisCmd}).close())
	}
	if a.pg != nil {
		err = errors.Join(err, a.pg.close())
	}
	if a.vm != nil {
		err = errors.Join(err, a.vm.close())
	}
	return err
}

// snapshot asks VictoriaMetrics for a snapshot of its data, which it makes
// of hard links, and names it
func (v *victoriaMetrics) snapshot(ctx context.Context) (string, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, v.base+"/snapshot/create", nil)
	if err != nil {
		return "", err
	}
	resp, err := v.client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	var answer struct {
		Status   string `json:"status"`
		Snapshot string `json:"snapshot"`
	}
	if err = json.NewDecoder(resp.Body).Decode(&answer); err != nil || answer.Status != "ok" {
		return "", errors.Join(err, fmt.Errorf("snapshot/create: %s", answer.Status))
	}
	return answer.Snapshot, nil
}

func (v *victoriaMetrics) deleteSnapshot(ctx context.Context, name string) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, v.base+"/snapshot/delete?snapshot="+name, nil)
	if err != nil {
		return
	}
	if resp, err := v.client.Do(req); err == nil {
		_ = resp.Body.Close()
	}
}

func newJSONLogger(w io.Writer) *slog.Logger { return slog.New(slog.NewJSONHandler(w, nil)) }
