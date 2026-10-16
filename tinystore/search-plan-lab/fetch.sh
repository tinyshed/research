#!/bin/sh
# Fetches NoMIRACL at one pinned revision into <corpus>, the directory given,
# and checks every file against corpus.sha256 beside this script.
set -eu
out=$1
here=$(cd "$(dirname "$0")" && pwd)
revision=ecd08778d0426a5ca28ac99763b0c9ddc2c78e68
for language in russian english chinese german french spanish japanese thai; do
  for file in corpus.jsonl.gz topics/dev.relevant.tsv qrels/dev.relevant.tsv; do
    mkdir -p "$out/$language/$(dirname "$file")"
    [ -s "$out/$language/$file" ] || curl -fsSL -o "$out/$language/$file" \
      "https://huggingface.co/datasets/miracl/nomiracl/resolve/$revision/data/$language/$file"
  done
done
cd "$out" && sha256sum -c "$here/corpus.sha256"
