#!/usr/bin/env python3
"""Build/test the new comparison without touching the archived round."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent;WORK=Path('/work/records-opt');DATA=ROOT.parent/'reports/data/records-optimization-2026-10-09'
env=os.environ.copy();env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1',CARGO_TARGET_DIR='/work/records-opt-target',CARGO_HOME='/work/cargo',RUSTUP_HOME='/work/rustup',SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(ROOT.parent/'sqlite-bench/generated/lib'),SQLITE3_INCLUDE_DIR=str(ROOT.parent/'sqlite-bench/generated/sqlite-amalgamation-3530400'));env['PATH']='/work/cargo/bin:'+env['PATH']
for key in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(key,None)
logs=[]
def call(args,cwd):
 p=subprocess.run(args,cwd=cwd,env=env,text=True,capture_output=True);logs.append({'command':args,'stdout':p.stdout.replace('/src','<repo>').replace('/work','<tmp>'),'stderr':p.stderr.replace('/src','<repo>').replace('/work','<tmp>'),'exit_code':p.returncode});assert p.returncode==0,logs[-1];return p.stdout
WORK.mkdir(exist_ok=True);DATA.mkdir(parents=True,exist_ok=True)
call(['gofmt','-w','.'],ROOT/'go');call(['go','test','-count=1','./...'],ROOT/'go');call(['go','build','-trimpath','-o',str(WORK/'go-records'),'.'],ROOT/'go')
call(['cargo','fmt'],ROOT/'rust');call(['cargo','test','--release','--locked','--offline'],ROOT/'rust')
for extra,dest in [([],WORK/'rust-records'),(['--features','counting-allocator'],WORK/'rust-alloc-records')]:
 call(['cargo','build','--release','--locked','--offline',*extra],ROOT/'rust');shutil.copyfile('/work/records-opt-target/release/tinystore-records-opt-bench',dest)
call(['ldd',str(WORK/'rust-records')],ROOT);call(['readelf','-d',str(WORK/'rust-records')],ROOT)
(DATA/'build.json').write_text(json.dumps({'commands':logs,'binary_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in WORK.glob('*records') if p.is_file()}},indent=2)+'\n');print('Builds and checks complete')
