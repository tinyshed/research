package main

import (
	"database/sql"
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"testing"

	"github.com/tinyshed/tinystore/internal/dbstat"
)

func TestPageCountsMatchSQLiteDBStat(t *testing.T) {
	for _, size := range []int{512, 4096, 65536} {
		for _, encoding := range []string{"UTF-8", "UTF-16le", "UTF-16be"} {
			for _, vacuum := range []string{"none", "incremental"} {
				t.Run(fmt.Sprintf("%d/%s/%s", size, encoding, vacuum), func(t *testing.T) {
					options := pageFixture{size: size, encoding: encoding, vacuum: vacuum}
					path, want := writePageFixture(t, options)
					objects, err := dbstat.Read(t.Context(), path)
					if err != nil {
						t.Fatal(err)
					}

					for _, object := range objects {
						if expected, found := want[object.Name]; !found || object != expected {
							t.Errorf("%s: got %+v, SQLite counted %+v", object.Name, object, expected)
						}
						delete(want, object.Name)
					}
					if len(want) != 0 {
						t.Errorf("SQLite's objects were missed: %+v", want)
					}
				})
			}
		}
	}
}

type pageFixture struct {
	size     int
	encoding string
	vacuum   string
}

func writePageFixture(t *testing.T, options pageFixture) (string, map[string]dbstat.Object) {
	t.Helper()
	path := filepath.Join(t.TempDir(), "objects.db")
	db, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	db.SetMaxOpenConns(1)
	t.Cleanup(func() { _ = db.Close() })

	setup := fmt.Sprintf(pageFixtureSQL, options.size, options.encoding, options.vacuum)
	if _, err = db.ExecContext(t.Context(), setup); err != nil {
		t.Fatal(err)
	}
	seedPageFixture(t, db)
	want := sqlitePageCounts(t, db)
	if err = db.Close(); err != nil {
		t.Fatal(err)
	}
	return path, want
}

const pageFixtureSQL = `pragma page_size = %d;
pragma encoding = '%s';
pragma auto_vacuum = %s;
pragma synchronous = off;
create table notes (id integer primary key, title text not null);
create index notes_title on notes (title);
create table files (id integer primary key, data blob);
create table words (word text primary key, seen integer) without rowid;
create table empty (id integer primary key);
create virtual table search using fts5(text);
create virtual table places using rtree(id, min_x, max_x);
with recursive numbers(n) as (values(1) union all select n + 1 from numbers where n < 20)
insert into files(data) select zeroblob(8500) from numbers;
insert into search values ('one small runtime'), ('a searchable record');
insert into places values (1, 0, 10), (2, 20, 30);
delete from files where id %% 2 = 0;`

func seedPageFixture(t *testing.T, db *sql.DB) {
	t.Helper()
	tx, err := db.BeginTx(t.Context(), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()

	notes, err := tx.PrepareContext(t.Context(), `insert into notes(title) values (?)`)
	if err != nil {
		t.Fatal(err)
	}
	defer notes.Close()
	words, err := tx.PrepareContext(t.Context(), `insert into words values (?, ?)`)
	if err != nil {
		t.Fatal(err)
	}
	defer words.Close()

	for i := range 1200 {
		title := fmt.Sprintf("note %05d %s", i, strings.Repeat("x", i%300))
		if _, err = notes.ExecContext(t.Context(), title); err != nil {
			t.Fatal(err)
		}
		if i < 400 {
			word := strings.Repeat(fmt.Sprintf("word %04d ", i), 80)
			if _, err = words.ExecContext(t.Context(), word, i); err != nil {
				t.Fatal(err)
			}
		}
	}
	if err = tx.Commit(); err != nil {
		t.Fatal(err)
	}
}

func sqlitePageCounts(t *testing.T, db *sql.DB) map[string]dbstat.Object {
	t.Helper()
	rows, err := db.QueryContext(t.Context(), sqlitePageCountsSQL)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()

	want := map[string]dbstat.Object{}
	for rows.Next() {
		var object dbstat.Object
		if err = rows.Scan(&object.Name, &object.Pages, &object.Bytes); err != nil {
			t.Fatal(err)
		}
		want[object.Name] = object
	}
	if err = errors.Join(rows.Err(), rows.Close()); err != nil {
		t.Fatal(err)
	}
	return want
}

const sqlitePageCountsSQL = `select name, count(*), sum(pgsize) from dbstat group by name`
