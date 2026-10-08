#!/usr/bin/env python3
"""Isolated builds; original executables/source are hash checked, never changed."""
import hashlib,json,os,subprocess,shutil,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent;WORK=Path(os.environ.get('KV_WORK','/work/kv-opt'));OLD=Path('/work/kv-native')
def call(a,**kw):return subprocess.check_output(a,text=True,**kw)
def env():
 e=os.environ.copy();e.update(GOWORK='off',CGO_ENABLED='0',CARGO_TARGET_DIR='/work/kv-opt-target',SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(ROOT.parent/'sqlite-bench/generated/lib'),SQLITE3_INCLUDE_DIR=str(ROOT.parent/'sqlite-bench/generated/sqlite-amalgamation-3530400'))
 for k in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:e.pop(k,None)
 return e
def files(root):return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest()for p in sorted(root.rglob('*'))if p.is_file()and not any(x in p.parts for x in ['target','__pycache__','bin'])}
def main():
 WORK.mkdir(exist_ok=True,parents=True);e=env()
 originals={'go-kv':'b31774ad7b97038d8ef8bec319d8143b9807286fcdebc166042818d396e7bf87','rust-kv':'a3dca0fb4af38129b50ba378ffd55dbf9bd2f8708cd2e48bc06256a38136cd71','rust-kv-telemetry':'09c72cd893eaf9925ee918c79009278d250058e1e5eac7416c14e5b906a9416d'}
 for n,h in originals.items():assert hashlib.sha256((OLD/n).read_bytes()).hexdigest()==h,(n,'original binary changed')
 if '--metadata-only'not in sys.argv:
  subprocess.run(['go','mod','tidy'],cwd=ROOT/'go',env=e,check=True);subprocess.run(['gofmt','-w','main.go'],cwd=ROOT/'go',check=True);subprocess.run(['go','build','-o',str(WORK/'go-kv'),'.'],cwd=ROOT/'go',env=e,check=True)
  subprocess.run(['cargo','fmt','--check'],cwd=ROOT/'rust',env=e,check=True);subprocess.run(['cargo','test','--locked'],cwd=ROOT/'rust',env=e,check=True)
  for n,features in [('rust-kv',[]),('rust-kv-telemetry',['telemetry']),('rust-kv-profile',['profile'])]:
   subprocess.run(['cargo','build','--release','--locked',*(['--features',','.join(features)]if features else[])],cwd=ROOT/'rust',env=e,check=True);shutil.copy2(Path(e['CARGO_TARGET_DIR'])/'release/tinystore-kv-opt-bench',WORK/n)
 manifest={'collection':'exploratory uncommitted; user waived harness commit gate','source':'e307c48a40126aad0e2873b6bf3aaedef8115483','prior_research_commit':'4e82af66b38ee057e29d3bbc7b25abd32320d95e','go':call(['go','version']),'rust':call(['rustc','--version','--verbose']),'native_build':json.loads((ROOT.parent/'sqlite-bench/generated/native-build.json').read_text()),'binaries':{p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in WORK.glob('*-kv*')if p.is_file()},'source_files':files(ROOT),'original_source_files':files(ROOT.parent/'kv-native-bench'),'original_binaries':originals}
 (WORK/'build.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':main()
