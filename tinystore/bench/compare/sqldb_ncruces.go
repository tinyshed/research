//go:build !mattn

package main

import (
	"context"
	"path/filepath"

	_ "github.com/ncruces/go-sqlite3/driver"
)

// ncruces: SQLite compiled to WebAssembly and run by wazero, without cgo
func init() {
	sqlEngine.order = append(sqlEngine.order, "ncruces")
	sqlEngine.contenders["ncruces"] = func(ctx context.Context, dir string) (subject, error) {
		return openHandSQL(ctx, "sqlite3", filepath.Join(dir, "app.db"))
	}
	engines["sqldb"] = sqlEngine // the table copied the engine before this ran
}
