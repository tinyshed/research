#!/usr/bin/env python3
"""Outer-host controller for disposable 64 MiB cgroups; correctness, not an SLO."""
import datetime as dt
import json
import os
from pathlib import Path
import subprocess as sp

ROOT=Path(__file__).resolve().parent
OUT=ROOT.parent/'reports/data/index-foundations-2026-10-15/memory.jsonl'
REV=sp.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
IMAGE=os.environ.get('RUST_IMAGE','rust:1.99-bookworm')
LAB=os.environ.get('LAB_CONTAINER','channels-locks-lab')
SCRATCH='/data/index-foundations-memory'
BIN='/target/release/index-foundations'
def execute(cmd):return sp.run(cmd,capture_output=True,text=True,timeout=600)
def lab(args):
    p=execute(['docker','exec',LAB,*args]);assert p.returncode==0,p.stderr;return p.stdout
lab(['mkdir','-p',SCRATCH])
for n in (100000,1000000):
    template=f'{SCRATCH}/template-{n}.db'
    lab(['env',f'BENCH_DB={template}',BIN,'prepare','source',str(n),'uniform','ordered'])
    for passno in (1,2):
        for kind in ('fts-set','geo','rstar','tantivy'):
            db=f'{SCRATCH}/case.db'
            lab(['python3','-c','import glob,os,shutil;[(shutil.rmtree(p) if os.path.isdir(p) else os.unlink(p)) for p in glob.glob("'+db+'*")]'])
            lab(['cp',template,db])
            name=f'index-foundations-memory-{kind}-{n}-{passno}'
            if kind=='tantivy':args=['tantivy',str(n),'uniform','prepared']
            elif kind=='rstar':args=['alternative',kind,str(n),'uniform','prepared']
            else:args=['native',kind,str(n),'uniform','ordered','100','prepared']
            command=['docker','run','--name',name,'--cpus','2','--memory','64m','--memory-swap','64m','--network','none',
                '-v','channels-locks-target:/target:ro','-v','channels-locks-data:/data',
                '-e',f'BENCH_DB={db}','-e','BENCH_CACHE_KIB=1024','-e','BENCH_BUILD_ONLY=1',IMAGE,BIN,*args]
            stamp=dt.datetime.now(dt.timezone.utc).isoformat();p=execute(command)
            inspect=execute(['docker','inspect',name,'--format','{{json .State}}']);assert inspect.returncode==0,inspect.stderr
            state=json.loads(inspect.stdout);execute(['docker','rm',name])
            recovery=execute(['docker','exec','-e',f'BENCH_DB={db}',LAB,BIN,'inspect'])
            assert recovery.returncode==0,recovery.stderr
            recovered=json.loads(recovery.stdout.strip().splitlines()[-1]);assert recovered['rows']==n and recovered['id_bounds']==[1,n],recovered
            r={'revision':REV,'utc':stamp,'label':f'{kind}-{n}-64m','pass_number':passno,'memory_limit_bytes':64*1024**2,'swap_limit_bytes':64*1024**2,'exit':p.returncode,'oom_killed':state['OOMKilled'],'stderr':p.stderr.replace(SCRATCH,'<scratch>'),'recovery':recovered,
                'command':f'docker run --cpus 2 --memory 64m --memory-swap 64m --network none -v <target>:/target:ro -v <data>:/data -e BENCH_DB=<scratch>/case.db -e BENCH_CACHE_KIB=1024 -e BENCH_BUILD_ONLY=1 <rust-image> <bin> '+ ' '.join(args)}
            if p.returncode==0:r['result']=json.loads(p.stdout.strip().splitlines()[-1])
            else:r['stdout']=p.stdout
            with OUT.open('a') as f:f.write(json.dumps(r)+'\n')
            print(kind,n,passno,p.returncode,'OOM' if state['OOMKilled'] else '',flush=True)
    lab(['rm',template])
lab(['python3','-c','import glob,os,shutil;[(shutil.rmtree(p) if os.path.isdir(p) else os.unlink(p)) for p in glob.glob("'+SCRATCH+'/case.db*")]'])
