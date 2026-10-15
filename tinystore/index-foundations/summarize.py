#!/usr/bin/env python3
"""Read-only reduction. Keep passes separate; raw files remain authoritative."""
from pathlib import Path
import collections
import json
import math

ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/index-foundations-2026-10-15'
OUT=ROOT.parent/'reports/index-foundations-measurements-2026-10-15.md'
lines=['# Follow-up measurement tables — 15 October 2026','','Generated with `python3 tinystore/index-foundations/summarize.py`. Values separated by `;` are pass 1; pass 2, never an average. Units and workload groups must be retained when quoting a number. See the architecture reports for boundaries, interpretation and acceptance gates. Raw JSONL contains complete commands, revisions, UTC timestamps, counters and samples.','']
def read(name):
 p=DATA/(name+'.jsonl')
 return [json.loads(s) for s in p.read_text().splitlines() if s.strip()] if p.exists() else []
def groups(name):
 d=collections.defaultdict(list)
 for r in read(name):d[r['label']].append(r)
 return [(k,sorted(v,key=lambda r:r['pass_number'])) for k,v in d.items()]
def f(x,d=2):return f'{x:.{d}f}'
def mib(x):return f(x/1024**2)
def ms(x):return f(x/1000)
def values(rs,fn):return '; '.join(str(fn(r['result'])) for r in rs)
def table(title,headers,rows):
 lines.extend(['## '+title,'','| '+' | '.join(headers)+' |','|'+'|'.join('---' for _ in headers)+'|'])
 for row in rows:lines.append('| '+' | '.join(map(str,row))+' |')
 lines.append('')
def indexbytes(r):
 if 'index_bytes'in r:return r['index_bytes']
 if 'serialized_bitmap_bytes'in r:return r['serialized_bitmap_bytes']+r['score_array_bytes']
 return sum(o['bytes'] for o in r.get('build_objects',[]) if o['object'].startswith('g0') or o['object'] in ('secondary','postings','grid'))
def mem(r):
 for k in ('final_memory','workload_memory','memory','resident_memory'):
  if k in r:return r[k].get('hwm_kib',0)/1024
 return 0
rows=[]
for label,rs in groups('static'):
 rows.append([label,values(rs,lambda r:f(r['build_us']/1e6)),values(rs,lambda r:ms(r['queries']['p50_us'])),values(rs,lambda r:ms(r['queries']['p95_us'])),values(rs,lambda r:ms(r['queries']['p99_us'])),values(rs,lambda r:f(r['query_per_s'])),values(rs,lambda r:mib(indexbytes(r))),values(rs,lambda r:f(mem(r))),values(rs,lambda r:mib(r.get('build_wal',{}).get('frame_bytes',0)))])
table('Static build and full result-set queries',['Scenario','Build s','Query p50 ms','p95 ms','p99 ms','Query/s','Index MiB¹','HWM MiB','Build WAL MiB'],rows)
lines.extend(['¹ Cache size for Roaring is serialized bitmaps plus score-array estimate, not durable disk. rstar has no standalone durable index bytes; its RAM is in HWM. Index objects exclude source. Native incremental work is excluded from build time, but process HWM spans the entire process. Scans create no secondary projection.',''])
rows=[]
for label,rs in groups('static'):
 if 'mutation_phases'not in rs[0]['result']:continue
 for i,phase in enumerate(rs[0]['result']['mutation_phases']):
  rows.append([label,phase['operation'],phase['rows'],values(rs,lambda r,i=i:ms(r['mutation_phases'][i]['batches']['p99_us'])),values(rs,lambda r,i=i:f(r['mutation_phases'][i]['wal_frames']*4120/r['mutation_phases'][i]['rows'],1))])
table('Native incremental transactions (100 rows/transaction)',['Scenario','Operation','Rows','Batch p99 ms','Physical WAL bytes/row'],rows)
rows=[]
for label,rs in groups('static'):
 if not any(k in label for k in ('-geo','-grid','-rstar','scan-geo')):continue
 for bucket in range(4):
  def quant(r):
   a=sorted(s['us'] for s in r['query_samples'] if s['bucket']==bucket)
   return a[min(len(a)-1,math.ceil(.99*len(a))-1)]
  rows.append([label,bucket,values(rs,lambda r:next(s['matched'] for s in r['query_samples'] if s['bucket']==bucket)),values(rs,lambda r:ms(quant(r)))])
table('Geo selectivity buckets (75 samples/bucket/pass)',['Scenario','Bucket','Matches','Query p99 ms'],rows)
for name in ('rebuilds','sensitivity'):
 rebuilds=[(k,v) for k,v in groups(name) if v[0]['result']['experiment']=='rebuild']
 if not rebuilds:continue
 rows=[]
 for label,rs in rebuilds:
  rows.append([label,values(rs,lambda r:f(r['rebuild_us']/1e6)),values(rs,lambda r:ms(r['foreground']['writer_service']['p50_us'])),values(rs,lambda r:ms(r['foreground']['writer_service']['p95_us'])),values(rs,lambda r:ms(r['foreground']['writer_service']['p99_us'])),values(rs,lambda r:ms(r['foreground']['writer_queue']['p99_us'])),values(rs,lambda r:ms(r['foreground']['scheduled_latency']['p99_us'])),values(rs,lambda r:ms(max(r['builder_holds']['max_us'],r['builder_setup_us'],r['activation_hold_us'],r['builder_checkpoint_holds']['max_us'])))])
 table(name+': writer and build',['Scenario','Rebuild s','Write service p50 ms','p95','p99','Writer queue p99 ms','Scheduled p99 ms','Max builder hold ms²'],rows)
 rows=[]
 for label,rs in rebuilds:
  rows.append([label,values(rs,lambda r:f(r['query_per_s'])),values(rs,lambda r:'/'.join(ms(a['latency']['p99_us']) for a in r['readers'])),values(rs,lambda r:mib(r['workload_wal']['frame_bytes'])),values(rs,lambda r:mib(r['workload_wal']['peak_logical_bytes'])),values(rs,lambda r:mib(r['peak_db_plus_wal_bytes'])),values(rs,lambda r:f(r['sampled_rss_peak_kib']/1024)),values(rs,lambda r:ms(r['activation_hold_us'])),values(rs,lambda r:str(r['time_budget_overruns'])+'/'+str(r['frame_budget_overruns']))])
 table(name+': reads and resources',['Scenario','Query/s','Readers p99 ms','Total WAL MiB','Peak logical WAL MiB','Peak DB+WAL MiB','Peak RSS MiB','Activation ms','Time/frame overruns'],rows)
lines.extend(['² Maximum across setup, chunk/monolithic build, checkpoint and activation holds. Foreground service excludes queue time; scheduled latency includes offered-workload backlog. Fixed main scenarios perform the same minimum 2000 transactions; long builds can outlast that write window. `continuous-*` keeps writes through publication. Different tx sizes, update ratios, caches and distributions are separate scenarios.',''])
rows=[]
for label,rs in groups('locks'):
 rows.append([label,values(rs,lambda r:f(r['grants_per_s'])),values(rs,lambda r:f(r['attempts_per_grant'])),values(rs,lambda r:ms(r['wait']['p99_us'])),values(rs,lambda r:f(r['jain_fairness'],3)),values(rs,lambda r:r['deadline_canceled']),values(rs,lambda r:f(r['cpu_s'])),values(rs,lambda r:f(mem(r)))])
table('Actual Core lease admission',['Scenario','Grant/s','Attempts/grant','Completed wait p99 ms','Jain fairness','Deadline cancels','CPU s','HWM MiB'],rows)
rows=[]
for label,rs in groups('channels'):
 rows.append([label,values(rs,lambda r:f(r['publishedPerS'])),values(rs,lambda r:r['delivered']),values(rs,lambda r:r['lost']),values(rs,lambda r:r['gapMissed']),values(rs,lambda r:f(r['latency']['p50Us']/1000)),values(rs,lambda r:f(r['latency']['p99Us']/1000)),values(rs,lambda r:mib(r['subscribedMemory']['rss']-r['idleMemory']['rss'])),values(rs,lambda r:mib(r['endMemory']['rss'])),values(rs,lambda r:f((r['cpuUs']['user']+r['cpuUs']['system'])/1e6))])
table('Real Bun receivers',['Scenario','Publish/s','Delivered','Undelivered at drain','Gap missed','p50 ms','p99 ms','Client subscription delta MiB','Client end RSS MiB','Client CPU s'],rows)
rows=[]
for label,rs in groups('outbox'):
 if rs[0]['result'].get('experiment')!='outbox':continue
 rows.append([label,values(rs,lambda r:f(r['build_us']/1e6)),values(rs,lambda r:ms(r['build_foreground']['service']['p99_us'])),values(rs,lambda r:ms(r['steady_foreground']['service']['p99_us'])),values(rs,lambda r:ms(r['query_service']['p50_us'])),values(rs,lambda r:ms(r['query_service']['p99_us'])),values(rs,lambda r:ms(r['query_scheduled']['p99_us'])),values(rs,lambda r:mib(r['wal']['frame_bytes'])),values(rs,lambda r:mib(r['external_bytes'])),values(rs,lambda r:f(mem(r)))])
table('SQLite source with native term index or durable Tantivy outbox',['Scenario','Build s','Build write p99 ms','Steady write p99 ms','Query p50 ms','Query p99 ms','Scheduled query p99 ms','WAL MiB','External files MiB','HWM MiB'],rows)
rows=[]
for label,rs in groups('sensitivity'):
 if rs[0]['result']['experiment']!='build-only':continue
 rows.append([label,values(rs,lambda r:f(r['build_us_including_diagnostic_observer']/1e6)),values(rs,lambda r:r['build_wal']['frames']),values(rs,lambda r:r['build_wal']['distinct_pages_per_commit_sum']),values(rs,lambda r:mib(r['build_wal']['frame_bytes'])),values(rs,lambda r:mib(r['build_wal']['peak_logical_bytes'])),values(rs,lambda r:mib(r['sampled_temp_file_peak_bytes']))])
table('Instrumented WAL/cache/order sensitivity',['Scenario','Build+observer s','WAL frames','Sum unique pages/commit','Total WAL MiB','Peak WAL MiB','Sampled temp MiB'],rows)
rows=[]
for label,rs in groups('geo-certified'):
 for bucket in range(4):
  rows.append([label,bucket,values(rs,lambda r:next(s['matched'] for s in r['samples'] if s['bucket']==bucket)),values(rs,lambda r:ms(sorted(s['plain_us'] for s in r['samples'] if s['bucket']==bucket)[3])),values(rs,lambda r:ms(sorted(s['certified_us'] for s in r['samples'] if s['bucket']==bucket)[3]))])
table('Certified geo same-connection diagnostic (8/bucket, lower median)',['Scenario','Bucket','Matches','Plain ms','Certified ms'],rows)
rows=[]
for label,rs in groups('large-crashes'):
 rows.append([label,values(rs,lambda r:ms(r['recovery_us'])),values(rs,lambda r:mib(r['wal_after_resume_bytes'])),values(rs,lambda r:r['status'])])
table('Large process-crash recovery (oracle validation excluded from resume time)',['Scenario','Resume ms','WAL after resume MiB','Oracle'],rows)
rows=[]
for label,rs in groups('memory'):
 rows.append([label,'; '.join(str(r['exit']) for r in rs),'; '.join(str(r['oom_killed']) for r in rs),'; '.join(str(r['recovery']['rows']) for r in rs)])
table('64 MiB cgroup / no swap correctness',['Scenario','Exit','OOM killed','Source rows after reopen'],rows)
lines.extend(['## Raw inventory','','| File | Records | Failed process exits | Revisions |','|---|---|---|---|'])
for p in sorted(DATA.glob('*.jsonl')):
 rs=read(p.stem);lines.append('| '+p.name+' | '+str(len(rs))+' | '+str(sum(r.get('exit',0)!=0 for r in rs))+' | '+', '.join(sorted({r.get('revision','unrecorded')[:12] for r in rs}))+' |')
lines.append('')
OUT.write_text('\n'.join(lines))
print(OUT)
