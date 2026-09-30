package main

import (
	"bytes"
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"time"
)

// A round is every contender of one engine, repeated, in an order that
// alternates so that no contender always runs first on a cold machine:
//
//	repeat 1  a b c
//	repeat 2  c b a
//	repeat 3  a b c
type round struct {
	Engine      string      `json:"engine"`
	Started     time.Time   `json:"started"`
	Environment environment `json:"environment"`
	Seconds     float64     `json:"seconds_a_stage"`
	Runs        []childRun  `json:"runs"`
	// Failed names each contender's repeat that failed and why; the round goes
	// on without it, and a later run of that contender alone fills the gap
	Failed []string `json:"failed,omitempty"`
}

func runAll(ctx context.Context, args []string) error {
	flags := flag.NewFlagSet("run", flag.ContinueOnError)
	engineName := flags.String("engine", "", "the engine to measure: "+strings.Join(engineNames(), ", "))
	only := flags.String("contenders", "", "a comma-separated subset of the engine's contenders")
	repeats := flags.Int("repeats", 3, "how many times each contender runs")
	seconds := flags.Float64("seconds", 5, "how long each timed stage runs")
	base := flags.String("dir", os.TempDir(), "where each run's directory is made, on the disk measured")
	out := flags.String("out", "", "the JSON file the round is written to")
	if err := flags.Parse(args); err != nil {
		return err
	}
	e, ok := engines[*engineName]
	if !ok {
		return fmt.Errorf("-engine %q: one of %s", *engineName, strings.Join(engineNames(), ", "))
	}
	contenders, err := chosen(e, *only)
	if err != nil {
		return err
	}

	r := round{Engine: *engineName, Started: time.Now().UTC(), Environment: describeEnvironment(), Seconds: *seconds}
	for repeat := range *repeats {
		order := slices.Clone(contenders)
		if repeat%2 == 1 {
			slices.Reverse(order)
		}
		for _, name := range order {
			run, err := runOnce(ctx, *engineName, name, repeat+1, *seconds, *base)
			if err != nil {
				failed := fmt.Sprintf("%s, repeat %d: %v", name, repeat+1, err)
				fmt.Fprintln(os.Stderr, "compare: "+failed)
				r.Failed = append(r.Failed, failed)
				continue
			}
			fmt.Fprintln(os.Stderr, run.summary())
			r.Runs = append(r.Runs, run)
		}
	}
	if err = writeJSON(*out, r); err != nil {
		return err
	}
	if len(r.Failed) > 0 {
		return fmt.Errorf("%d of the round's runs failed", len(r.Failed))
	}
	return nil
}

// runOnce runs one contender in a child process with a directory of its own,
// removed afterwards
func runOnce(ctx context.Context, engineName, contender string, repeat int, seconds float64, base string) (
	childRun, error,
) {
	dir, err := os.MkdirTemp(base, "compare-"+contender+"-")
	if err != nil {
		return childRun{}, err
	}
	defer os.RemoveAll(dir)
	// a server that runs as its own user, as Postgres does, must reach its
	// directory under this one, which MkdirTemp makes for its owner alone
	if err = os.Chmod(dir, 0o755); err != nil {
		return childRun{}, err
	}
	self, err := os.Executable()
	if err != nil {
		return childRun{}, err
	}
	child := exec.CommandContext(ctx, self, "child", "-engine", engineName, "-contender", contender,
		"-dir", dir, "-seconds", strconv.FormatFloat(seconds, 'f', -1, 64))
	var stdout, stderr bytes.Buffer
	child.Stdout, child.Stderr = &stdout, &stderr
	if err = child.Run(); err != nil {
		return childRun{}, fmt.Errorf("%w\n%s", err, stderr.String())
	}
	var run childRun
	if err = json.Unmarshal(stdout.Bytes(), &run); err != nil {
		return childRun{}, fmt.Errorf("its output: %w\n%s", err, stdout.String())
	}
	run.Repeat = repeat
	return run, nil
}

func chosen(e engine, only string) ([]string, error) {
	if only == "" {
		return e.order, nil
	}
	var names []string
	for name := range strings.SplitSeq(only, ",") {
		if _, ok := e.contenders[name]; !ok {
			return nil, fmt.Errorf("no contender %q: %s", name, strings.Join(e.order, ", "))
		}
		names = append(names, name)
	}
	return names, nil
}

func writeJSON(path string, value any) error {
	body, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	if path == "" {
		_, err = os.Stdout.Write(append(body, '\n'))
		return err
	}
	if err = os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, append(body, '\n'), 0o644)
}
