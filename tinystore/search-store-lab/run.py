#!/usr/bin/env python3
"""Run after search-lab timing and all builds finish; standalone storage controls."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess as sp
ROOT=Path(__file__).resolve().parent
OUT=ROOT.parent/'reports/data/search-2026-10-16/owned-store.jsonl'
BIN=Path('/target/release/search-store-lab')
SCRATCH=Path('/data/search-store-lab');SCRATCH.mkdir(exist_ok=True)
REV=sp.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
SHA=hashlib.sha256(BIN.read_bytes()).hexdigest()
for n in (100000,1000000):
 for passno in (1,2):
  for engine in ('fts','tant')if passno==1 else('tant','fts'):
   label=f'{n}-{engine}-owned-hydrated'
   if OUT.exists()and any(r['label']==label and r['pass_number']==passno and r['exit']==0 for r in map(json.loads,OUT.read_text().splitlines())):continue
   db=SCRATCH/'case'
   for f in SCRATCH.glob('case*'):
    if f.is_dir():shutil.rmtree(f)
    else:f.unlink()
   cmd=[str(BIN),engine,str(ROOT.parent/'search-lab/corpus/mixed.json'),str(n)]
   stamp=dt.datetime.now(dt.timezone.utc).isoformat()
   p=sp.run(cmd,env=dict(os.environ,BENCH_DB=str(db)),capture_output=True,text=True,timeout=1800)
   r={'utc':stamp,'revision':REV,'binary_sha256':SHA,'label':label,'pass_number':passno,'command':'BENCH_DB=<scratch>/case <bin> '+engine+' <corpus>/mixed.json '+str(n),'exit':p.returncode,'stderr':p.stderr.replace(str(ROOT),'<harness>').replace(str(SCRATCH),'<scratch>')}
   if not p.returncode:r['result']=json.loads(p.stdout.strip().splitlines()[-1])
   else:r['stdout']=p.stdout
   with OUT.open('a')as f:f.write(json.dumps(r)+'\n')
   print(label,passno,p.returncode,flush=True)
   if p.returncode:raise RuntimeError(r)
for f in SCRATCH.glob('case*'):
 if f.is_dir():shutil.rmtree(f)
 else:f.unlink()
