#!/bin/sh
# Round 1z: run a command while holding one of the session's locks.
#
#     sh with-lock.sh <lock folder> <gpu|pairs> <command> [...]
#
# The machine is shared with another round: one job on the graphics card at
# a time, one build of pairs at a time. The lock is a file holding the
# round's name; it is waited for while another holds it and removed when the
# command ends, however it ends.
dir=$1; name=$2; shift 2
lock="$dir/$name.lock"
mkdir -p "$dir"
set -o noclobber
until { echo "round-1z $(date -u +%FT%TZ) $1 $2 $3 $4 $5" > "$lock"; } 2>/dev/null; do sleep 20; done
set +o noclobber
trap 'rm -f "$lock"' EXIT INT TERM
"$@"
