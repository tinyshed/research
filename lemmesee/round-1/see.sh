#!/bin/sh
# Round 1 end to end on one database: its snapshot, the rules' candidates
# and each configuration's five, the pool run on the database, and the page
# that shows them to compare and to judge. Everything lands in
# lemmesee/corpus/see/<fixture>-<engine>/, which git ignores.
#
#   sh lemmesee/round-1/see.sh postgres|mysql <fixture> "<url>" ["<admin url>"] ["<note>"]
set -eu

here=$(cd "$(dirname "$0")" && (pwd -W 2>/dev/null || pwd))
lemmesee=$(dirname "$here")
bin=$here/harness/target/release
engine=$1
fixture=$2
url=$3
admin=${4:-}
note=${5:-}
out=$lemmesee/corpus/see/$fixture-$engine
mkdir -p "$out"

cargo build --release --quiet --manifest-path "$here/harness/Cargo.toml"
if [ -n "$admin" ]; then set -- --admin "$admin"; else set --; fi
"$bin/snapshot" "$engine" --url "$url" "$@" --out "$out/snapshot.json" > "$out/snapshot.txt"
"$bin/rules" --snapshot "$out/snapshot.json" --out "$out/proposal" > "$out/proposal.txt"
"$bin/run" --snapshot "$out/snapshot.json" --proposal "$out/proposal" --url "$url" "$@" \
  --fixture "$fixture" --note "$note" --out "$out"
