#!/bin/sh
# Fetches MS MARCO passage v1 into <corpus>, the directory given: the
# collection with the dev small queries and judgments from MS MARCO's own
# distribution, and the TREC deep-learning 2019 and 2020 passage queries
# with NIST's judgments. Checks every file against msmarco-sha256.txt in
# tinystore/bench. A proxy goes in https_proxy, as curl reads it.
set -eu
out=$1
here=$(cd "$(dirname "$0")" && pwd)
marco=https://msmarco.z22.web.core.windows.net/msmarcoranking
nist=https://trec.nist.gov/data/deep
mkdir -p "$out"
cd "$out"
get() { [ -s "$2" ] || curl -fL --retry 5 -o "$2" "$1/$2"; }
get "$marco" collectionandqueries.tar.gz
get "$marco" msmarco-test2019-queries.tsv.gz
get "$marco" msmarco-test2020-queries.tsv.gz
get "$nist" 2019qrels-pass.txt
get "$nist" 2020qrels-pass.txt
[ -s collection.tsv ] || tar -xzf collectionandqueries.tar.gz \
  collection.tsv queries.dev.small.tsv qrels.dev.small.tsv
sha256sum -c "$here/../msmarco-sha256.txt"
