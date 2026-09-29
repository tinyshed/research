// Command mattn keeps a table through database/sql and mattn/go-sqlite3,
// SQLite's C through cgo, writes a key and reads it back.
package main

import (
	"database/sql"
	"os"

	_ "github.com/mattn/go-sqlite3"
)

func main() {
	db, err := sql.Open("sqlite3", os.Args[1])
	if err != nil {
		panic(err)
	}
	defer db.Close()
	if _, err = db.Exec(`create table if not exists kv (key text primary key, value blob)`); err != nil {
		panic(err)
	}
	var value []byte
	if err = db.QueryRow(`select value from kv where key = ?`, "k").Scan(&value); err != nil && err != sql.ErrNoRows {
		panic(err)
	}
	_, _ = os.Stdout.Write(value)
}
