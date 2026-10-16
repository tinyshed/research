#!/usr/bin/env python3
"""Run after performance: inventory, same-query membership and independent qrels."""
import collections
import datetime
import json
import math
from pathlib import Path
import subprocess
ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/search-2026-10-16'
expected={'retrieval':54,'gold':12,'analyzer':12,'concurrent':4,'scale':4,'owned-store':8}
rows={k:[json.loads(s)for s in (DATA/(k+'.jsonl')).read_text().splitlines()]for k in expected}
for k,n in expected.items():
 assert len(rows[k])==n,(k,len(rows[k]),n)
 assert all(r['exit']==0 for r in rows[k]),k
 pairs=collections.defaultdict(list)
 for r in rows[k]:pairs[r['label']].append(r['pass_number'])
 assert all(sorted(v)==([1]if k=='gold'else[1,2])for v in pairs.values()),(k,pairs)
binary=json.loads((DATA/'binary-sizes.json').read_text())
for k in ('retrieval','gold','analyzer','concurrent','scale'):
 assert all(r['binary_sha256']==binary['artifacts']['full']['sha256']for r in rows[k]),k
checks=[json.loads(s)for s in (DATA/'correctness.jsonl').read_text().splitlines()]
assert sorted(r['pass_number']for r in checks if r['exit']==0)==[1,2]
failures=[r for r in checks if r['exit']!=0]
assert len(failures)==1 and 'missing.is_some()'in failures[0]['stderr']
groups=collections.defaultdict(dict)
for r in rows['retrieval']:
 corpus,engine,mode=r['label'].split('-')
 if mode not in ('icu','auto','multi','hybrid'):continue
 groups[corpus,mode,r['pass_number']][engine]=r['result']['set_checks']
assert len(groups)==20
assert all(v['fts']==v['tant']for v in groups.values())
cache={};validated=0
for r in rows['retrieval']+rows['gold']:
 corpus=r['label'].split('-')[0]if r in rows['retrieval']else'gold'
 if corpus not in cache:
  c=json.loads((ROOT/'corpus'/(corpus+'.json')).read_text());cache[corpus]={q['key']:q for q in c['queries']}
 for name in ('query_samples','judged_pool_samples'):
  samples=r['result'][name]
  for sample in samples:
   q=cache[corpus][sample['key']];relevant=set(q['relevant']);ids=sample['ids']
   assert len(ids)==len(set(ids)) and len(ids)<=10
   hits=sum(i in relevant for i in ids);assert hits==sample['hits']
   rr=next((1/(i+1)for i,v in enumerate(ids)if v in relevant),0)
   dcg=sum(1/math.log2(i+2)for i,v in enumerate(ids)if v in relevant)
   ideal=sum(1/math.log2(i+2)for i in range(min(10,len(relevant))))
   assert math.isclose(sample['mrr10'],rr,abs_tol=1e-12)
   assert math.isclose(sample['ndcg10'],dcg/ideal if ideal else 0,abs_tol=1e-12)
   assert math.isclose(sample['recall10'],hits/len(relevant)if relevant else 0,abs_tol=1e-12)
   negatives=sum(q['judged'].get(str(i))==0 for i in ids)
   assert sample['known_negative_at10']==negatives
   if name=='judged_pool_samples':assert all(str(i)in q['judged']for i in ids)
   validated+=1
for r in rows['concurrent']:
 a=r['result'];assert a['status']=='pass'and a['writer_transactions']==1000 and a['rollback_transactions']==31
for r in rows['owned-store']:
 a=r['result'];assert a['status']=='pass'and a['hydrated_bytes']>0 and a['rows']in (100000,1000000)
 assert all(p['commit_and_visibility']['n']==100//p['batch']for p in a['mutation_phases'])
result={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'status':'pass','cohorts':expected,'matched_membership_groups':20,'independently_recomputed_query_scores':validated,'retained_preflight_failure':'missing-tokenizer prepare-only probe','excluded_from_matched_backend_claims':'naive Jieba multi-position query handling','scope':'Inventory, paired passes, membership checksums and relevance recomputation; not product/OS/power-loss guarantees'}
(DATA/'verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
