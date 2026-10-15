#!/usr/bin/env python3
"""Serial, interleaved fresh-process experiments; run inside the lab container."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import sys

ROOT = Path(__file__).resolve().parent
BIN = os.environ.get('BENCH_BIN', '/target/release/index-foundations')
SCRATCH = Path(os.environ.get('BENCH_SCRATCH', '/data/index-foundations'))
SCRATCH.mkdir(parents=True, exist_ok=True)
OUT = Path(os.environ.get('BENCH_OUT', str(ROOT.parent / 'reports/data/index-foundations-2026-10-15')))
OUT.mkdir(parents=True, exist_ok=True)
REV = sp.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
BASE_ENV = dict(os.environ, RUSSIAN_CORPUS=str(ROOT/'corpus/russian.txt'))

def tidy(path):
    for p in path.parent.glob(path.name+'*'):
        if p.is_dir(): shutil.rmtree(p)
        else: p.unlink()

def command(args, db, timeout=1200):
    env = dict(BASE_ENV, BENCH_DB=str(db))
    t = dt.datetime.now(dt.timezone.utc).isoformat()
    p = sp.run([BIN,*map(str,args)],env=env,text=True,capture_output=True,timeout=timeout)
    record = {'revision':REV,'utc':t,'command':'BENCH_DB=<scratch>/case.db '+ ' '.join(['<bin>',*map(str,args)]),
              'exit':p.returncode,'stderr':p.stderr.replace(str(ROOT),'<harness>').replace(str(SCRATCH),'<scratch>')}
    if p.returncode == 0:
        record['result'] = json.loads(p.stdout.strip().splitlines()[-1])
    else: record['stdout']=p.stdout
    return record

def save(group, label, passno, record):
    record.update(label=label, pass_number=passno)
    with (OUT/(group+'.jsonl')).open('a') as f: f.write(json.dumps(record,ensure_ascii=False)+'\n')
    print(group,passno,label,'OK' if record['exit']==0 else 'FAIL',flush=True)
    if record['exit']!=0: print(record,flush=True);raise RuntimeError(label)

def template(kind,n,shape,order='ordered'):
    p=SCRATCH/f'template-{kind}-{n}-{shape}-{order}.db'
    if not p.exists():
        r=command(['prepare',kind,n,shape,order],p)
        save('preparations',p.stem,0,r)
    return p

def prepared(args,kind,n,shape,order='ordered'):
    src=template(kind,n,shape,order);db=SCRATCH/'case.db';tidy(db);shutil.copyfile(src,db)
    record=command([*args,'prepared'],db)
    record['template_bytes']=src.stat().st_size
    tidy(db);return record

def checks():
    for passno in (1,2):
        for kind in ('fts','geo'):
            for algo in ('frontier','seen','delta'):
                db=SCRATCH/'check.db';tidy(db)
                save('correctness',kind+'-'+algo,passno,command(['rebuild-check',kind,algo],db));tidy(db)
        db=SCRATCH/'quality.db';tidy(db);save('correctness','quality',passno,command(['quality'],db));tidy(db)
        for kind in ('fts','geo'):
            for algo in ('frontier','seen','delta'):
                for stage in ('prepared','chunk-uncommitted','chunk-committed','switch-uncommitted','switched','retire-uncommitted'):
                    db=SCRATCH/'crash.db';tidy(db)
                    proc=sp.Popen([BIN,'crash',kind,algo,stage],env=dict(BASE_ENV,BENCH_DB=str(db)),stdout=sp.PIPE,stderr=sp.PIPE,text=True)
                    ready=proc.stdout.readline();assert json.loads(ready)['ready'],proc.stderr.read()
                    proc.send_signal(signal.SIGKILL);proc.wait(timeout=5)
                    r=command(['recover'],db);r['fault']={'kind':kind,'algorithm':algo,'stage':stage,'signal':'SIGKILL','worker_exit':proc.returncode}
                    save('crashes',kind+'-'+algo+'-'+stage,passno,r);tidy(db)

def static():
    for n in (100000,1000000):
        for shape,kinds in [('uniform',['btree','postings','roaring','scan-secondary','geo','grid','rstar','scan-geo','fts','fts-external','tantivy','scan-text']),('russian',['fts','fts-external','tantivy']),('zipf',['fts','fts-external','tantivy']),('clustered',['geo','grid','rstar'])]:
            for passno in (1,2):
                # Reverse the second pass to expose order/temperature effects.
                for kind in kinds if passno==1 else reversed(kinds):
                    if kind=='tantivy':args=['tantivy',n,shape]
                    elif kind in ('roaring','rstar'):args=['alternative',kind,n,shape]
                    else:args=['native',kind,n,shape,'ordered',100]
                    save('static',f'{n}-{shape}-{kind}',passno,prepared(args,'source',n,shape))
            # Sources reused by rebuild remain; disk is otherwise bounded by one shape at a time.
            for p in SCRATCH.glob(f'template-*-{n}-{shape}-*.db'):p.unlink()

def rebuilds():
    candidates=[('none',0,0,'selective',1,10),('monolithic',0,0,'selective',1,10),('frontier',1000,32,'selective',1,10),('frontier',5000,256,'selective',1,10),('seen',2000,64,'selective',1,10),('delta',2000,64,'selective',1,10),('frontier',2000,64,'full',1,10),('frontier',2000,64,'selective',32,90)]
    for n in (100000,1000000):
        for kind,shape in [('fts','russian'),('geo','uniform'),('geo','clustered')]:
            for passno in (1,2):
                for algo,time,frames,guard,tx,ratio in candidates if passno==1 else reversed(candidates):
                    args=['rebuild',kind,algo,n,shape,time,frames,guard,tx,ratio,2000]
                    label=f'{n}-{shape}-{kind}-{algo}-{time}-{frames}-{guard}-tx{tx}-u{ratio}'
                    save('rebuilds',label,passno,prepared(args,kind,n,shape))
            for p in SCRATCH.glob(f'template-*-{n}-{shape}-*.db'):p.unlink()

def locks():
    for clients in (10,100,1000):
        for resources in (1,clients):
            for passno in (1,2):
                for algo in ('poll','fifo') if passno==1 else ('fifo','poll'):
                    db=SCRATCH/'locks';tidy(db)
                    save('locks',f'{algo}-{clients}-{resources}',passno,command(['locks',algo,clients,resources,3],db));tidy(db)

if __name__=='__main__':
    group=sys.argv[1]
    {'checks':checks,'static':static,'rebuilds':rebuilds,'locks':locks}[group]()
