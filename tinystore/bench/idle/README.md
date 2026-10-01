# Small Consumer Memory

One cgo-free, stripped Go program per case. `empty` links only the standard
library and the shared sampling protocol. Every other program links the
root and only the engine it opens; `all` opens every engine in separate files.
The test checks that this dependency boundary stays true.

Each child runs in a new directory and reports four phases:

1. `linked`: package initialization has run, but no store is open.
2. `opened`: default `Open`, plus one empty bucket/queue where applicable.
3. `gc`: an explicit GC; no artificial memory return to the OS.
4. `released`: `debug.FreeOSMemory`, a diagnostic rather than normal idle.

The parent waits 250 ms after each response and samples `/proc/status` and
`smaps_rollup`. RSS, PSS, anonymous/file RSS and the process's high-water RSS
stay separate from Go's live heap and reserved memory. No corpus is loaded.
The high-water mark here is **startup peak**, not peak under an application
workload. It must not be joined with another binary's load peak in a chart.

Builds finish before sampling. Five passes reverse case order every second
pass. Source and harness commits, compiler, CPU and kernel are recorded.

```sh
docker run --rm -v <research>:/src -v idle-data:/data \
  -v idle-go:/go -v idle-cache:/root/.cache/go-build \
  -e GOWORK=off -e CGO_ENABLED=0 -e GOFLAGS=-buildvcs=false \
  -e TINYSTORE_COMMIT=<source> -e HARNESS_COMMIT=<harness> \
  -e OUT=results/<machine> -w /src/tinystore/bench/idle \
  tinystore-compare-sdk sh run.sh
```
