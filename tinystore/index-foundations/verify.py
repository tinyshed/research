#!/usr/bin/env python3
"""Validate complete evidence inventory and independent accounting identities."""
from pathlib import Path
import collections
import datetime
import json
import subprocess
P=Path(__file__).resolve().parent.parent/'reports/data/index-foundations-2026-10-15'
expected={'native-sql':16,'static':96,'locks':24,'channels':108,'rebuilds':120,'outbox':18,'geo-certified':8,'sensitivity':88,'large-crashes':16,'allocations':10,'memory':16,'core-sql':2,'pressure':4,'lease-correctness':4}
rows={k:[json.loads(s) for s in (P/(k+'.jsonl')).read_text().splitlines()] for k in expected}
for k,n in expected.items():
 assert len(rows[k])==n,(k,len(rows[k]),n)
 assert all(r['exit']==0 for r in rows[k]) if k!='memory' else all(r['exit']==0 or r['oom_killed'] for r in rows[k]),k
 labels=collections.defaultdict(list)
 for r in rows[k]:labels[r['label']].append(r['pass_number'])
 assert all(sorted(v)==[1,2] for v in labels.values()),(k,labels)
assert all(r.get('matches_static_checksum') for r in rows['native-sql'])
matched=collections.defaultdict(list)
for r in rows['static']:
 a=r['result'];kind=a.get('kind','tantivy');family='text' if kind.startswith('fts')or kind in ('tantivy','scan-text')else'geo'if kind in ('geo','grid','rstar','scan-geo')else'secondary'
 matched[(a['rows'],a['shape'],r['pass_number'],family)].append(a['query_checksum'])
assert len(matched)==24 and all(len(set(v))==1 for v in matched.values())
for r in rows['locks']:
 a=r['result'];assert sum(a['grants_by_client'])==a['grants'];assert a['attempts']>=a['grants'];assert len(a['grants_by_client'])==a['clients']
for r in rows['channels']:
 a=r['result'];assert a['delivered']==sum(a['deliveredBySubscriber']);assert a['lost']==a['expectedDeliveries']-a['delivered'];assert a['expectedDeliveries']==a['published']*a['subscribers'];assert a['status']['published']==a['published'];assert a['orderingErrors']==0 and a['afterCancel']['active_subscriptions']==0 and a['noReplay']
for r in rows['rebuilds']:
 a=r['result'];assert a['status']=='pass';assert a['validation']['integrity']=='ok';assert all(x['latency_sample_every']==17 for x in a['readers'])
for name in ('correctness','crashes'):
 a=[json.loads(s) for s in (P/(name+'.jsonl')).read_text().splitlines()];latest=a[-1]['revision'];b=[r for r in a if r['revision']==latest];assert len(b)==(26 if name=='correctness' else 72);assert all(r['exit']==0 and r['result']['status']=='pass' for r in b)
result={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'records':expected,'matched_static_groups':24,'latest_correctness_revision':latest,'latest_correctness_records':26,'latest_crash_records':72,'status':'pass','scope':'Inventory, paired passes, result checksums and independent accounting; no product SLO or power-loss proof'}
(P/'verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
