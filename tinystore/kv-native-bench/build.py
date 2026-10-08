#!/usr/bin/env python3
"""Build pinned production Go, stock safe rusqlite and separate diagnostics."""
import hashlib,json,os,subprocess,shutil,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent
WORK=Path(os.environ.get('KV_WORK','/work/kv-native'))
def call(a,**kw):return subprocess.check_output(a,text=True,**kw)
def env():
 e=os.environ.copy();e.update(GOWORK='off',CGO_ENABLED='0',CARGO_TARGET_DIR='/work/kv-target',SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(ROOT.parent/'sqlite-bench/generated/lib'),SQLITE3_INCLUDE_DIR=str(ROOT.parent/'sqlite-bench/generated/sqlite-amalgamation-3530400'))
 for k in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:e.pop(k,None)
 return e
def main():
 WORK.mkdir(exist_ok=True,parents=True);e=env()
 if '--metadata-only' not in sys.argv:
  subprocess.run(['go','mod','tidy'],cwd=ROOT/'go',env=e,check=True)
  subprocess.run(['gofmt','-w','main.go'],cwd=ROOT/'go',check=True)
  subprocess.run(['go','build','-o',str(WORK/'go-kv'),'.'],cwd=ROOT/'go',env=e,check=True)
  subprocess.run(['cargo','fmt','--check'],cwd=ROOT/'rust',env=e,check=True)
  subprocess.run(['cargo','test','--locked'],cwd=ROOT/'rust',env=e,check=True)
  for name,extra in [('rust-kv',[]),('rust-kv-telemetry',['--features','telemetry'])]:
   subprocess.run(['cargo','build','--release','--locked',*extra],cwd=ROOT/'rust',env=e,check=True)
   shutil.copy2(Path(e['CARGO_TARGET_DIR'])/'release/tinystore-kv-native-bench',WORK/name)
 manifest={'collection':'exploratory uncommitted, explicit user waiver of harness commit gate','source':'e307c48a40126aad0e2873b6bf3aaedef8115483','source_note':'host verified commit; submodule Git administrative path is outside container mount','source_content':{},'go':call(['go','version']),'rust':call(['rustc','--version','--verbose']),'native_build':json.loads((ROOT.parent/'sqlite-bench/generated/native-build.json').read_text()),'binaries':{},'source_files':{}}
 for p in sorted((ROOT.parent/'source').rglob('*')):
  if p.is_file() and p.suffix in ['.go','.sql','.mod','.sum'] and not any(x in p.parts for x in ['.git','node_modules']):manifest['source_content'][str(p.relative_to(ROOT.parent/'source'))]=hashlib.sha256(p.read_bytes()).hexdigest()
 for p in WORK.glob('*-kv*'):
  if p.is_file():manifest['binaries'][p.name]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size}
 for p in sorted(ROOT.rglob('*')):
  if p.is_file() and not any(x in p.parts for x in ['target','__pycache__','bin']):manifest['source_files'][str(p.relative_to(ROOT))]=hashlib.sha256(p.read_bytes()).hexdigest()
 (WORK/'build.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':main()
