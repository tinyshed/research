package main

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"time"

	"github.com/tinyshed/tinystore/server/internal/client"
	"github.com/tinyshed/tinystore/server/wire"
)

// TinyStore through its sidecar: `tinystore serve --local` in a process of its
// own, found through SERVE and proved as the SDKs find it, and called over one
// multiplexed Unix-socket connection as they call it. It is what a Bun or
// Python application pays, set beside Redis on the same kind of socket.
//
// TINYSTORE_BIN names the binary, which run.sh builds from the submodule
// before the rounds, so that no build is timed as the server's start.

type tinyStoreServeKV struct {
	server *exec.Cmd
	conn   *client.Conn
	bucket uint64
}

func openTinyStoreServeKV(ctx context.Context, dir string) (subject, error) {
	return openServedKV(ctx, dir, startSidecar)
}

// openTinyStoreServerKV is TinyStore as a remote server, `tinystore serve
// --listen tcp://…` with a token, on loopback, with its defaults, 1 GiB of
// memory among them; what a program on another machine pays, less the network.
func openTinyStoreServerKV(ctx context.Context, dir string) (subject, error) {
	return openServedKV(ctx, dir, startServer)
}

func openServedKV(ctx context.Context, dir string,
	start func(context.Context, string) (*exec.Cmd, *client.Conn, error),
) (subject, error) {
	server, conn, err := start(ctx, dir)
	if err != nil {
		return nil, err
	}
	t := &tinyStoreServeKV{server: server, conn: conn}
	body, err := conn.Call(ctx, wire.KVOpen, wire.KVBucket{Name: "sessions"})
	if err != nil {
		return nil, errors.Join(err, t.close())
	}
	var handle wire.Handle
	if err = handle.Decode(body); err != nil {
		return nil, errors.Join(err, t.close())
	}
	t.bucket = handle.Handle
	return t, nil
}

// startSidecar starts tinystore serve on dir and connects once its SERVE says
// where it listens
func startSidecar(ctx context.Context, dir string) (*exec.Cmd, *client.Conn, error) {
	binary := os.Getenv("TINYSTORE_BIN")
	if binary == "" {
		return nil, nil, errors.New("TINYSTORE_BIN names no tinystore binary; run.sh builds one")
	}
	server := exec.Command(binary, "serve", "--dir", dir, "--local", "--idle", "1h")
	server.Stdout, server.Stderr = os.Stderr, os.Stderr
	if err := server.Start(); err != nil {
		return nil, nil, fmt.Errorf("tinystore serve: %w", err)
	}
	for deadline := time.Now().Add(30 * time.Second); ; time.Sleep(20 * time.Millisecond) {
		if _, err := os.Stat(filepath.Join(dir, "server", "SERVE")); err == nil {
			conn, err := client.Found(ctx, dir, wire.Hello{})
			if err == nil {
				return server, conn, nil
			}
		}
		if time.Now().After(deadline) {
			_ = server.Process.Kill()
			return nil, nil, errors.Join(errors.New("tinystore serve did not answer within thirty seconds"),
				server.Wait())
		}
	}
}

// startServer starts tinystore serve listening on a loopback port with a
// data token, and connects with the token once it answers
func startServer(ctx context.Context, dir string) (*exec.Cmd, *client.Conn, error) {
	return startServerMigrated(ctx, dir, nil)
}

// startServerMigrated starts the server with an admin token and a data one:
// migrate runs on an admin connection first, as a deployment applies its
// migrations, and the application connects with the data token
func startServerMigrated(ctx context.Context, dir string,
	migrate func(context.Context, *client.Conn) error,
) (*exec.Cmd, *client.Conn, error) {
	binary := os.Getenv("TINYSTORE_BIN")
	if binary == "" {
		return nil, nil, errors.New("TINYSTORE_BIN names no tinystore binary; run.sh builds one")
	}
	secret := make([]byte, 64)
	if _, err := rand.Read(secret); err != nil {
		return nil, nil, err
	}
	token := base64.RawURLEncoding.EncodeToString(secret[:32])
	admin := base64.RawURLEncoding.EncodeToString(secret[32:])
	tokens := filepath.Join(filepath.Dir(dir), filepath.Base(dir)+".tokens")
	if err := os.WriteFile(tokens, []byte("admin "+admin+"\ndata "+token+"\n"), 0o600); err != nil {
		return nil, nil, err
	}
	port, err := freePort()
	if err != nil {
		return nil, nil, err
	}
	endpoint := "tcp://127.0.0.1:" + strconv.Itoa(port)
	server := exec.Command(binary, "serve", "--dir", dir, "--listen", endpoint, "--tokens", tokens)
	server.Stdout, server.Stderr = os.Stderr, os.Stderr
	if err = server.Start(); err != nil {
		return nil, nil, fmt.Errorf("tinystore serve: %w", err)
	}
	for deadline := time.Now().Add(30 * time.Second); ; time.Sleep(20 * time.Millisecond) {
		if conn, err := client.Dial(ctx, endpoint, wire.Hello{Token: admin}); err == nil {
			if migrate != nil {
				err = migrate(ctx, conn)
			}
			if err = errors.Join(err, conn.Close()); err == nil {
				conn, err = client.Dial(ctx, endpoint, wire.Hello{Token: token})
			}
			if err != nil {
				_ = server.Process.Kill()
				return nil, nil, errors.Join(err, server.Wait())
			}
			return server, conn, nil
		}
		if time.Now().After(deadline) {
			_ = server.Process.Kill()
			return nil, nil, errors.Join(errors.New("tinystore serve did not answer within thirty seconds"),
				server.Wait())
		}
	}
}

func (t *tinyStoreServeKV) set(ctx context.Context, key string, value []byte) error {
	_, err := t.conn.Call(ctx, wire.KVSet, wire.KVCall{Handle: t.bucket, Key: key,
		Value: wire.KVValue{Kind: wire.KVBytes, Bytes: value}})
	return err
}

func (t *tinyStoreServeKV) get(ctx context.Context, key string) ([]byte, error) {
	body, err := t.conn.Call(ctx, wire.KVGet, wire.KVCall{Handle: t.bucket, Key: key})
	if err != nil {
		return nil, err
	}
	var entry wire.KVEntry
	if err = entry.Decode(body); err != nil {
		return nil, err
	}
	if !entry.Found {
		return nil, fmt.Errorf("%s: not found", key)
	}
	return entry.Value.Bytes, nil
}

func (t *tinyStoreServeKV) servicePID() int { return t.server.Process.Pid }

// close lets the server go as the SDKs do: the connection ends, and SIGINT
// closes its store
func (t *tinyStoreServeKV) close() error {
	err := t.conn.Close()
	_ = t.server.Process.Signal(os.Interrupt)
	return errors.Join(err, t.server.Wait())
}
