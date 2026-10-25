#!/usr/bin/env bash
# bundles each plugin as `dashbin plugin build` would: one ES module for a worker, the dashbin module left to the host
set -euo pipefail
cd "$(dirname "$0")"
bun install --frozen-lockfile 2>/dev/null || bun install
mkdir -p dist/install
for p in stripe abort; do
  bun build $p.js --target=browser --format=esm --conditions=worker --external dashbin --outfile dist/$p.js
done
for p in install/*.js; do
  bun build $p --target=browser --format=esm --external dashbin --outfile dist/$p
done
ls -l dist dist/install | awk '{print $5, $9}'
