#!/bin/sh
# SchemaPile-Perm, Zenodo record 12682521 (version 0.2, 7 July 2024, CC BY 4.0;
# each schema under its own permissive licence): fetch its JSON into
# lemmesee/corpus/ and check it against hashes.txt.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
corpus=$(cd "$here/.." && pwd)/corpus
dir=$corpus/schemapile-perm/12682521
mkdir -p "$dir"
[ -f "$dir/schemapile-perm.json.gz" ] || curl -sSfL -o "$dir/schemapile-perm.json.gz" \
  "https://zenodo.org/records/12682521/files/schemapile-perm.json.gz?download=1"
(cd "$corpus" && grep -v '^#' "$here/hashes.txt" | sha256sum -c -)
