#!/usr/bin/env python3
"""Build/test records harness, including a separate allocation binary."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent
WORK=Path('/work/records-native');DATA=ROOT.parent/'reports/data/records-native-2026-10-08'
env=os.environ.copy();env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1',CARGO_TARGET_DIR='/work/records-target',CARGO_HOME='/work/cargo',RUSTUP_HOME='/work/rustup',SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(ROOT.parent/'sqlite-bench/generated/lib'),SQLITE3_INCLUDE_DIR=str(ROOT.parent/'sqlite-bench/generated/sqlite-amalgamation-3530400'))
env['PATH']='/work/cargo/bin:'+env['PATH']
for name in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(name,None)
def call(args,cwd=ROOT):
 p=subprocess.run(args,cwd=cwd,env=env,text=True,capture_output=True)
 logs.append({'command':args,'stdout':p.stdout.replace('/src','<repo>').replace('/work','<tmp>'),'stderr':p.stderr.replace('/src','<repo>').replace('/work','<tmp>'),'exit_code':p.returncode})
 if p.returncode:raise RuntimeError(logs[-1])
 return p.stdout
logs=[];WORK.mkdir(exist_ok=True);DATA.mkdir(exist_ok=True,parents=True)
call(['gofmt','-w','.'],ROOT/'go');call(['go','test','-count=1','./...'],ROOT/'go');call(['go','build','-trimpath','-o',str(WORK/'go-records'),'.'],ROOT/'go')
call(['cargo','fmt','--check'],ROOT/'rust');call(['cargo','test','--release','--locked','--offline'],ROOT/'rust')
for features,destination in [([],WORK/'rust-records'),(['--features','counting-allocator'],WORK/'rust-alloc-records')]:
 call(['cargo','build','--release','--locked','--offline',*features],ROOT/'rust');shutil.copyfile('/work/records-target/release/tinystore-records-native-bench',destination)
call(['ldd',str(WORK/'rust-records')]);call(['readelf','-d',str(WORK/'rust-records')])
(DATA/'build.json').write_text(json.dumps({'commands':logs,'binary_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [WORK/'go-records',WORK/'rust-records',WORK/'rust-alloc-records']}},indent=2)+'\n')
print('Records builds and checks complete')
