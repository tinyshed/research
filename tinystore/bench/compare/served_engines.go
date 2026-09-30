package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"

	"github.com/tinyshed/tinystore/server/internal/client"
	"github.com/tinyshed/tinystore/server/wire"
)

// The SQL, jobs and blobs engines through tinystore serve, as a sidecar and as
// a remote server, beside the same engines embedded. They are left out of each
// engine's default order, so that a night's rounds stay as they were, and run
// as a round of their own:
//
//	compare run -engine jobs -contenders tinystore,tinystore-sidecar,tinystore-server

func init() {
	sqlEngine.contenders["tinystore-sidecar"] = func(ctx context.Context, dir string) (subject, error) {
		return openServedSQL(ctx, dir, startSidecar)
	}
	sqlEngine.contenders["tinystore-server"] = func(ctx context.Context, dir string) (subject, error) {
		return openServedSQL(ctx, dir, func(ctx context.Context, dir string) (*exec.Cmd, *client.Conn, error) {
			return startServerMigrated(ctx, dir, func(ctx context.Context, admin *client.Conn) error {
				_, err := admin.Call(ctx, wire.SQLOpen, wire.SQLDatabase{Name: "app", Migrations: appMigrations})
				return err
			})
		})
	}
	jobsEngine.contenders["tinystore-sidecar"] = func(ctx context.Context, dir string) (subject, error) {
		return openServedJobs(ctx, dir, startSidecar)
	}
	jobsEngine.contenders["tinystore-server"] = func(ctx context.Context, dir string) (subject, error) {
		return openServedJobs(ctx, dir, startServer)
	}
	blobsEngine.contenders["tinystore-sidecar"] = func(ctx context.Context, dir string) (subject, error) {
		return openServedBlobs(ctx, dir, startSidecar)
	}
	blobsEngine.contenders["tinystore-server"] = func(ctx context.Context, dir string) (subject, error) {
		return openServedBlobs(ctx, dir, startServer)
	}
}

type starter func(context.Context, string) (*exec.Cmd, *client.Conn, error)

// served is one tinystore serve and the connection to it
type served struct {
	server *exec.Cmd
	conn   *client.Conn
}

func (s *served) open(ctx context.Context, method wire.Method, request client.Message) (uint64, error) {
	body, err := s.conn.Call(ctx, method, request)
	if err != nil {
		return 0, err
	}
	var h wire.Handle
	return h.Handle, h.Decode(body)
}

func (s *served) servicePID() int { return s.server.Process.Pid }

func (s *served) close() error {
	err := s.conn.Close()
	_ = s.server.Process.Signal(os.Interrupt)
	return errors.Join(err, s.server.Wait())
}

// SQL: the notes database, a read a query's download and a write an exec

type servedSQL struct {
	served
	db uint64
}

func openServedSQL(ctx context.Context, dir string, start starter) (subject, error) {
	server, conn, err := start(ctx, dir)
	if err != nil {
		return nil, err
	}
	s := &servedSQL{served: served{server: server, conn: conn}}
	if s.db, err = s.open(ctx, wire.SQLOpen, wire.SQLDatabase{Name: "app", Migrations: appMigrations}); err != nil {
		return nil, errors.Join(err, s.close())
	}
	return s, nil
}

func (s *servedSQL) getNote(ctx context.Context, id int64) (note, error) {
	st, err := s.conn.Open(ctx, wire.SQLQuery, wire.SQLStatement{Handle: s.db, SQL: notesGet, Args: []any{id}}, true)
	if err != nil {
		return note{}, err
	}
	if _, err = st.Response(ctx); err != nil {
		return note{}, err
	}
	var found note
	rows := 0
	for {
		body, last, err := st.Next(ctx)
		if err != nil {
			return note{}, err
		}
		if last {
			return found, errNotFound(fmt.Sprintf("note %d", id), rows == 1)
		}
		var row wire.SQLRow
		if err = row.Decode(body); err != nil {
			return note{}, err
		}
		if found, err = noteOf(row.Values); err != nil {
			return note{}, err
		}
		rows++
	}
}

func noteOf(values []any) (note, error) {
	if len(values) != 3 {
		return note{}, fmt.Errorf("a note of %d columns", len(values))
	}
	id, ok := values[0].(int64)
	title, titled := values[1].(string)
	body, bodied := values[2].(string)
	if !ok || !titled || !bodied {
		return note{}, fmt.Errorf("a note of %T, %T, %T", values[0], values[1], values[2])
	}
	return note{ID: id, Title: title, Body: body}, nil
}

func (s *servedSQL) insertNote(ctx context.Context, n note) error {
	_, err := s.conn.Call(ctx, wire.SQLExec, wire.SQLStatement{Handle: s.db, SQL: notesInsert,
		Args: []any{n.ID, n.Title, n.Body}})
	return err
}

func (s *servedSQL) updateNote(ctx context.Context, id int64, body string) error {
	_, err := s.conn.Call(ctx, wire.SQLExec, wire.SQLStatement{Handle: s.db, SQL: notesUpdate, Args: []any{body, id}})
	return err
}

// Jobs: an enqueue a call, and a drain one work stream that acknowledges each
// job it is handed and ends once none is due

type servedJobs struct {
	served
	queue uint64
}

func openServedJobs(ctx context.Context, dir string, start starter) (subject, error) {
	server, conn, err := start(ctx, dir)
	if err != nil {
		return nil, err
	}
	s := &servedJobs{served: served{server: server, conn: conn}}
	if s.queue, err = s.open(ctx, wire.JobsOpen, wire.JobsQueue{Name: "mail"}); err != nil {
		return nil, errors.Join(err, s.close())
	}
	return s, nil
}

func (s *servedJobs) enqueue(ctx context.Context, n int) error {
	value := fmt.Sprintf(`{"n":%d,"text":"send the weekly digest"}`, n)
	_, err := s.conn.Call(ctx, wire.JobsEnqueue, wire.JobsBatch{Handle: s.queue, Jobs: []wire.JobsJob{{Value: value}}})
	return err
}

func (s *servedJobs) drain(ctx context.Context, workers int) (int64, error) {
	st, err := s.conn.Open(ctx, wire.JobsWork, wire.JobsWorkers{Handle: s.queue, Workers: uint64(workers),
		UntilIdle: true}, false)
	if err != nil {
		return 0, err
	}
	if _, err = st.Response(ctx); err != nil {
		return 0, err
	}
	var done int64
	for {
		body, last, err := st.Next(ctx)
		if err != nil || last {
			return done, err
		}
		var held wire.JobsHeld
		if err = held.Decode(body); err != nil {
			return done, err
		}
		if err = st.Send(ctx, wire.JobsOutcome{Job: held.Job, How: wire.JobAck}.Append(nil), false); err != nil {
			return done, err
		}
		done++
	}
}

// Blobs: a put an upload in bodies as large as the connection agreed, and a
// get a whole download

type servedBlobs struct {
	served
	bucket uint64
}

func openServedBlobs(ctx context.Context, dir string, start starter) (subject, error) {
	server, conn, err := start(ctx, dir)
	if err != nil {
		return nil, err
	}
	s := &servedBlobs{served: served{server: server, conn: conn}}
	if s.bucket, err = s.open(ctx, wire.BlobsOpen, wire.BlobsBucket{Name: "files"}); err != nil {
		return nil, errors.Join(err, s.close())
	}
	return s, nil
}

func (s *servedBlobs) put(ctx context.Context, key string, body []byte) error {
	st, err := s.conn.Open(ctx, wire.BlobsPut, wire.BlobsCall{Handle: s.bucket, Key: key, Size: int64(len(body))}, false)
	if err != nil {
		return err
	}
	piece := int(min(s.conn.Welcome.MaxBody, s.conn.Welcome.StreamCredit, s.conn.Welcome.ConnectionCredit))
	for sent := 0; ; sent += piece {
		end := min(sent+piece, len(body))
		last := end == len(body)
		// the connection keeps a body until it is written, so each is its own
		if err = st.Send(ctx, append([]byte(nil), body[sent:end]...), last); err != nil {
			return err
		}
		if last {
			break
		}
	}
	_, err = st.Response(ctx)
	return err
}

func (s *servedBlobs) get(ctx context.Context, key string) (int, error) {
	st, err := s.conn.Open(ctx, wire.BlobsGet, wire.BlobsCall{Handle: s.bucket, Key: key}, true)
	if err != nil {
		return 0, err
	}
	answer, err := st.Response(ctx)
	if err != nil {
		return 0, err
	}
	var object wire.BlobsObject
	if err = object.Decode(answer); err != nil {
		return 0, err
	}
	if !object.Found {
		return 0, errNotFound(key, false)
	}
	n := 0
	for {
		body, last, err := st.Next(ctx)
		if err != nil {
			return n, err
		}
		n += len(body)
		if last {
			return n, nil
		}
	}
}
