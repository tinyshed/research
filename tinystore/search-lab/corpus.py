#!/usr/bin/env python3
"""Fetch pinned human-judged MIRACL/NoMIRACL pools; never commit their text."""
import concurrent.futures as cf
import gzip
import hashlib
import json
from pathlib import Path
import urllib.request

ROOT=Path(__file__).resolve().parent
DATA=ROOT/'corpus'
OUT=ROOT.parent/'reports/data/search-2026-10-16'
REV='ecd08778d0426a5ca28ac99763b0c9ddc2c78e68'
BASE=f'https://huggingface.co/datasets/miracl/nomiracl/resolve/{REV}/data/'
DATA.mkdir(exist_ok=True);OUT.mkdir(parents=True,exist_ok=True)
def fetch(item):
 lang,name=item; files={}
 for path in ('corpus.jsonl.gz','topics/dev.relevant.tsv','qrels/dev.relevant.tsv'):
  f=DATA/(lang+'-'+path.replace('/','-'));url=BASE+name+'/'+path
  if not f.exists():
   with urllib.request.urlopen(url) as r, f.open('wb') as w:w.write(r.read())
  files[path]={'url':url,'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()}
 corpus=[json.loads(s) for s in gzip.open(DATA/(lang+'-corpus.jsonl.gz'),'rt')]
 print(lang,len(corpus),'first schema',list(corpus[0]),flush=True)
 unique={}
 for d in corpus:
  key=str(d['docid'])
  if key in unique:assert unique[key]==d,('conflicting duplicate',lang,key)
  unique[key]=d
 docs=[]
 for i,d in enumerate(unique.values()):
  docs.append({'id':i+1,'key':str(d.get('_id',d.get('docid',d.get('id')))),'title':d.get('title',''),'body':d.get('text',d.get('body',''))})
 bykey={d['key']:d['id'] for d in docs}
 topics=[s.split('\t',1) for s in (DATA/(lang+'-topics-dev.relevant.tsv')).read_text().splitlines()]
 # Stable hash sample rather than file-prefix topics. No tuning on relevance.
 qrels={};missing=[];incomplete=set()
 for line in (DATA/(lang+'-qrels-dev.relevant.tsv')).read_text().splitlines():
  q,_,doc,rel=line.split()
  if doc not in bykey:missing.append({'query':q,'doc':doc,'relevance':int(rel)});incomplete.add(q);continue
  qrels.setdefault(q,{})[bykey[doc]]=int(rel)
 topics=sorted([a for a in topics if a[0] not in incomplete],key=lambda a:hashlib.sha256(a[0].encode()).hexdigest())[:200]
 queries=[{'key':q,'text':text,'relevant':[d for d,v in qrels[q].items() if v>0],'judged':qrels[q]}for q,text in topics]
 (DATA/(lang+'.json')).write_text(json.dumps({'docs':docs,'queries':queries},ensure_ascii=False))
 return lang,{'upstream_files':files,'upstream_rows':len(corpus),'identical_duplicate_ids':len(corpus)-len(docs),'missing_qrel_documents':missing,'queries_excluded_for_missing_judged_documents':sorted(incomplete),'documents':len(docs),'queries':len(queries),'relevant_judgments':sum(len(q['relevant'])for q in queries),'derived_sha256':hashlib.sha256((DATA/(lang+'.json')).read_bytes()).hexdigest()}
if __name__=='__main__':
 cohorts=dict(cf.ThreadPoolExecutor(max_workers=3).map(fetch,[('ru','russian'),('en','english'),('zh','chinese')]))
 pools={k:json.loads((DATA/(k+'.json')).read_text())for k in cohorts}
 mixed={'docs':[],'queries':[]}
 for lang,p in pools.items():
  offset=len(mixed['docs'])
  mixed['docs'] += [dict(d,id=d['id']+offset,key=lang+':'+d['key'])for d in p['docs']]
  mixed['queries'] += [dict(q,key=lang+':'+q['key'],language=lang,relevant=[i+offset for i in q['relevant']],judged={str(int(i)+offset):v for i,v in q['judged'].items()})for q in p['queries']]
 (DATA/'mixed.json').write_text(json.dumps(mixed,ensure_ascii=False))
 (OUT/'corpus.json').write_text(json.dumps({'dataset':'miracl/nomiracl','revision':REV,'upstream':'https://huggingface.co/datasets/miracl/nomiracl','license_note':'Read upstream README: human judgments CC BY-SA 4.0; Wikipedia passages retain source attribution/license. Text fetched and ignored, not redistributed.','selection':'200 dev.relevant queries/language sorted by SHA256(query id); entire released judged passage pool (dev/test union), not full Wikipedia corpus','cohorts':cohorts,'mixed':{'documents':len(mixed['docs']),'queries':len(mixed['queries'])}},indent=2)+'\n')
