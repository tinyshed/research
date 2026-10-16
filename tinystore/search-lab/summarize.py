#!/usr/bin/env python3
"""Read-only tables: every paired pass retained, no averaging scenarios."""
import collections
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/search-2026-10-16'
OUT=ROOT.parent/'reports/search-measurements-2026-10-16.md'
lines=['# Search measurement tables — 16 October 2026','','Values separated by `;` are pass 1; pass 2. These are native research CLI measurements, not public Core/Wire/SDK rates. See the architecture report and raw ledger for corpus, semantics and durability boundaries.','']
def read(name):
 p=DATA/(name+'.jsonl');return [json.loads(x)for x in p.read_text().splitlines()]if p.exists()else[]
def grouped(name):
 g=collections.defaultdict(list)
 for r in read(name):
  if r['exit']==0:g[r['label']].append(r)
 return [(k,sorted(rs,key=lambda r:r['pass_number']))for k,rs in g.items()]
def f(x):return f'{x:.3f}'
def ms(x):return f(x/1000)
def mib(x):return f(x/1048576)
def vals(rs,fn):return '; '.join(str(fn(r['result']))for r in rs)
def table(title,headers,rows):
 if not rows:return
 lines.extend(['## '+title,'','| '+' | '.join(headers)+' |','|'+'|'.join('---'for h in headers)+'|'])
 for r in rows:lines.append('| '+' | '.join(map(str,r))+' |')
 lines.append('')
p=DATA/'binary-sizes.json'
if p.exists():
 b=json.loads(p.read_text())['artifacts'];base=b['baseline']['bytes'];table('Stripped Linux CLI binaries',['Profile','Bytes','MiB','Delta from baseline MiB'],[[k,v['bytes'],mib(v['bytes']),mib(v['bytes']-base)]for k,v in b.items()])
 lines.extend(['Whole reachable executables with the same release/thin-LTO profile. ICU segmentation data/models, Whatlang and Jieba dictionaries are embedded as enabled; this is not a measured increase to the TinyStore product binary.',''])
for group in ('retrieval','scale'):
 rows=[];quality=[]
 for label,rs in grouped(group):
  rows.append([label,vals(rs,lambda a:a['rows']),vals(rs,lambda a:f(a['build_us']/1e6)),vals(rs,lambda a:ms(a['queries']['p50_us'])),vals(rs,lambda a:ms(a['queries']['p95_us'])),vals(rs,lambda a:ms(a['queries']['p99_us'])),vals(rs,lambda a:f(a['query_per_s'])),vals(rs,lambda a:mib(a['build_external_bytes']if a['engine']=='tant'else sum(o['bytes']for o in a['build_objects']if o['name'].startswith('search_')))),vals(rs,lambda a:mib(a['source_bytes'])),vals(rs,lambda a:f(a['build_memory']['rss_kib']/1024)),vals(rs,lambda a:mib(a['build_wal_file_bytes']))])
  if group=='retrieval':quality.append([label,vals(rs,lambda a:f(a['quality']['ndcg10'])),vals(rs,lambda a:f(a['quality']['mrr10'])),vals(rs,lambda a:f(a['quality']['recall10'])),vals(rs,lambda a:f(a['quality']['known_negative_per_query'])),vals(rs,lambda a:f(a['quality']['judged_per_query'])),vals(rs,lambda a:f(a['judged_pool_quality']['ndcg10'])),vals(rs,lambda a:f(a['judged_pool_quality']['recall10']))])
 table(group.capitalize()+' build/top-10',['Scenario','Docs','Build s','Query p50 ms','p95 ms','p99 ms','Query/s','Index MiB','Source MiB','Build RSS MiB','Build WAL file MiB'],rows)
 table('Natural query relevance (200 queries/language; mixed=600)',['Scenario','nDCG@10','MRR@10','Recall@10','Known negatives/query','Judged docs/query','Judged-pool nDCG@10','Judged-pool recall@10'],quality)
 if rows:lines.extend(['Main query timings return IDs, not hydrated source documents. Both engines index full positions/frequencies. Ranked BM25 defaults, field normalization and synonym scoring differ; same analyzer does not imply identical ranks. Unicode61 is an analyzer baseline, not the same token semantics as ICU. Standalone source/index commits in the Tantivy arm are independent. Scale repeats the natural pool and limits to the first 60 queries (RU, over mixed RU/EN/ZH documents); its relevance values are not population-quality measurements.',''])
rows=[]
for label,rs in grouped('retrieval'):
 for i,phase in enumerate(rs[0]['result']['mutations']):
  rows.append([label,phase['operation'],vals(rs,lambda a,i=i:ms(a['mutations'][i]['source_commit']['p99_us'])),vals(rs,lambda a,i=i:ms(a['mutations'][i]['external_commit_reload_us'])),vals(rs,lambda a,i=i:mib(a['mutations'][i]['wal_file_bytes']))])
table('Source-backed incremental phases: 1000 rows, 100 rows/commit',['Scenario','Operation','SQL/enqueue batch p99 ms','External commit/reload total ms','SQLite WAL MiB'],rows)
lines.extend(['For Tantivy the first latency includes SQLite work and enqueue; its separate durable commits/reloads are the next column, a total over ten commits. Do not quote the first column as complete durable Search write latency or zero SQLite WAL as zero external I/O. The owned-store control below measures complete commit/visibility latency.',''])
rows=[]
for label,rs in grouped('analyzer'):
 rows.append([label,vals(rs,lambda a:mib(a['bytes'])),vals(rs,lambda a:a['tokens']),vals(rs,lambda a:f(a['mib_per_s'])),vals(rs,lambda a:ms(a['setup_us'])),vals(rs,lambda a:f((a['ready_memory']['rss_kib']-a['idle_memory']['rss_kib'])/1024)),vals(rs,lambda a:f(a['end_memory']['hwm_kib']/1024))])
table('Analyzer sample (every floor(N/5000)-th mixed document, both fields)',['Mode','Input MiB','Emitted tokens','MiB/s','Init ms','Init RSS delta MiB','HWM MiB'],rows)
rows=[]
for label,rs in grouped('concurrent'):
 rows.append([label,vals(rs,lambda a:f(a['initial_indexed_ingest_us']/1e6)),vals(rs,lambda a:ms(a['writer_service']['p50_us'])),vals(rs,lambda a:ms(a['writer_service']['p95_us'])),vals(rs,lambda a:ms(a['writer_service']['p99_us'])),vals(rs,lambda a:ms(a['writer_scheduled']['p99_us'])),vals(rs,lambda a:','.join(ms(r['p99_us'])for r in a['reader_samples'])),vals(rs,lambda a:f(a['cpu_s'])),vals(rs,lambda a:mib(a['wal_file_bytes']))])
table('Concurrent SQLite Search: 100k docs, 1000 tx, 10% indexed updates, rollback every 33rd',['Scenario','Indexed ingest s','Write p50 ms','p95 ms','p99 ms','Scheduled write p99 ms','Reader p99 ms each','CPU s','WAL MiB'],rows)
rows=[]
for label,rs in grouped('owned-store'):
 rows.append([label,vals(rs,lambda a:f(a['bulk_import_us']/1e6)),vals(rs,lambda a:ms(a['query_top10_hydrated']['p50_us'])),vals(rs,lambda a:ms(a['query_top10_hydrated']['p95_us'])),vals(rs,lambda a:ms(a['query_top10_hydrated']['p99_us'])),vals(rs,lambda a:f(a['query_per_s'])),vals(rs,lambda a:mib(a['initial_backend_bytes'])),vals(rs,lambda a:mib(a['initial_wal_peak_bytes'])),vals(rs,lambda a:f(a['build_memory']['rss_kib']/1024)),vals(rs,lambda a:mib(a['final_backend_bytes']))])
table('Authoritative standalone collections: actual stored fields hydrated, 60 queries',['Scenario','Import s','Query p50 ms','p95 ms','p99 ms','Query/s','Backend MiB','WAL MiB','Build RSS MiB','After churn MiB'],rows)
individual=[];batched=[]
for label,rs in grouped('owned-store'):
 for i,phase in enumerate(rs[0]['result']['mutation_phases']):
  rate=vals(rs,lambda a,i=i:f(a['mutation_phases'][i]['documents']*1e6/a['mutation_phases'][i]['commit_and_visibility']['sum_us']))
  if phase['batch']==1:
   individual.append([label,phase['operation'],vals(rs,lambda a,i=i:ms(a['mutation_phases'][i]['commit_and_visibility']['p50_us'])),vals(rs,lambda a,i=i:ms(a['mutation_phases'][i]['commit_and_visibility']['p95_us'])),vals(rs,lambda a,i=i:ms(a['mutation_phases'][i]['commit_and_visibility']['p99_us'])),rate])
  else:batched.append([label,phase['operation'],vals(rs,lambda a,i=i:ms(a['mutation_phases'][i]['commit_and_visibility']['sum_us'])),rate])
table('Owned-store individual commits: 100 operations/pass, commit and visibility included',['Scenario','Operation','p50 ms','p95 ms','p99 ms','Observed Doc/s'],individual)
table('One 100-document commit/pass: observed batch time, not latency quantiles',['Scenario','Operation','Complete commit ms','Amortized Doc/s'],batched)
lines.extend(['The owned-store query sample is the first 60 natural RU queries over mixed RU/EN/ZH documents; it is not a balanced multilingual latency distribution. Mutations change the first 100 RU documents. Bulk import is one durable commit through upsert; incremental batch=1 and batch=100 are distinct workloads. Batch=100 has one sample/pass: its amortized rate is not steady-state throughput. SQLite checkpoints occur outside operation latency; Tantivy uses synchronous commit/reload and its normal background merge behavior. These timings do not establish TinyStore grouped-commit integration or cross-engine transactions.',''])
table('Raw inventory',['File','Records','Nonzero process exits'],[[p.name,len(rs:=read(p.stem)),sum(r['exit']!=0 for r in rs)]for p in sorted(DATA.glob('*.jsonl'))])
OUT.write_text('\n'.join(lines)+'\n');print(OUT)
