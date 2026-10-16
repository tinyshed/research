#!/usr/bin/env python3
"""Run only after compilation and other experiments have stopped."""
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import subprocess as sp

ROOT=Path(__file__).resolve().parent
OUT=ROOT.parent/'reports/data/index-foundations-2026-10-15/outbox.jsonl'
BIN=os.environ.get('OUTBOX_BIN','/target/release/index-outbox-lab')
PREP=os.environ.get('PREP_BIN','/target/release/index-foundations')
SCRATCH=Path(os.environ.get('OUTBOX_SCRATCH','/data/index-outbox-lab'))
ENV=dict(os.environ,RUSSIAN_CORPUS=str(ROOT.parent/'index-foundations/corpus/russian.txt'))
REV=sp.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
SCRATCH.mkdir(exist_ok=True)
def tidy(db):
    for p in db.parent.glob(db.name+'*'):
        if p.is_dir():shutil.rmtree(p)
        else:p.unlink()
def run(args,db,label,passno):
    stamp=dt.datetime.now(dt.timezone.utc).isoformat()
    p=sp.run([BIN,*map(str,args)],env=dict(ENV,BENCH_DB=str(db)),capture_output=True,text=True,timeout=600)
    r={'revision':REV,'binary_source_revision':BINARY_REV,'experimental_env':{'BENCH_SQL_BULK':'1'},'utc':stamp,'label':label,'pass_number':passno,'command':'BENCH_SQL_BULK=1 BENCH_DB=<scratch>/case.db <outbox-bin> '+' '.join(map(str,args)),'exit':p.returncode,'stderr':p.stderr.replace(str(ROOT),'<harness>').replace(str(SCRATCH),'<scratch>')}
    if p.returncode==0:r['result']=json.loads(p.stdout.strip().splitlines()[-1])
    else:r['stdout']=p.stdout
    with OUT.open('a') as f:f.write(json.dumps(r)+'\n')
    print(label,passno,p.returncode,flush=True)
    if p.returncode:raise RuntimeError(r)
# Serial best-native control; it restores the original main binary afterward.
sp.run(['python3',str(ROOT.parent/'index-foundations/native-sql.py')],check=True)
sp.run(['cargo','build','--release','--locked'],cwd=ROOT,check=True)
BINARY_REV=sp.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
ENV['BENCH_SQL_BULK']='1'
for n in (100000,1000000):
    template=SCRATCH/'template.db';tidy(template)
    sp.run([PREP,'prepare','source',str(n),'russian','ordered'],env=dict(ENV,BENCH_DB=str(template)),check=True,capture_output=True)
    for passno in (1,2):
        for mode in ('native','outbox'):
            db=SCRATCH/'case.db';tidy(db);shutil.copyfile(template,db)
            run(['run',mode,n,'prepared'],db,f'{mode}-{n}',passno);tidy(db)
    tidy(template)
for passno in (1,2):
    for stage in ('source-only','external-uncommitted','external-before-ack','ack-before-cleanup','cleanup-committed'):
        db=SCRATCH/'crash.db';tidy(db)
        p=sp.Popen([BIN,'crash',stage],env=dict(ENV,BENCH_DB=str(db)),stdout=sp.PIPE,stderr=sp.PIPE,text=True)
        ready=p.stdout.readline()
        assert ready and json.loads(ready)['ready'],p.stderr.read()
        p.kill();p.wait(timeout=5)
        run(['recover'],db,stage,passno);tidy(db)
