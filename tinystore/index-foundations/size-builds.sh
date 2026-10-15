#!/bin/sh
set -eu
# Run in the lab, before all performance work. Does not change the main binary.
cp /target/release/index-foundations /target/release/index-foundations-main
for variant in baseline text geo bitmap; do
  if [ "$variant" = baseline ]; then
    cargo build --release --locked --no-default-features
  else
    cargo build --release --locked --no-default-features --features "$variant"
  fi
  cp /target/release/index-foundations "/target/release/index-foundations-$variant"
  strip "/target/release/index-foundations-$variant"
done
cargo build --release --locked --features alloc-metrics
cp /target/release/index-foundations /target/release/index-foundations-alloc
cp /target/release/index-foundations-main /target/release/index-foundations
python3 - <<'PY'
import datetime,json,os,pathlib,subprocess
variants=['baseline','text','geo','bitmap']
r={'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'profile':'release, stripped ELF, no default features; each variant adds only its named optional dependency','bytes':{v:os.path.getsize('/target/release/index-foundations-'+v) for v in variants},'command':'sh size-builds.sh','interpretation':'Reachable whole research CLI, not incremental library artifact size; source baseline includes SQLite and Snowball. Shared code removal and linker reachability matter.'}
p=pathlib.Path('../reports/data/index-foundations-2026-10-15/binary-sizes.json');p.write_text(json.dumps(r,indent=2)+'\n')
PY
