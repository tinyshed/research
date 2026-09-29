//go:build mattn

package main

import (
	"context"
	"path/filepath"

	_ "github.com/mattn/go-sqlite3"
)

// mattn: SQLite's own C through cgo, the upper line for what a pure-Go SQLite
// costs; built only with -tags mattn and a C compiler, which run.sh does
func init() {
	sqlEngine.order = append(sqlEngine.order, "mattn")
	sqlEngine.contenders["mattn"] = func(ctx context.Context, dir string) (subject, error) {
		dsn := "file:" + filepath.ToSlash(filepath.Join(dir, "app.db")) +
			"?_journal_mode=WAL&_synchronous=FULL&_busy_timeout=5000"
		return openHandSQLWith(ctx, "sqlite3", dsn+"&_txlock=immediate", dsn+"&_query_only=1")
	}
	engines["sqldb"] = sqlEngine // the table copied the engine before this ran
}
