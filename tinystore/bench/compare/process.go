package main

import (
	"bufio"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"runtime/debug"
	"strconv"
	"strings"
)

// residentBytes is this process's resident memory now, VmRSS
func residentBytes() int64 {
	return statusBytes("/proc/self/status", "VmRSS:")
}

// peakResidentBytes is a process's peak resident memory, VmHWM; pid 0 is this
// one. The kernel keeps the peak, so a brief one between samples is not missed.
func peakResidentBytes(pid int) int64 {
	path := "/proc/self/status"
	if pid != 0 {
		path = "/proc/" + strconv.Itoa(pid) + "/status"
	}
	return statusBytes(path, "VmHWM:")
}

// statusBytes reads one kB line of /proc/…/status, as Linux writes it:
//
//	VmHWM:     28712 kB   →   29401088
func statusBytes(path, field string) int64 {
	f, err := os.Open(path)
	if err != nil {
		return 0
	}
	defer f.Close()
	lines := bufio.NewScanner(f)
	for lines.Scan() {
		rest, ok := strings.CutPrefix(lines.Text(), field)
		if !ok {
			continue
		}
		kb, err := strconv.ParseInt(strings.TrimSuffix(strings.TrimSpace(rest), " kB"), 10, 64)
		if err != nil {
			return 0
		}
		return kb << 10
	}
	return 0
}

// directoryBytes is the size of every file under dir, as ls counts it
func directoryBytes(dir string) (int64, error) {
	var total int64
	err := filepath.WalkDir(dir, func(_ string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() {
			return err
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if info.Mode().IsRegular() {
			total += info.Size()
		}
		return nil
	})
	return total, err
}

// environment is what a round ran on, as its report states it.
type environment struct {
	Go         string            `json:"go"`
	GOMAXPROCS int               `json:"gomaxprocs"`
	CPU        string            `json:"cpu"`
	Kernel     string            `json:"kernel"`
	Modules    map[string]string `json:"modules"`
}

func describeEnvironment() environment {
	e := environment{Go: runtime.Version(), GOMAXPROCS: runtime.GOMAXPROCS(0), Modules: map[string]string{}}
	if release, err := os.ReadFile("/proc/sys/kernel/osrelease"); err == nil {
		e.Kernel = strings.TrimSpace(string(release))
	}
	if info, err := os.ReadFile("/proc/cpuinfo"); err == nil {
		for line := range strings.Lines(string(info)) {
			if name, ok := strings.CutPrefix(line, "model name"); ok {
				e.CPU = strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(name), ":"))
				break
			}
		}
	}
	if build, ok := debug.ReadBuildInfo(); ok {
		for _, m := range build.Deps {
			version := m.Version
			if m.Replace != nil {
				version = "replaced by " + m.Replace.Path
			}
			e.Modules[m.Path] = version
		}
	}
	return e
}
