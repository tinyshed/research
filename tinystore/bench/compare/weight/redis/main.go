// Command redis is the client alone: it writes a key and reads it back from a
// server that runs apart, whose own binary it does not count.
package main

import (
	"context"
	"os"

	"github.com/redis/go-redis/v9"
)

func main() {
	ctx := context.Background()
	client := redis.NewClient(&redis.Options{Network: "unix", Addr: os.Args[1]})
	defer client.Close()
	if err := client.Set(ctx, "k", "v", 0).Err(); err != nil {
		panic(err)
	}
	value, err := client.Get(ctx, "k").Bytes()
	if err != nil {
		panic(err)
	}
	_, _ = os.Stdout.Write(value)
}
