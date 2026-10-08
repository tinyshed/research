#!/usr/bin/env python3
"""Verify and measure real metrics pipelines on fixed identical operation traces."""
import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/metrics-native-2026-10-08'
EPOCH=1700000000000
NORMAL_NOW=EPOCH+6000
IMPLEMENTATIONS=['go_metrics','rust_metrics']
WRITE_CASES={
 'ingest_append1':'head','ingest_replace1':'head','ingest_scrape100':'scrape',
 'ingest_register8':'empty','ingest_shuffled240':'head','ingest_duplicates240':'head',
 'maintain_ready':'ready','expire_all':'sealed',
}
READ_CASES=['read_head_point','read_head_full8','read_sealed_point','read_sealed_boundary','read_sealed_full8','read_sealed_full16','read_filtered_one','stream_sealed_full8','read_where','read_prefix','read_noneof','read_since']
AGGREGATE_CASES=['aggregate_'+op for op in ['count','sum','avg','min','max','delta','increase','rate','cut_sum','cut_avg','grouped_sum','grouped_increase']]
CASES=list(WRITE_CASES)+READ_CASES+AGGREGATE_CASES
EDGE_CASES=['read_edge']+['aggregate_edge_'+op+'_'+str(i) for op,i in [('sum',0),('sum',1),('sum',3),('sum',4),('sum',5),('sum',6),('sum',8),('sum',9),('sum',11),('avg',6),('avg',8),('avg',9),('min',5),('max',5),('increase',10),('rate',10)]]

def call(args,**kwargs):
 result=subprocess.run(args,check=False,text=True,capture_output=True,**kwargs)
 if result.returncode:
  raise RuntimeError(f"Command failed ({result.returncode}): {args}\n{result.stdout}\n{result.stderr}")
 return result.stdout

def source_hashes():
 paths=[ROOT/'run.py',ROOT/'rust/build.rs',ROOT/'rust/Cargo.toml',ROOT/'rust/Cargo.lock',ROOT/'go/go.mod',ROOT/'go/go.sum']
 paths+=sorted((ROOT/'go').glob('*.go'))+sorted((ROOT/'rust/src').rglob('*.rs'))
 return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}

def binaries():
 return {'go_metrics':ROOT/'bin/go-metrics-bench','rust_metrics':ROOT/'rust/target/release/tinystore-metrics-native-bench'}

def build(env):
 (ROOT/'bin').mkdir(exist_ok=True)
 call(['go','build','-trimpath','-o',str(binaries()['go_metrics']),'.'],cwd=ROOT/'go',env=env)
 generated=ROOT.parent/'sqlite-bench/generated'
 if not (generated/'lib/libsqlite3.a').exists():
  call(['python3',str(ROOT.parent/'sqlite-bench/build_native.py')],env=env)
 native_env=env.copy();native_env.update(SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(generated/'lib'),SQLITE3_INCLUDE_DIR=str(generated/'sqlite-amalgamation-3530400'))
 call(['cargo','build','--release','--locked'],cwd=ROOT/'rust',env=native_env)

def fixture_for(case):
 return WRITE_CASES.get(case,'head' if case.startswith('read_head') else 'edge' if case in EDGE_CASES else 'sealed')

def run_one(implementation,case,mode,env,cpu,iterations=1,warm=0,extra=(),capture_storage=False):
 fixture=ROOT/'stores'/fixture_for(case)/'metrics.db'
 with tempfile.TemporaryDirectory(prefix='run-',dir=ROOT/'stores') as directory:
  database=Path(directory)/'metrics.db';shutil.copyfile(fixture,database)
  args=[str(binaries()[implementation]),'--db',str(database),'--case',case,'--mode',mode,'--iterations',str(iterations),'--warm',str(warm)]
  if case=='expire_all':args+=['--now',str(NORMAL_NOW+30*24*60*60*1000+5000)]
  args+=list(extra)
  timefile=Path(directory)/'resource.txt'
  timed=mode=='memory'
  if timed:
   timebin=env.get('TINYSTORE_TIME') or shutil.which('time',path=env.get('PATH'))
   assert timebin,'GNU time is required for peak process RSS'
   args=[timebin,'-f','%M', '-o',str(timefile),*args]
  result=json.loads(call(['taskset','-c',str(cpu),*args],env=env))
  if timed:result['max_rss_kib']=int(timefile.read_text().strip())
  if capture_storage:
   result['production_go_crossread']=json.loads(call([str(binaries()['go_metrics']),'--db',str(database),'--mode','digest'],env=env))['final_digest']
   result['physical']=json.loads(call([str(binaries()['go_metrics']),'--db',str(database),'--mode','physical'],env=env))
  return result

def normalized(result):
 return {k:v for k,v in result.items() if k not in ['implementation','physical']}

def verify(env,cpu):
 verification=[];plans=[]
 for case in CASES+EDGE_CASES:
  count=1 if case in ['maintain_ready','expire_all'] or case in EDGE_CASES else 4
  print('Verify '+case,flush=True)
  outputs=[run_one(impl,case,'verify',env,cpu,count,capture_storage=case in WRITE_CASES) for impl in IMPLEMENTATIONS]
  assert normalized(outputs[0])==normalized(outputs[1]),(case,outputs)
  verification.append({'case':case,'fixture':fixture_for(case),'result':normalized(outputs[0]),'physical':{impl:r.get('physical') for impl,r in zip(IMPLEMENTATIONS,outputs)} if case in WRITE_CASES else None})
  if case in READ_CASES and case!='stream_sealed_full8' or case in AGGREGATE_CASES:
   outputs=[run_one(impl,case,'plan',env,cpu) for impl in IMPLEMENTATIONS]
   assert outputs[0]==outputs[1],('plan',case,outputs)
   plans.append({'case':case,**outputs[0]})
 guards={}
 for mode,case in [('guards','read_sealed_full8'),('edge-guards','read_edge')]:
  outputs=[run_one(impl,case,mode,env,cpu) for impl in IMPLEMENTATIONS]
  assert outputs[0]==outputs[1],(mode,outputs)
  assert all(v for k,v in outputs[0].items() if k!='final_digest'),outputs
  guards[mode]=outputs[0]
 DATA.mkdir(parents=True,exist_ok=True)
 (DATA/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
 (DATA/'plans.json').write_text(json.dumps(plans,indent=2)+'\n')
 (DATA/'guards.json').write_text(json.dumps(guards,indent=2)+'\n')
 return verification

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--skip-build',action='store_true');parser.add_argument('--verify-only',action='store_true');parser.add_argument('--passes',type=int,default=5);args=parser.parse_args()
 env=os.environ.copy();env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1');env.pop('GOGC',None)
 if not args.skip_build:build(env)
 (ROOT/'stores').mkdir(exist_ok=True);DATA.mkdir(parents=True,exist_ok=True)
 cpu=min(os.sched_getaffinity(0))
 for kind in ['sealed','ready','head','scrape','empty','edge']:
  db=ROOT/'stores'/kind/'metrics.db'
  if not db.exists():db.parent.mkdir(exist_ok=True);call([str(binaries()['go_metrics']),'--db',str(db),'--mode','fixture','--fixture',kind],env=env)
 sources=source_hashes();binary_hashes={k:hashlib.sha256(p.read_bytes()).hexdigest() for k,p in binaries().items()}
 verify(env,cpu)
 assert sources==source_hashes(),'source changed during verification'
 if args.verify_only:print('Verified same sample/aggregate bits, plans, guards and Go cross-reader compatibility',flush=True);return
 metadata={'started_utc':dt.datetime.now(dt.timezone.utc).isoformat(),'source_sha256':sources,'binary_sha256':binary_hashes,'tinystore_commit':call(['git','rev-parse','HEAD'],cwd=ROOT.parent/'source').strip(),'go':call(['go','version'],env=env).strip(),'rustc':call(['rustc','--version'],env=env).strip(),'platform':platform.platform(),'cpu':call(['lscpu']).strip(),'cpu_affinity':cpu,'cpu_quota':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'memory_limit':Path('/sys/fs/cgroup/memory.max').read_text().strip(),'filesystem':call(['stat','-f','-c','%T',str(ROOT/'stores')]).strip(),'gomaxprocs':1,'gogc':'default100','passes':args.passes,'harness_status':'uncommitted local prototype; source/binary hashes recorded','native_build':json.loads((ROOT.parent/'sqlite-bench/generated/native-build.json').read_text()),'fixtures':{kind:{'bytes':(ROOT/'stores'/kind/'metrics.db').stat().st_size,'sha256':hashlib.sha256((ROOT/'stores'/kind/'metrics.db').read_bytes()).hexdigest()} for kind in ['sealed','ready','head','scrape','empty','edge']}}
 inspections={}
 for kind in ['sealed','head','scrape','ready','edge']:
  case=next(c for c in CASES+EDGE_CASES if fixture_for(c)==kind)
  inspections[kind]={'go':run_one('go_metrics',case,'physical',env,cpu),'native':run_one('rust_metrics',case,'inspect',env,cpu)}
  expected={'page_size':4096,'synchronous':2,'fullfsync':1,'checkpoint_fullfsync':1,'foreign_keys':1,'busy_timeout':5000,'cache_size':-1024,'trusted_schema':0,'wal_autocheckpoint':1000,'mmap_size':0,'version':'3.53.4'}
  for key,value in expected.items():
   assert inspections[kind]['go']['settings'][key]==value
   for role in ['reader','writer']:assert inspections[kind]['native'][role][key]==value
  assert inspections[kind]['native']['reader']['source_id']==inspections[kind]['go']['settings']['source_id']
 (DATA/'inspect.json').write_text(json.dumps(inspections,indent=2)+'\n')
 # Estimate workload size on fresh Go copies, then FIX the same n and warm trace
 # for both implementations in every retained pass. No mutable calibration state survives.
 counts={};pilots=[]
 for case in CASES:
  if case in ['maintain_ready','expire_all']:counts[case]=1;continue
  count=8 if case in WRITE_CASES else 16
  pilot=run_one('go_metrics',case,'bench',env,cpu,count,32);pilots.append(pilot)
  cap=16 if case=='ingest_register8' else 512 if case=='ingest_scrape100' else 4096 if case in WRITE_CASES else 16384
  n=max(1,min(cap,math.ceil(0.15e9/pilot['ns_per_op'])))
  counts[case]=n
 metadata['fixed_iterations']=counts;metadata['warm_operations']=32;metadata['one_shot_cases']=['maintain_ready','expire_all']
 (DATA/'pilot.json').write_text(json.dumps(pilots,indent=2)+'\n')
 (DATA/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
 timings=[]
 with (DATA/'timings.jsonl').open('w') as raw:
  for p in range(args.passes):
   print(f'Fixed-trace timing pass {p+1}/{args.passes}',flush=True)
   for case in CASES:
    # Each case has adjacent paired stacks, with order alternated across pass/case.
    order=IMPLEMENTATIONS if (p+CASES.index(case))%2==0 else IMPLEMENTATIONS[::-1]
    final_digests=[]
    for implementation in order:
     row=run_one(implementation,case,'bench',env,cpu,counts[case],0 if case in ['maintain_ready','expire_all'] else 32,capture_storage=case in WRITE_CASES)
     if case in WRITE_CASES:final_digests.append(row['production_go_crossread'])
     row.update(pass_id=p+1,fixture=fixture_for(case),utc=dt.datetime.now(dt.timezone.utc).isoformat());timings.append(row);raw.write(json.dumps(row)+'\n');raw.flush()
    if final_digests:assert final_digests[0]==final_digests[1],('timed write trace final digest',case,final_digests)
 summaries=[]
 for case in CASES:
  samples={impl:[r['ns_per_op'] for r in timings if r['case']==case and r['implementation']==impl] for impl in IMPLEMENTATIONS}
  medians={impl:statistics.median(values) for impl,values in samples.items()}
  summaries.append({'case':case,'fixture':fixture_for(case),'iterations':counts[case],'median_ns':medians,'passes_ns':samples,'go_over_native':medians['go_metrics']/medians['rust_metrics']})
 (DATA/'summary.json').write_text(json.dumps(summaries,indent=2)+'\n')
 memory=[]
 for case in ['read_head_full8','read_sealed_full8','read_sealed_full16','stream_sealed_full8','aggregate_cut_sum','aggregate_sum']:
  for p in range(3):
   for impl in IMPLEMENTATIONS if p%2==0 else IMPLEMENTATIONS[::-1]:
    row=run_one(impl,case,'memory',env,cpu,16,0);row['pass_id']=p+1;memory.append(row)
 with (DATA/'memory.jsonl').open('w') as raw:
  for row in memory:raw.write(json.dumps(row)+'\n')
 assert source_hashes()==sources,'sources changed during measurements'
 assert {k:hashlib.sha256(p.read_bytes()).hexdigest() for k,p in binaries().items()}==binary_hashes,'binaries changed during measurements'
 metadata['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat();(DATA/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
 print(f'Complete: {len(timings)} paired fixed-trace timings, {len(memory)} memory processes',flush=True)

if __name__=='__main__':main()
