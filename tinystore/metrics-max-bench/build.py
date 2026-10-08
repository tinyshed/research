#!/usr/bin/env python3
"""Build independent portable, LTO, host-target, telemetry and PGO binaries."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parent
TARGET=ROOT/'build-targets'
PGO=ROOT/'pgo'
FLAGS=['--fast-exact','1','--fast-codec','1','--threads','0','--tuning','15']

def call(args,env=None,cwd=None):
 result=subprocess.run(args,env=env,cwd=cwd,text=True,capture_output=True)
 if result.returncode:raise RuntimeError(f'{args}\n{result.stdout}\n{result.stderr}')
 return result.stdout

def environment():
 env=os.environ.copy()
 env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1')
 for name in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_LTO','CARGO_PROFILE_RELEASE_CODEGEN_UNITS']:env.pop(name,None)
 generated=ROOT.parent/'sqlite-bench/generated'
 env.update(SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(generated/'lib'),SQLITE3_INCLUDE_DIR=str(generated/'sqlite-amalgamation-3530400'))
 return env

def compile_one(name,flags='',features=()):
 print('Build '+name,flush=True)
 target_dir=ROOT/'rust/target' if name in ['default','telemetry'] else TARGET/name
 env=environment();env.update(CARGO_TARGET_DIR=str(target_dir))
 if flags:env['RUSTFLAGS']=flags
 args=['cargo','build','--release','--locked','--manifest-path',str(ROOT/'rust/Cargo.toml')]
 if features:args+=['--features',','.join(features)]
 output=call(args,env=env)
 binary=ROOT/'bin'/('rust-'+name);shutil.copy2(target_dir/'release/tinystore-metrics-native-bench',binary)
 return {'name':name,'flags':flags,'features':features,'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'bytes':binary.stat().st_size,'build_output':output}

def train(binary,env):
 PGO.mkdir(exist_ok=True);raw=PGO/'raw';raw.mkdir(exist_ok=True)
 for old in raw.glob('*.profraw'):old.unlink()
 env=env.copy();env['LLVM_PROFILE_FILE']=str(raw/'%p-%m.profraw')
 # Wide64, full16, edge corpus, prefix/NoneOf and grouped operations are held
 # out of training; verification and measurement exercise them afterwards.
 scenarios=[('read_head_full8','head',256),('read_sealed_full8','sealed',256),
  ('aggregate_cut_sum','sealed',256),('aggregate_count','sealed',256),
  ('ingest_scrape100','scrape',64),('maintain_ready','ready',1)]
 rows=[]
 for case,kind,count in scenarios:
  with tempfile.TemporaryDirectory(prefix='pgo-',dir=ROOT/'stores') as d:
   db=Path(d)/'metrics.db';shutil.copyfile(ROOT/'stores'/kind/'metrics.db',db)
   args=[str(binary),'--db',str(db),'--mode','bench','--case',case,'--iterations',str(count),'--warm','0' if count==1 else '32',*FLAGS]
   row=json.loads(call(args,env=env));row['fixture']=kind;rows.append(row)
 return rows,sorted(raw.glob('*.profraw'))

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--only',nargs='*',default=['default','telemetry','lto','native','pgo']);args=parser.parse_args()
 (ROOT/'bin').mkdir(exist_ok=True);TARGET.mkdir(exist_ok=True)
 metadata={'portable_lto_flags':'-C lto=thin -C embed-bitcode=yes -C codegen-units=1','builds':[]}
 lto=metadata['portable_lto_flags'];env=environment()
 for name in args.only:
  if name=='default':metadata['builds'].append(compile_one('default'))
  elif name=='telemetry':metadata['builds'].append(compile_one('telemetry',features=['telemetry']))
  elif name=='lto':metadata['builds'].append(compile_one('lto',lto))
  elif name=='native':metadata['builds'].append(compile_one('native',lto+' -C target-cpu=native'))
  elif name=='pgo':
   PGO.mkdir(exist_ok=True)
   metadata['builds'].append(compile_one('pgo-train',lto+' -C profile-generate='+str(PGO/'raw')))
   metadata['training'],raw=train(ROOT/'bin/rust-pgo-train',env)
   sysroot=Path(call(['rustc','--print','sysroot'],env=env).strip())
   tool=sysroot/'lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata'
   merged=PGO/'merged.profdata'
   call([str(tool),'merge','-o',str(merged),*[str(p) for p in raw]],env=env)
   metadata['profile_sha256']=hashlib.sha256(merged.read_bytes()).hexdigest()
   metadata['profdata_tool']=call([str(tool),'--version'],env=env).strip()
   metadata['builds'].append(compile_one('pgo',lto+' -C profile-use='+str(merged)))
  else:raise ValueError(name)
 metadata['rustc']=call(['rustc','--version','--verbose'],env=env)
 # Separate invocations can prepare the diagnostic build early without losing
 # the provenance of later compiler variants.
 p=ROOT/'build-metadata.json'
 if p.exists():
  old=json.loads(p.read_text());seen={row['name'] for row in metadata['builds']}
  metadata['builds']=[r for r in old.get('builds',[]) if r['name'] not in seen]+metadata['builds']
  for key in ['training','profile_sha256','profdata_tool']:
   if key not in metadata and key in old:metadata[key]=old[key]
 p.write_text(json.dumps(metadata,indent=2)+'\n')
 print('Build variants ready',flush=True)

if __name__=='__main__':main()
