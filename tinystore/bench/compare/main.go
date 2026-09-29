// Command compare measures TinyStore's engines against the libraries and
// services an application would otherwise use for the same job.
//
//	compare run -engine kv -out results/kv.json        every contender, repeated and interleaved
//	compare weight -out results/weight.json            what linking each contender costs a program
//	compare child -engine kv -contender bbolt ...      one contender in a process of its own
//
// The module's path is under the server's so that Go lets it import the
// server's own client, which the sidecar contenders call through.
//
// Each contender runs in a fresh child process with a fresh directory, so the
// memory a run reports is that contender's alone, and the parent only orders
// the runs and gathers what the children print.
package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
)

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	err := dispatch(ctx, os.Args[1:])
	stop()
	if err != nil {
		fmt.Fprintln(os.Stderr, "compare:", err)
		os.Exit(1)
	}
}

func dispatch(ctx context.Context, args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("usage: compare run|weight|child ...")
	}
	switch args[0] {
	case "run":
		return runAll(ctx, args[1:])
	case "weight":
		return weighAll(ctx, args[1:])
	case "child":
		return runChild(ctx, args[1:])
	}
	return fmt.Errorf("no command %q: run, weight or child", args[0])
}
