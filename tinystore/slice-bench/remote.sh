#!/bin/sh
# The harness's cases on a host without Docker: the programs copied into
# <dir>/bin, a process and a fresh store a case, the programs in turn and
# their order turned round every other pass. It prints a line of JSON a run
# and writes the lines to <out>, with the host's processor and kernel first.
#
#   remote.sh <dir> <out> "<name>=<binary> ..." "<case>:<callers> ..."
#
# PASSES and SECONDS_A_CASE are the passes and the seconds a case, 3 and 3.
set -eu

dir=$1
out=$2
programs=$3
cases=$4
passes=${PASSES:-3}
seconds=${SECONDS_A_CASE:-3}

turned() {
	for program in $programs; do turned="$program ${turned:-}"; done
	echo "$turned"
	unset turned
}

model=$(lscpu | sed -n 's/^Model name: *//p' | head -1)
echo "{\"host\": {\"processors\": $(nproc), \"model\": \"$model\", \"kernel\": \"$(uname -sr)\", \"libc\": \"$(ldd --version | head -1)\", \"virtualization\": \"$(systemd-detect-virt 2>/dev/null || echo unknown)\", \"started_utc\": \"$(date -u +%Y-%m-%dT%H:%M:%S+00:00)\", \"passes\": $passes, \"seconds_a_case\": $seconds}}" | tee "$out"

pass=0
while [ "$pass" -lt "$passes" ]; do
	order=$programs
	[ $((pass % 2)) -eq 1 ] && order=$(turned)
	for each in $cases; do
		for program in $order; do
			rm -rf "$dir/store" && mkdir -p "$dir/store"
			row=$("$dir/bin/${program#*=}" --dir "$dir/store" --case "${each%%:*}" --callers "${each##*:}" --seconds "$seconds" | tail -1)
			echo "{\"program\": \"${program%%=*}\", \"pass\": $pass, \"row\": $row}" | tee -a "$out"
		done
	done
	pass=$((pass + 1))
done
rm -rf "$dir/store"
echo "{\"finished_utc\": \"$(date -u +%Y-%m-%dT%H:%M:%S+00:00)\"}" | tee -a "$out"
