package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"slices"
	"strings"
	"time"
)

// engine is one of TinyStore's engines and what it is measured against.
type engine struct {
	order      []string // TinyStore first, then the others as a table lists them
	contenders map[string]opener
	measure    func(ctx context.Context, s subject, seconds float64) ([]stage, error)
}

// opener opens a contender's store in an empty directory, which it owns.
type opener func(ctx context.Context, dir string) (subject, error)

// subject is an opened contender. A service runs as a process of its own,
// whose memory is counted with the client's.
type subject interface {
	close() error
	servicePID() int
}

// services is a contender that runs several: each one's processes count.
type services interface {
	servicePIDs() []int
}

// pidsOf is every service process a subject started, none for a library
func pidsOf(s subject) []int {
	if many, ok := s.(services); ok {
		return many.servicePIDs()
	}
	if pid := s.servicePID(); pid != 0 {
		return []int{pid}
	}
	return nil
}

var engines = map[string]engine{
	"kv":      kvEngine,
	"sqldb":   sqlEngine,
	"metrics": metricsEngine,
	"records": recordsEngine,
	"jobs":    jobsEngine,
	"blobs":   blobsEngine,
	"stack":   stackEngine,
}

func engineNames() []string {
	names := make([]string, 0, len(engines))
	for name := range engines {
		names = append(names, name)
	}
	slices.Sort(names)
	return names
}

// childRun is what one contender did in its own process.
type childRun struct {
	Contender string  `json:"contender"`
	Repeat    int     `json:"repeat"`
	Stages    []stage `json:"stages"`
	// the process's resident memory once its store is open and empty, and its
	// peak over the whole run; a service's is added to the client's
	OpenRSS    int64 `json:"open_rss_bytes"`
	PeakRSS    int64 `json:"peak_rss_bytes"`
	ServiceRSS int64 `json:"service_peak_rss_bytes,omitempty"`
	// a service's processes' proportional set size once the stages ended,
	// which counts memory they share once
	ServicePSS int64 `json:"service_pss_bytes,omitempty"`
	DiskBytes  int64 `json:"disk_bytes"`
	// how long opening took, a service's start included, and how many
	// processes the contender runs, this one counted
	OpenSeconds float64 `json:"open_seconds"`
	Processes   int     `json:"processes"`
}

func (r childRun) summary() string {
	var parts []string
	for _, s := range r.Stages {
		parts = append(parts, fmt.Sprintf("%s×%d %.0f/s p99 %.1fµs", s.Name, s.Goroutines, s.PerSecond, s.P99Micros))
	}
	return fmt.Sprintf("%-10s #%d  peak %.1f MiB  disk %.1f MiB  %s", r.Contender, r.Repeat,
		float64(r.PeakRSS+max(r.ServicePSS, r.ServiceRSS))/(1<<20), float64(r.DiskBytes)/(1<<20), strings.Join(parts, "  "))
}

func runChild(ctx context.Context, args []string) error {
	flags := flag.NewFlagSet("child", flag.ContinueOnError)
	engineName := flags.String("engine", "", "")
	contender := flags.String("contender", "", "")
	dir := flags.String("dir", "", "")
	seconds := flags.Float64("seconds", 5, "")
	if err := flags.Parse(args); err != nil {
		return err
	}
	e, ok := engines[*engineName]
	if !ok {
		return fmt.Errorf("no engine %q", *engineName)
	}
	open, ok := e.contenders[*contender]
	if !ok {
		return fmt.Errorf("no contender %q", *contender)
	}

	run, err := measureContender(ctx, e, open, *dir, *seconds)
	if err != nil {
		return err
	}
	run.Contender = *contender
	return json.NewEncoder(os.Stdout).Encode(run)
}

func measureContender(ctx context.Context, e engine, open opener, dir string, seconds float64) (childRun, error) {
	began := time.Now()
	s, err := open(ctx, dir)
	if err != nil {
		return childRun{}, fmt.Errorf("open: %w", err)
	}
	run := childRun{OpenSeconds: time.Since(began).Seconds()}
	time.Sleep(200 * time.Millisecond) // let a service finish starting before its memory is read
	run.OpenRSS = residentBytes()

	run.Stages, err = e.measure(ctx, s, seconds)
	if err != nil {
		_ = s.close()
		return childRun{}, err
	}
	run.PeakRSS = peakResidentBytes(0)
	run.Processes = 1
	for _, pid := range pidsOf(s) {
		run.ServiceRSS += peakResidentBytes(pid)
		run.ServicePSS += servicePSS(pid)
		run.Processes += len(processTree(pid))
	}
	if err = s.close(); err != nil {
		return childRun{}, fmt.Errorf("close: %w", err)
	}
	run.DiskBytes, err = directoryBytes(dir)
	return run, err
}
