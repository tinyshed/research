#!/usr/bin/env python3
"""Matched best-native build control. Invoked serially before outbox runs."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess as sp
import run

ROOT=Path(__file__).resolve().parent
OUT=run.OUT/'native-sql.jsonl'
TARGET=Path(os.environ.get('LAB_TARGET','/target/release'))
MAIN=TARGET/'index-foundations'
SQL=TARGET/'index-foundations-sql-control'
rows=[json.loads(s) for s in OUT.read_text().splitlines()] if OUT.exists() else []
complete={(r['label'],r['pass_number']) for r in rows if r['exit']==0 and r.get('matches_static_checksum')}
if len(complete)==16:
    print('native-sql paired controls already complete',flush=True)
    raise SystemExit(0)
# No performance work is active here: finish-series waits for this driver before
# starting outbox, and main rebuilds finished before it invoked the outbox driver.
backup=TARGET/'index-foundations-before-sql-control'
shutil.copyfile(MAIN,backup)
try:
    sp.run(['cargo','build','--release','--locked'],cwd=ROOT,check=True)
    shutil.copyfile(MAIN,SQL)
finally:
    shutil.copyfile(backup,MAIN)
run.BIN=str(SQL)
sha=hashlib.sha256(SQL.read_bytes()).hexdigest()
static=[json.loads(s) for s in (run.OUT/'static.jsonl').read_text().splitlines()]
reference={r['label']:r['result']['query_checksum'] for r in static}
run.BASE_ENV['BENCH_VERIFY_FTS']='1'
for n in (100000,1000000):
    for kind in ('fts','fts-set'):
        for passno in (1,2):
            for mode in ('row-loop','sql-select') if passno==1 else ('sql-select','row-loop'):
                label=f'{n}-russian-{kind}-{mode}'
                if (label,passno) in complete:continue
                if mode=='sql-select':run.BASE_ENV['BENCH_SQL_BULK']='1'
                else:run.BASE_ENV.pop('BENCH_SQL_BULK',None)
                r=run.prepared(['native',kind,n,'russian','ordered',100],'source',n,'russian')
                r['binary_sha256']=sha
                r['binary_source_revision']=run.REV
                r['experimental_env']={'BENCH_VERIFY_FTS':'1','BENCH_SQL_BULK':'1' if mode=='sql-select' else 'unset'}
                r['matches_static_checksum']=r['exit']==0 and r['result']['query_checksum']==reference[f'{n}-russian-{kind}']
                run.save('native-sql',label,passno,r)
                assert r['matches_static_checksum'],label
    run.tidy(run.SCRATCH/f'template-source-{n}-russian-ordered.db')
