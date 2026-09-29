package main

import (
	"bufio"
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"math/rand/v2"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"time"
)

// The crash round asks whether what a contender acknowledged survives its
// process being killed, and how long it takes to open again. A writer prints
// each write it was told is durable; the parent kills its whole process group
// with SIGKILL at a random moment, servers included, then a verifier opens the
// same directory and reads back every acknowledged key.
//
// It proves nothing about a power loss, since the page cache outlives a
// killed process; it catches acknowledgements given before the write reached
// the kernel, and recovery that loses or breaks what was written.

const (
	crashWriters = 8
	crashKeys    = 8_000 // each writer owns every eighth
)

// crashCycle is one kill and the check after it.
type crashCycle struct {
	Acknowledged int     `json:"acknowledged"`
	Lost         int     `json:"lost"`
	Wrong        int     `json:"wrong"`
	OpenSeconds  float64 `json:"open_seconds"`
	FirstProblem string  `json:"first_problem,omitempty"`
}

type crashRun struct {
	Contender string       `json:"contender"`
	Cycles    []crashCycle `json:"cycles"`
}

func runCrash(ctx context.Context, args []string) error {
	flags := flag.NewFlagSet("crash", flag.ContinueOnError)
	only := flags.String("contenders", strings.Join(crashContenders, ","), "")
	cycles := flags.Int("cycles", 20, "kills a contender")
	base := flags.String("dir", os.TempDir(), "")
	out := flags.String("out", "", "")
	if err := flags.Parse(args); err != nil {
		return err
	}
	var runs []crashRun
	for name := range strings.SplitSeq(*only, ",") {
		run, err := crashContender(ctx, name, *cycles, *base)
		if err != nil {
			return fmt.Errorf("%s: %w", name, err)
		}
		fmt.Fprintf(os.Stderr, "%-18s %s\n", name, run.summary())
		runs = append(runs, run)
	}
	return writeJSON(*out, runs)
}

var crashContenders = []string{"tinystore", "tinystore-sidecar", "sqlite", "bbolt", "badger", "pebble", "redis"}

func (r crashRun) summary() string {
	acked, lost, wrong, open := 0, 0, 0, 0.0
	for _, c := range r.Cycles {
		acked, lost, wrong, open = acked+c.Acknowledged, lost+c.Lost, wrong+c.Wrong, max(open, c.OpenSeconds)
	}
	return fmt.Sprintf("%d cycles, %d acknowledged, %d lost, %d wrong, slowest open %.2f s",
		len(r.Cycles), acked, lost, wrong, open)
}

func crashContender(ctx context.Context, name string, cycles int, base string) (crashRun, error) {
	dir, err := os.MkdirTemp(base, "crash-"+name+"-")
	if err != nil {
		return crashRun{}, err
	}
	defer os.RemoveAll(dir)
	if err = os.Chmod(dir, 0o755); err != nil {
		return crashRun{}, err
	}
	run := crashRun{Contender: name}
	for range cycles {
		acked, err := writeUntilKilled(ctx, name, dir)
		if err != nil {
			return run, err
		}
		cycle, err := verifyAfterCrash(ctx, name, dir, acked)
		if err != nil {
			return run, err
		}
		run.Cycles = append(run.Cycles, cycle)
	}
	return run, nil
}

// writeUntilKilled starts a writer, reads what it acknowledges, and kills it
// and everything it started one to three seconds after its first
func writeUntilKilled(ctx context.Context, name, dir string) (map[int]int, error) {
	self, err := os.Executable()
	if err != nil {
		return nil, err
	}
	writer := exec.CommandContext(ctx, self, "crash-write", "-contender", name, "-dir", dir)
	ownGroup(writer)
	writer.Stderr = os.Stderr
	stdout, err := writer.StdoutPipe()
	if err != nil {
		return nil, err
	}
	if err = writer.Start(); err != nil {
		return nil, err
	}
	acked := map[int]int{}
	lines := bufio.NewScanner(stdout)
	var killAt time.Time
	for lines.Scan() {
		key, version, ok := strings.Cut(lines.Text(), " ")
		if !ok {
			continue
		}
		i, _ := strconv.Atoi(key)
		v, _ := strconv.Atoi(version)
		acked[i] = max(acked[i], v)
		if killAt.IsZero() {
			killAt = time.Now().Add(time.Second + time.Duration(rand.Int64N(int64(2*time.Second))))
			time.AfterFunc(time.Until(killAt), func() { killGroup(writer) })
		}
	}
	_ = writer.Wait() // killed: its error says so
	if len(acked) == 0 {
		return nil, fmt.Errorf("the writer acknowledged nothing before it ended")
	}
	return acked, nil
}

// verifyAfterCrash hands a verifier the acknowledged versions and reads its
// verdict
func verifyAfterCrash(ctx context.Context, name, dir string, acked map[int]int) (crashCycle, error) {
	file, err := os.CreateTemp("", "acked-*.json")
	if err != nil {
		return crashCycle{}, err
	}
	defer os.Remove(file.Name())
	if err = json.NewEncoder(file).Encode(acked); err != nil {
		return crashCycle{}, err
	}
	if err = file.Close(); err != nil {
		return crashCycle{}, err
	}
	self, err := os.Executable()
	if err != nil {
		return crashCycle{}, err
	}
	verifier := exec.CommandContext(ctx, self, "crash-verify", "-contender", name, "-dir", dir, "-acked", file.Name())
	verifier.Stderr = os.Stderr
	out, err := verifier.Output()
	if err != nil {
		return crashCycle{}, fmt.Errorf("verify: %w", err)
	}
	var cycle crashCycle
	return cycle, json.Unmarshal(out, &cycle)
}

// crashWrite is the writer's process: each writer walks its keys, writing
// each a higher version, and prints each write once it returns
func crashWrite(ctx context.Context, args []string) error {
	name, dir, _, err := crashFlags(args)
	if err != nil {
		return err
	}
	s, err := kvEngine.contenders[name](ctx, dir)
	if err != nil {
		return err
	}
	store := s.(kvStore)
	out := bufio.NewWriter(os.Stdout)
	var mu sync.Mutex
	var wg sync.WaitGroup
	for w := range crashWriters {
		wg.Go(func() {
			for v := 1; ; v++ {
				for i := w; i < crashKeys; i += crashWriters {
					if store.set(ctx, kvKey(i), kvValue(i, v)) != nil {
						return
					}
					mu.Lock()
					fmt.Fprintf(out, "%d %d\n", i, v)
					_ = out.Flush()
					mu.Unlock()
				}
			}
		})
	}
	wg.Wait() // until killed
	return nil
}

// crashVerify opens the directory again and checks every acknowledged key
// holds its version or the next, which may have been written unacknowledged
func crashVerify(ctx context.Context, args []string) error {
	name, dir, ackedPath, err := crashFlags(args)
	if err != nil {
		return err
	}
	text, err := os.ReadFile(ackedPath)
	if err != nil {
		return err
	}
	var acked map[int]int
	if err = json.Unmarshal(text, &acked); err != nil {
		return err
	}
	began := time.Now()
	s, err := kvEngine.contenders[name](ctx, dir)
	if err != nil {
		return fmt.Errorf("open after the crash: %w", err)
	}
	cycle := crashCycle{OpenSeconds: time.Since(began).Seconds(), Acknowledged: len(acked)}
	store := s.(kvStore)
	for i, v := range acked {
		got, err := store.get(ctx, kvKey(i))
		switch {
		case err != nil:
			cycle.Lost++
			cycle.note(fmt.Sprintf("%s version %d: %v", kvKey(i), v, err))
		case string(got) != string(kvValue(i, v)) && string(got) != string(kvValue(i, v+1)):
			cycle.Wrong++
			cycle.note(fmt.Sprintf("%s: not version %d nor %d", kvKey(i), v, v+1))
		}
	}
	if err = s.close(); err != nil {
		return err
	}
	return json.NewEncoder(os.Stdout).Encode(cycle)
}

func (c *crashCycle) note(problem string) {
	if c.FirstProblem == "" {
		c.FirstProblem = problem
	}
}

func crashFlags(args []string) (name, dir, acked string, err error) {
	flags := flag.NewFlagSet("crash", flag.ContinueOnError)
	flags.StringVar(&name, "contender", "", "")
	flags.StringVar(&dir, "dir", "", "")
	flags.StringVar(&acked, "acked", "", "")
	err = flags.Parse(args)
	if _, ok := kvEngine.contenders[name]; err == nil && !ok {
		err = fmt.Errorf("no kv contender %q", name)
	}
	return name, dir, acked, err
}
