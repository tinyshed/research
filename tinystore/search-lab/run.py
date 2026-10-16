#!/usr/bin/env python3
"""Serial fresh-process paired runs; binary builds must have finished first."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess as sp
import sys
ROOT=Path(__file__).resolve().parent
OUT=ROOT.parent/'reports/data/search-2026-10-16';OUT.mkdir(exist_ok=True,parents=True)
BIN=Path(os.environ.get('SEARCH_BIN','/target/release/search-lab'))
SCRATCH=Path(os.environ.get('SEARCH_SCRATCH','/data/search-lab'));SCRATCH.mkdir(exist_ok=True)
REV=sp.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
SHA=hashlib.sha256(BIN.read_bytes()).hexdigest()
def tidy():
 for f in SCRATCH.glob('case.db*'):
  if f.is_dir():shutil.rmtree(f)
  else:f.unlink()
def run(group,label,passno,args,env={}):
 path=OUT/(group+'.jsonl')
 if path.exists() and any(r['label']==label and r['pass_number']==passno and r['exit']==0 for r in map(json.loads,path.read_text().splitlines())):return
 tidy();t=dt.datetime.now(dt.timezone.utc).isoformat()
 p=sp.run([str(BIN),*map(str,args)],env=dict(os.environ,BENCH_DB=str(SCRATCH/'case.db'),**env),capture_output=True,text=True,timeout=1800)
 r={'revision':REV,'binary_sha256':SHA,'utc':t,'label':label,'pass_number':passno,'experimental_env':env,'command':'BENCH_DB=<scratch>/case.db <bin> '+' '.join(str(a).replace(str(ROOT),'<harness>')for a in args),'exit':p.returncode,'stderr':p.stderr.replace(str(ROOT),'<harness>').replace(str(SCRATCH),'<scratch>')}
 if not p.returncode:r['result']=json.loads(p.stdout.strip().splitlines()[-1])
 else:r['stdout']=p.stdout
 with path.open('a')as f:f.write(json.dumps(r,ensure_ascii=False)+'\n')
 print(group,label,passno,p.returncode,flush=True);tidy()
 if p.returncode:raise RuntimeError(r)
def quality():
 for corpus in ('ru','en','zh','mixed'):
  pairs=[('fts','unicode'),('fts','icu'),('tant','icu'),('fts','auto'),('tant','auto')]
  if corpus=='mixed':pairs += [('fts','multi'),('tant','multi'),('fts','detect'),('fts','hybrid'),('tant','hybrid')]
  if corpus=='zh':pairs += [('fts','jieba'),('tant','jieba')]
  for passno in (1,2):
   for engine,mode in pairs if passno==1 else reversed(pairs):run('retrieval',f'{corpus}-{engine}-{mode}',passno,['run',engine,mode,ROOT/'corpus'/f'{corpus}.json'])
def checks():
 for passno in (1,2):run('correctness','custom-tokenizer-lifecycle',passno,['check'])
 for engine,mode in [('fts',m)for m in ('unicode','icu','auto','multi','detect','hybrid','jieba')]+[('tant',m)for m in ('icu','auto','multi','hybrid','jieba')]:
  run('gold',f'{engine}-{mode}',1,['run',engine,mode,ROOT/'corpus/gold.json'])
def analyzer():
 for passno in (1,2):
  modes=['icu','auto','multi','detect','hybrid','jieba']
  for mode in modes if passno==1 else reversed(modes):run('analyzer',mode,passno,['analyze',mode,ROOT/'corpus/mixed.json'])
def scale():
 # Million-row complete collection costs are measured by search-store-lab;
 # do not repeat a separate postings-only million-row cross-product.
 for n in (100000,):
  for passno in (1,2):
   for engine in ('fts','tant')if passno==1 else('tant','fts'):run('scale',f'mixed-{n}-{engine}-auto',passno,['run',engine,'auto',ROOT/'corpus/mixed.json',n],{'BENCH_QUERY_LIMIT':'60'})
def concurrent():
 for passno in (1,2):
  for readers in (0,2)if passno==1 else(2,0):run('concurrent',f'fts-auto-100k-readers{readers}',passno,['concurrent',ROOT/'corpus/mixed.json',readers])
if __name__=='__main__':
 {'checks':checks,'quality':quality,'analyzer':analyzer,'scale':scale,'concurrent':concurrent}[sys.argv[1]]()
