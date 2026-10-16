#!/bin/sh
# Fetches vec1.c, SQLite's vector extension, at its version-0.7 check-in into
# <dir>, the directory given, and checks it; the lab's `vec1` feature builds
# it from VEC1_C=<dir>/vec1.c.
set -eu
out=$1
checkin=8fc7b115a4724ffe89a370deefbee3baadcf195ecb5e69dc89ec4c7b0b401447
mkdir -p "$out"
[ -s "$out/vec1.c" ] || curl -fsSL -A "Mozilla/5.0" -o "$out/vec1.c" \
  "https://sqlite.org/vec1/raw/vec1.c?ci=$checkin"
echo "8571bb4f77f9547d11ad11e2f72e0de7d3b2ab44e7930151998bce9377ed4b86  $out/vec1.c" | sha256sum -c
