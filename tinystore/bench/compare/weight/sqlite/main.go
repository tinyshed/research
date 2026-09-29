// Command sqlite keeps a table through database/sql and modernc, writes a key
// and reads it back.
package main

import (
	"database/sql"
	"os"

	_ "modernc.org/sqlite"
)

func main() {
	db, err := sql.Open("sqlite", os.Args[1])
	if err != nil {
		panic(err)
	}
	defer db.Close()
	if _, err = db.Exec(`create table if not exists kv (key text primary key, value blob)`); err != nil {
		panic(err)
	}
	if _, err = db.Exec(`insert into kv values (?, ?)`, "k", []byte("v")); err != nil {
		panic(err)
	}
	var value []byte
	if err = db.QueryRow(`select value from kv where key = ?`, "k").Scan(&value); err != nil {
		panic(err)
	}
	_, _ = os.Stdout.Write(value)
}
