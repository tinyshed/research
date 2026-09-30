package main

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing/fstest"
	"time"

	_ "github.com/jackc/pgx/v5/stdlib"

	"github.com/tinyshed/tinystore"
	"github.com/tinyshed/tinystore/sqldb"
)

const (
	notesGet    = `select id, title, body from note where id = ?`
	notesInsert = `insert into note (id, title, body) values (?, ?, ?)`
	notesUpdate = `update note set body = ? where id = ?`
)

// TinyStore: sqldb's own database, its migration the one schema.

type tinyStoreSQL struct {
	noService
	store *tinystore.Store
	db    *sqldb.DB
}

func openTinyStoreSQL(ctx context.Context, dir string) (subject, error) {
	store, err := tinystore.Open(ctx, dir, tinystore.Options{})
	if err != nil {
		return nil, err
	}
	migrations := fstest.MapFS{"0001_notes.sql": {Data: []byte(notesSchema + ";\n")}}
	db, err := sqldb.Open(ctx, store, "app", migrations, nil)
	if err != nil {
		return nil, errors.Join(err, store.Close(ctx))
	}
	return &tinyStoreSQL{store: store, db: db}, nil
}

func (t *tinyStoreSQL) getNote(ctx context.Context, id int64) (note, error) {
	found, ok, err := sqldb.One[note](ctx, t.db, notesGet, id)
	if err == nil && !ok {
		err = fmt.Errorf("note %d: not found", id)
	}
	return found, err
}

func (t *tinyStoreSQL) insertNote(ctx context.Context, n note) error {
	_, err := t.db.Exec(ctx, notesInsert, n.ID, n.Title, n.Body)
	return err
}

func (t *tinyStoreSQL) updateNote(ctx context.Context, id int64, body string) error {
	_, err := t.db.Exec(ctx, notesUpdate, body, id)
	return err
}

func (t *tinyStoreSQL) close() error { return t.store.Close(context.Background()) }

// SQLite by hand, through database/sql, the way the kv comparison keeps it: a
// writer pool of one connection and a pool of readers. Two drivers: modernc,
// which TinyStore uses, and ncruces, SQLite compiled to wasm; neither needs cgo.

type handSQL struct {
	noService
	writer, reader *sql.DB
}

func openSQLiteSQL(ctx context.Context, dir string) (subject, error) {
	return openHandSQL(ctx, "sqlite", filepath.Join(dir, "app.db"))
}

func openHandSQL(ctx context.Context, driver, path string) (subject, error) {
	dsn := "file:" + filepath.ToSlash(path) +
		"?_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)&_pragma=busy_timeout(5000)"
	return openHandSQLWith(ctx, driver, dsn+"&_txlock=immediate", dsn+"&_pragma=query_only(1)")
}

// openHandSQLWith opens the writer pool of one and the pool of readers with
// the DSNs a driver spells its pragmas in
func openHandSQLWith(ctx context.Context, driver, writerDSN, readerDSN string) (subject, error) {
	writer, err := sql.Open(driver, writerDSN)
	if err != nil {
		return nil, err
	}
	writer.SetMaxOpenConns(1)
	if _, err = writer.ExecContext(ctx, notesSchema); err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	reader, err := sql.Open(driver, readerDSN)
	if err != nil {
		return nil, errors.Join(err, writer.Close())
	}
	keepReaders(reader)
	return &handSQL{writer: writer, reader: reader}, nil
}

func (h *handSQL) getNote(ctx context.Context, id int64) (note, error) {
	var n note
	err := h.reader.QueryRowContext(ctx, notesGet, id).Scan(&n.ID, &n.Title, &n.Body)
	return n, err
}

func (h *handSQL) insertNote(ctx context.Context, n note) error {
	_, err := h.writer.ExecContext(ctx, notesInsert, n.ID, n.Title, n.Body)
	return err
}

func (h *handSQL) updateNote(ctx context.Context, id int64, body string) error {
	_, err := h.writer.ExecContext(ctx, notesUpdate, body, id)
	return err
}

func (h *handSQL) close() error { return errors.Join(h.reader.Close(), h.writer.Close()) }

// Postgres: a cluster of its own in the run's directory, on a Unix socket,
// with its defaults, which commit synchronously; one pool through pgx.

type postgresSQL struct {
	db     *sql.DB
	server *exec.Cmd
	data   string
}

func openPostgresSQL(ctx context.Context, dir string) (subject, error) {
	data, err := initPostgres(ctx, dir)
	if err != nil {
		return nil, err
	}
	server := asPostgres(ctx, postgresBinary("postgres"), "-D", data, "-k", dir, "-c", "listen_addresses=")
	server.Stdout, server.Stderr = os.Stderr, os.Stderr
	if err = server.Start(); err != nil {
		return nil, fmt.Errorf("postgres: %w", err)
	}
	db, err := sql.Open("pgx", "host="+dir+" user=postgres dbname=postgres sslmode=disable")
	if err != nil {
		return nil, err
	}
	// a pool below Postgres's default max_connections of 100, as an application
	// sizes it: past that a burst waits for a connection instead of being refused
	db.SetMaxOpenConns(postgresConnections)
	db.SetMaxIdleConns(postgresConnections)
	p := &postgresSQL{db: db, server: server, data: data}
	for deadline := time.Now().Add(30 * time.Second); ; time.Sleep(50 * time.Millisecond) {
		if _, err = db.ExecContext(ctx, strings.Replace(strings.TrimSuffix(notesSchema, " strict"),
			"integer primary key", "bigint primary key", 1)); err == nil {
			return p, nil
		}
		if time.Now().After(deadline) {
			return nil, errors.Join(fmt.Errorf("postgres did not answer within thirty seconds: %w", err), p.close())
		}
	}
}

// initPostgres makes a cluster owned by the postgres user, which Postgres
// requires, since it refuses to run as root
func initPostgres(ctx context.Context, dir string) (string, error) {
	data := filepath.Join(dir, "pg")
	if _, err := os.Stat(filepath.Join(data, "PG_VERSION")); err == nil {
		return data, nil // a cluster already there, opened again
	}
	if err := exec.CommandContext(ctx, "chown", "postgres:postgres", dir).Run(); err != nil {
		return "", fmt.Errorf("chown: %w", err)
	}
	initdb := asPostgres(ctx, postgresBinary("initdb"), "-D", data, "--auth=trust", "--no-sync")
	if text, err := initdb.CombinedOutput(); err != nil {
		return "", fmt.Errorf("initdb: %w\n%s", err, text)
	}
	return data, nil
}

func asPostgres(ctx context.Context, binary string, args ...string) *exec.Cmd {
	return exec.CommandContext(ctx, "runuser", append([]string{"-u", "postgres", "--", binary}, args...)...)
}

// postgresBinary finds a server program, which Debian keeps outside PATH
func postgresBinary(name string) string {
	found, _ := filepath.Glob("/usr/lib/postgresql/*/bin/" + name)
	if len(found) == 0 {
		return name
	}
	return found[len(found)-1]
}

const postgresConnections = 64

// keepReaders keeps a hand-made SQLite's readers open, as TinyStore's pool
// does: database/sql keeps two idle connections unless told otherwise, so at
// 64 goroutines every other read would open the file and set its pragmas again
func keepReaders(reader *sql.DB) {
	reader.SetMaxOpenConns(readers)
	reader.SetMaxIdleConns(readers)
}

const readers = 64

func (p *postgresSQL) getNote(ctx context.Context, id int64) (note, error) {
	var n note
	err := p.db.QueryRowContext(ctx, `select id, title, body from note where id = $1`, id).Scan(&n.ID, &n.Title, &n.Body)
	return n, err
}

func (p *postgresSQL) insertNote(ctx context.Context, n note) error {
	_, err := p.db.ExecContext(ctx, `insert into note (id, title, body) values ($1, $2, $3)`, n.ID, n.Title, n.Body)
	return err
}

func (p *postgresSQL) updateNote(ctx context.Context, id int64, body string) error {
	_, err := p.db.ExecContext(ctx, `update note set body = $1 where id = $2`, body, id)
	return err
}

func (p *postgresSQL) servicePID() int { return p.server.Process.Pid }

// close stops the server with pg_ctl's fast mode, since the runuser that
// started it does not pass a signal on, and kills it if that has not ended it
// in thirty seconds
func (p *postgresSQL) close() error {
	err := p.db.Close()
	stop := asPostgres(context.Background(), postgresBinary("pg_ctl"), "-D", p.data, "stop", "-m", "fast", "-w")
	if text, stopErr := stop.CombinedOutput(); stopErr != nil {
		err = errors.Join(err, fmt.Errorf("pg_ctl stop: %w\n%s", stopErr, text))
	}
	done := make(chan error, 1)
	go func() { done <- p.server.Wait() }()
	select {
	case waited := <-done:
		return errors.Join(err, waited)
	case <-time.After(30 * time.Second):
		_ = p.server.Process.Kill()
		return errors.Join(err, errors.New("postgres did not stop within thirty seconds"), <-done)
	}
}
