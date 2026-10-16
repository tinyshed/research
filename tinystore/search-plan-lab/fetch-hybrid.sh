#!/bin/sh
# Fetches the hybrid round's corpora into <dir>, the directory given, each
# at one pinned revision: Amazon's Shopping Queries (ESCI), products and
# judged query-product pairs in English, Spanish and Japanese, Apache-2.0;
# and CQADupStack's twelve StackExchange forums as MTEB keeps them, with
# their judged duplicates. hybrid.sha256 beside this script checks them.
set -eu
out=$1
here=$(cd "$(dirname "$0")" && pwd)
esci=7916cdf6ab75a462e77f20ab40428a10923998d5
mkdir -p "$out/esci"
for file in shopping_queries_dataset_products.parquet shopping_queries_dataset_examples.parquet shopping_queries_dataset_sources.csv; do
  [ -s "$out/esci/$file" ] || curl -fsSL -o "$out/esci/$file" \
    "https://github.com/amazon-science/esci-data/raw/$esci/shopping_queries_dataset/$file"
done
while read -r forum revision; do
  mkdir -p "$out/cqadupstack/$forum/qrels"
  for file in corpus.jsonl queries.jsonl qrels/test.tsv; do
    [ -s "$out/cqadupstack/$forum/$file" ] || curl -fsSL -o "$out/cqadupstack/$forum/$file" \
      "https://huggingface.co/datasets/mteb/cqadupstack-$forum/resolve/$revision/$file"
  done
done <<EOF
android e03f271e3f4a75f49787c838a62f671b35bc9004
english fa6afc4ce926dafd42b7b7c0a19ae40c4632d895
gaming 1c9769860b1d666aae049c6ba60d76ecb244253e
gis 080d0534a7c9fa7b856e77b752c042a6ddbd9714
mathematica fc1af0b6c82dfd3a2aabfb4d9606ba7226268f05
physics 9c2faaa08d173bccf24e1cc3fc1e39bf3caea961
programmers 339629ee116f204b54b1cfd50b9084199a6d415c
stats 6b474f6f6dca1fe17f197f04500b537417692250
tex 2b6f1e278efc77f54ab343b200f5fc7fcd2fc934
unix 3197b09145642430d9db451fcdc1e1987e54d74c
webmasters c691d4af16c19a79df7ee93bfbd12cc186a5bc92
wordpress 6117bc4829afb46d0af6ad6106d1f942e1048281
EOF
if [ -s "$here/hybrid.sha256" ]; then
  cd "$out" && sha256sum -c --quiet "$here/hybrid.sha256"
fi
