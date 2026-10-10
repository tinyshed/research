#!/bin/sh
# Where a Bun program's time goes, for trees built as build.sh builds them:
# its JavaScript with the core inside at 64 reads in flight, by bun's own
# profiler, and the core under the bare loop at 256 in flight, by perf with
# whole stacks. Runs in a container that has perf and the volume, the trees
# at /perf/slice-<label>:
#
#   TREES="dur gen" OUT=/src/tinystore/reports/data/<round> sh profiles.sh
#
# cpuprof.py beside this file sums a .cpuprofile by function.
set -u
here=$(dirname "$0")
for tree in ${TREES}; do
	at=/perf/slice-$tree
	rm -rf /perf/run-diag /tmp/prof && mkdir -p /perf/run-diag /tmp/prof
	/perf/bin/bun --cpu-prof --cpu-prof-dir /tmp/prof "$here/bun/bench.ts" \
		--sdk "$at/rust-src/sdk/js/src/index.ts" --flavor rust --mode embedded \
		--binary "$at/bin/tinystore-rust" --library "$at/bin/libtinystore_ffi.so" \
		--dir /perf/run-diag --case kv-get --callers 64 --seconds 6 > /tmp/row.json 2>/dev/null
	{
		echo "# bun --cpu-prof, the SDK with the core inside, kv-get, 64 in flight, 6 s; the last 5 s"
		echo "# tree $tree, tinystore $(cat "$at/rust-commit")"
		cat /tmp/row.json
		python3 "$here/cpuprof.py" /tmp/prof/*.cpuprofile 45 5
	} > "$OUT/profile-bun-$tree.txt"

	rm -rf /perf/run-diag && mkdir -p /perf/run-diag
	perf record -o /tmp/core.data -e cpu-clock -F 999 --call-graph dwarf,16384 -D 3000 -- \
		/perf/bin/bun "$here/bun/raw.ts" --library "$at/bin/libtinystore_ffi.so" --options \
		--dir /perf/run-diag --callers 256 --seconds 6 --decode none > /tmp/row.json 2>/tmp/core.err
	{
		echo "# perf record -e cpu-clock -F 999 --call-graph dwarf, the bare loop, kv-get, 256 in flight, 6 s after 3 s"
		echo "# tree $tree, tinystore $(cat "$at/rust-commit"); every thread, by what its time is under (children)"
		cat /tmp/row.json
		perf report -i /tmp/core.data --children --sort symbol --stdio -g none 2>/dev/null |
			grep -v '^#' | grep -v '^$' | head -120 | cut -c1-200
	} > "$OUT/profile-core-$tree.txt"
	echo "profiled $tree"
done
