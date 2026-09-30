package main

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"sync"
	"sync/atomic"
	"time"

	"github.com/tinyshed/tinystore/server/internal/client"
	"github.com/tinyshed/tinystore/server/wire"
)

// The same application through tinystore serve, as a Bun or Python program
// uses its store: one connection, each engine's calls on it. Its log lines go
// up one records.lines stream, as another program's output does, and its
// counters are pushed every ten seconds, as the services' are to Victoria.

type servedApp struct {
	server   *exec.Cmd
	conn     *client.Conn
	sessions uint64
	db       uint64
	queue    uint64
	files    uint64
	lines    *client.Stream
	queued   chan string // log lines waiting for the stream; full, a line is dropped
	// slots bound the requests in flight below the streams a connection may
	// have open, so that a burst waits for a stream as an SDK's calls do,
	// rather than being refused
	slots    chan struct{}
	dropped  atomic.Int64
	requests atomic.Int64
	writes   atomic.Int64
	stop     context.CancelFunc
	running  sync.WaitGroup
}

func openSidecarApp(ctx context.Context, dir string) (subject, error) {
	return openServedApp(ctx, dir, startSidecar)
}

func openServerApp(ctx context.Context, dir string) (subject, error) {
	return openServedApp(ctx, dir, func(ctx context.Context, dir string) (*exec.Cmd, *client.Conn, error) {
		return startServerMigrated(ctx, dir, func(ctx context.Context, admin *client.Conn) error {
			_, err := admin.Call(ctx, wire.SQLOpen, wire.SQLDatabase{Name: "app", Migrations: appMigrations})
			return err
		})
	})
}

func openServedApp(ctx context.Context, dir string,
	start func(context.Context, string) (*exec.Cmd, *client.Conn, error),
) (subject, error) {
	server, conn, err := start(ctx, dir)
	if err != nil {
		return nil, err
	}
	a := &servedApp{server: server, conn: conn, slots: make(chan struct{}, conn.Welcome.InFlight-8)}
	if err = a.openHandles(ctx); err != nil {
		return nil, errors.Join(err, a.close())
	}
	background, stop := context.WithCancel(context.Background())
	a.stop = stop
	a.running.Go(func() { a.work(background) })
	a.running.Go(func() { a.pushMetrics(background) })
	a.running.Go(func() { a.sendLines(background) })
	return a, nil
}

func (a *servedApp) openHandles(ctx context.Context) error {
	var err error
	if a.sessions, err = a.handle(ctx, wire.KVOpen, wire.KVBucket{Name: "sessions"}); err != nil {
		return err
	}
	migrations := []wire.SQLMigration{{Name: "0001_notes.sql", Text: notesSchema + ";\n"}}
	if a.db, err = a.handle(ctx, wire.SQLOpen, wire.SQLDatabase{Name: "app", Migrations: migrations}); err != nil {
		return err
	}
	if a.queue, err = a.handle(ctx, wire.JobsOpen, wire.JobsQueue{Name: "index"}); err != nil {
		return err
	}
	if a.files, err = a.handle(ctx, wire.BlobsOpen, wire.BlobsBucket{Name: "attachments"}); err != nil {
		return err
	}
	a.queued = make(chan string, 4096)
	a.lines, err = a.conn.Open(ctx, wire.RecordsLines, wire.RecordsStream{Stream: "app"}, false)
	return err
}

var appMigrations = []wire.SQLMigration{{Name: "0001_notes.sql", Text: notesSchema + ";\n"}}

func (a *servedApp) handle(ctx context.Context, method wire.Method, open client.Message) (uint64, error) {
	body, err := a.conn.Call(ctx, method, open)
	if err != nil {
		return 0, err
	}
	var h wire.Handle
	return h.Handle, h.Decode(body)
}

func (a *servedApp) seed(ctx context.Context) error {
	return seedApp(ctx,
		func(ctx context.Context, user int) error {
			_, err := a.conn.Call(ctx, wire.KVSet, wire.KVCall{Handle: a.sessions, Key: sessionKey(user),
				Value: wire.KVValue{Kind: wire.KVBytes, Bytes: kvValue(user, 0)}})
			return err
		},
		func(ctx context.Context, n note) error {
			return a.exec(ctx, notesInsert, n.ID, n.Title, n.Body)
		})
}

func (a *servedApp) serve(ctx context.Context, r appRequest) error {
	select {
	case a.slots <- struct{}{}:
	case <-ctx.Done():
		return ctx.Err()
	}
	defer func() { <-a.slots }()
	if err := a.session(ctx, r.user); err != nil {
		return err
	}
	if err := a.readNote(ctx, r.note); err != nil {
		return err
	}
	if r.write {
		if err := a.exec(ctx, notesUpdate, makeNote(int64(r.note), r.n).Body, r.note); err != nil {
			return err
		}
		value := fmt.Sprintf(`{"n":%d,"text":"index the note"}`, r.note)
		if _, err := a.conn.Call(ctx, wire.JobsEnqueue, wire.JobsBatch{Handle: a.queue,
			Jobs: []wire.JobsJob{{Value: value}}}); err != nil {
			return err
		}
		a.writes.Add(1)
	}
	if r.upload {
		if err := a.upload(ctx, fmt.Sprintf("%d/%d-%d", r.user, r.note, r.n)); err != nil {
			return err
		}
	}
	line := fmt.Sprintf(`{"level":"info","msg":"request","user":%d,"note":%d,"write":%t}`+"\n", r.user, r.note, r.write)
	select {
	case a.queued <- line:
	default:
		a.dropped.Add(1) // as TinyStore's own handler drops and counts, never waiting
	}
	a.requests.Add(1)
	return nil
}

// sendLines sends what the requests logged, what has gathered in one DATA,
// as a program's buffered logger writes its output
func (a *servedApp) sendLines(ctx context.Context) {
	var chunk []byte
	for {
		select {
		case <-ctx.Done():
			return
		case line := <-a.queued:
			chunk = append(chunk[:0], line...)
		}
		for more := true; more && len(chunk) < 64<<10; {
			select {
			case line := <-a.queued:
				chunk = append(chunk, line...)
			default:
				more = false
			}
		}
		if a.lines.Send(ctx, chunk, false) != nil {
			return
		}
	}
}

func (a *servedApp) session(ctx context.Context, user int) error {
	body, err := a.conn.Call(ctx, wire.KVGet, wire.KVCall{Handle: a.sessions, Key: sessionKey(user)})
	if err != nil {
		return err
	}
	var entry wire.KVEntry
	if err = entry.Decode(body); err != nil {
		return err
	}
	return errNotFound(sessionKey(user), entry.Found)
}

// readNote downloads a query's columns and its one row
func (a *servedApp) readNote(ctx context.Context, id int) error {
	st, err := a.conn.Open(ctx, wire.SQLQuery, wire.SQLStatement{Handle: a.db, SQL: notesGet, Args: []any{int64(id)}}, true)
	if err != nil {
		return err
	}
	if _, err = st.Response(ctx); err != nil {
		return err
	}
	rows := 0
	for {
		_, last, err := st.Next(ctx)
		if err != nil {
			return err
		}
		if last {
			return errNotFound("note "+strconv.Itoa(id), rows == 1)
		}
		rows++
	}
}

func (a *servedApp) exec(ctx context.Context, statement string, args ...any) error {
	_, err := a.conn.Call(ctx, wire.SQLExec, wire.SQLStatement{Handle: a.db, SQL: statement, Args: wireArgs(args)})
	return err
}

// wireArgs spells Go integers as the int64 the profile writes
func wireArgs(args []any) []any {
	out := make([]any, len(args))
	for i, arg := range args {
		if n, ok := arg.(int); ok {
			arg = int64(n)
		}
		out[i] = arg
	}
	return out
}

func (a *servedApp) upload(ctx context.Context, key string) error {
	st, err := a.conn.Open(ctx, wire.BlobsPut, wire.BlobsCall{Handle: a.files, Key: key, Size: int64(len(attachment))}, false)
	if err != nil {
		return err
	}
	if err = st.Send(ctx, bytes.Clone(attachment), true); err != nil {
		return err
	}
	_, err = st.Response(ctx)
	return err
}

// work runs the queue's jobs through a work stream, acknowledging each
func (a *servedApp) work(ctx context.Context) {
	st, err := a.conn.Open(ctx, wire.JobsWork, wire.JobsWorkers{Handle: a.queue, Workers: 1}, false)
	if err != nil {
		return
	}
	if _, err = st.Response(ctx); err != nil {
		return
	}
	for {
		body, last, err := st.Next(ctx)
		if err != nil || last {
			return
		}
		var held wire.JobsHeld
		if held.Decode(body) != nil {
			return
		}
		if st.Send(ctx, wire.JobsOutcome{Job: held.Job, How: wire.JobAck}.Append(nil), false) != nil {
			return
		}
	}
}

func (a *servedApp) pushMetrics(ctx context.Context) {
	tick := time.NewTicker(10 * time.Second)
	defer tick.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case now := <-tick.C:
			at := now.UnixMilli()
			_, _ = a.conn.Call(ctx, wire.MetricsIngest, wire.MetricsBatch{Series: []wire.MetricsSeries{
				{Labels: map[string]string{"__name__": "requests"}, Kind: "counter", Times: []int64{at},
					Values: []float64{float64(a.requests.Load())}},
				{Labels: map[string]string{"__name__": "writes"}, Kind: "counter", Times: []int64{at},
					Values: []float64{float64(a.writes.Load())}},
			}})
		}
	}
}

func (a *servedApp) servicePID() int { return a.server.Process.Pid }

func (a *servedApp) close() error {
	if a.stop != nil {
		a.stop()
	}
	a.running.Wait()
	var err error
	if a.lines != nil {
		err = a.lines.Send(context.Background(), nil, true)
	}
	err = errors.Join(err, a.conn.Close())
	_ = a.server.Process.Signal(os.Interrupt)
	return errors.Join(err, a.server.Wait())
}
