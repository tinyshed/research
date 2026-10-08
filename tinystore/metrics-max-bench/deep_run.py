#!/usr/bin/env python3
"""Fixed-trace deep optimizations, independent ablations and compiler variants."""
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
import run as previous
from build import environment

ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/metrics-deep-2026-10-08'
WIDE=['read_wide64','stream_wide64']+['aggregate_wide_'+s for s in ['sum','cut_sum','cut_avg','cut_count','grouped_cut_sum']]
CASES=previous.CASES+WIDE+['read_edge','aggregate_edge_sum_3','aggregate_edge_increase_10']
BASE=['--fast-exact','1','--fast-codec','1']
def flags(mask,threads=0):return [*BASE,'--threads',str(threads),'--tuning',str(mask)]
VARIANTS={
 'go':('bin/go-reference',[]),
 'rust_previous':('bin/rust-reference',BASE+['--threads','0']),
 'rust_control':('bin/rust-default',flags(0)),
 'rust_new':('bin/rust-default',flags(15)),
 'rayon2':('bin/rust-default',flags(15,2)),
 'rust_lto':('bin/rust-lto',flags(15)),
 'rust_native':('bin/rust-native',flags(15)),
 'rust_pgo':('bin/rust-pgo',flags(15)),
 'pgo_rayon2':('bin/rust-pgo',flags(15,2)),
 'only_bits':('bin/rust-default',flags(1)),
 'only_buffers':('bin/rust-default',flags(2)),
 'only_fused':('bin/rust-default',flags(4)),
 'only_specialized':('bin/rust-default',flags(8)),
}
MAIN=list(VARIANTS)[:9]
ABLATIONS=list(VARIANTS)[9:]
ABLATION_CASES=['ingest_scrape100','maintain_ready','read_head_full8','read_sealed_full16','read_wide64','aggregate_sum','aggregate_cut_sum','aggregate_wide_cut_sum','aggregate_wide_grouped_cut_sum']
PROFILE_CASES=['read_head_point','read_head_full8','read_sealed_full8','read_sealed_full16','read_wide64','stream_wide64','aggregate_sum','aggregate_count','aggregate_cut_sum','aggregate_wide_cut_sum','read_edge']

def call(args,**kwargs):
 r=subprocess.run(args,text=True,capture_output=True,**kwargs)
 if r.returncode:raise RuntimeError(f'{args}: {r.returncode}\n{r.stdout}\n{r.stderr}')
 return r.stdout

def fixture_for(case):return 'wide' if case in WIDE else previous.fixture_for(case)
def select_cpus():
 allowed=sorted(os.sched_getaffinity(0));first=allowed[0]
 def cache(cpu):return Path(f'/sys/devices/system/cpu/cpu{cpu}/cache/index0/shared_cpu_list').read_text().strip()
 second=next((c for c in allowed[1:] if cache(c)!=cache(first)),allowed[1])
 return [first,second]

def run_one(variant,case,mode,env,cpus,iterations=1,warm=0,force=False,override=None):
 binary,args_flags=override or VARIANTS[variant]
 with tempfile.TemporaryDirectory(prefix='deep-',dir=ROOT/'stores') as d:
  db=Path(d)/'metrics.db';shutil.copyfile(ROOT/'stores'/fixture_for(case)/'metrics.db',db)
  args=[str(ROOT/binary),'--db',str(db),'--case',case,'--mode',mode,'--iterations',str(iterations),'--warm',str(warm),*args_flags]
  if case=='expire_all':args+=['--now',str(previous.NORMAL_NOW+30*24*60*60*1000+5000)]
  if force and variant in ['rayon2','pgo_rayon2']:args+=['--force-parallel','1']
  resource=Path(d)/'rss'
  if mode=='memory':args=[shutil.which('time',path=env['PATH']),'-f','%M','-o',str(resource),*args]
  affinity=','.join(map(str,cpus if variant in ['rayon2','pgo_rayon2'] else cpus[:1]))
  row=json.loads(call(['taskset','-c',affinity,*args],env=env))
  if mode=='memory':row['max_rss_kib']=int(resource.read_text())
  if case in previous.WRITE_CASES and mode in ['bench','verify']:
   row['go_crossread']=json.loads(call([str(ROOT/'bin/go-reference'),'--db',str(db),'--mode','digest'],env=env))['final_digest']
  return row

def sources():
 paths=[ROOT/name for name in ['deep_run.py','build.py','run.py']]
 for folder in ['go','rust','reference-rust']:
  paths += [p for p in (ROOT/folder).rglob('*') if p.is_file() and 'target' not in p.parts and (p.suffix in ['.rs','.go'] or p.name in ['Cargo.toml','Cargo.lock','go.mod','go.sum'])]
 return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}
def binary_hashes():return {binary:hashlib.sha256((ROOT/binary).read_bytes()).hexdigest() for binary,_ in VARIANTS.values()}

def verify(env,cpus):
 rows=[]
 for case in dict.fromkeys(CASES+previous.EDGE_CASES):
  n=1 if case in ['maintain_ready','expire_all'] or case in previous.EDGE_CASES else 4
  outputs={v:run_one(v,case,'verify',env,cpus,n,force=True) for v in VARIANTS}
  norm=lambda r:{k:v for k,v in r.items() if k!='implementation'}
  expected=norm(outputs['go']);assert all(norm(x)==expected for x in outputs.values()),(case,outputs)
  rows.append({'case':case,'forced_parallel':True,'result':expected})
  print('Verified '+case,flush=True)
 guards={}
 for mode,case in [('guards','read_sealed_full8'),('edge-guards','read_edge')]:
  outputs={v:run_one(v,case,mode,env,cpus,force=True) for v in VARIANTS}
  assert all(x==outputs['go'] for x in outputs.values()),(mode,outputs)
  assert all(v for k,v in outputs['go'].items() if k!='final_digest');guards[mode]=outputs
 plans=[]
 for case in previous.READ_CASES+previous.AGGREGATE_CASES+WIDE:
  if case.startswith('stream_'):continue
  outputs={v:run_one(v,case,'plan',env,cpus) for v in MAIN}
  assert all(x==outputs['go'] for x in outputs.values()),('plans',case,outputs)
  plans.append({'case':case,**outputs['go']})
 for name,data in [('verification',rows),('guards',guards),('plans',plans)]:
  (DATA/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')

def profiles(env,cpus):
 rows=[]
 for case in PROFILE_CASES:
  for pass_id in range(3):
   for mask in [0,15] if pass_id%2==0 else [15,0]:
    row=run_one('rust_control',case,'profile',env,cpus,32,32,override=('bin/rust-telemetry',flags(mask)))
    row.update(mask=mask,pass_id=pass_id+1);rows.append(row)
 (DATA/'profiles.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in rows))

def main():
 p=argparse.ArgumentParser();p.add_argument('--verify-only',action='store_true');p.add_argument('--profiles-only',action='store_true');p.add_argument('--skip-verify',action='store_true');p.add_argument('--passes',type=int,default=5);a=p.parse_args()
 DATA.mkdir(parents=True,exist_ok=True);env=environment();cpus=select_cpus()
 hashes=sources();binaries=binary_hashes()
 if not a.skip_verify:
  verify(env,cpus)
  (DATA/'verified-hashes.json').write_text(json.dumps({'source':hashes,'binary':binaries},indent=2)+'\n')
 else:assert json.loads((DATA/'verified-hashes.json').read_text())=={'source':hashes,'binary':binaries},'skip-verify requires unchanged verified sources/binaries'
 assert sources()==hashes
 if a.verify_only:return
 profiles(env,cpus)
 if a.profiles_only:return
 counts={};pilot=[]
 for case in CASES:
  if case in ['maintain_ready','expire_all']:counts[case]=1;continue
  row=run_one('rust_previous',case,'bench',env,cpus,16,32);pilot.append(row)
  cap=16 if case=='ingest_register8' else 512 if case=='ingest_scrape100' else 4096 if case in previous.WRITE_CASES else 16384
  counts[case]=max(1,min(cap,math.ceil(0.12e9/row['ns_per_op'])))
 metadata={'started_utc':dt.datetime.now(dt.timezone.utc).isoformat(),'tinystore_commit':call(['git','rev-parse','HEAD'],cwd=ROOT.parent/'source').strip(),'source_sha256':hashes,'binary_sha256':binaries,'go':call(['go','version'],env=env).strip(),'rustc':call(['rustc','--version','--verbose'],env=env).strip(),'platform':platform.platform(),'cpu':call(['lscpu']).strip(),'cpu_quota':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'memory_limit':Path('/sys/fs/cgroup/memory.max').read_text().strip(),'cpus':cpus,'variants':VARIANTS,'gomaxprocs':1,'warm':32,'iterations':counts,'passes':a.passes,'harness_status':'committed prototype; local VM/container measurements with source/binary hashes' if (ROOT/'measured-commit.txt').exists() else 'uncommitted local prototype','harness_commit':(ROOT/'measured-commit.txt').read_text().strip() if (ROOT/'measured-commit.txt').exists() else 'uncommitted local prototype','baseline_note':'Previous optimized Rust port remeasured in this session; control is new source with all deep switches off','build':json.loads((ROOT/'build-metadata.json').read_text()),'fixtures':{kind:{'sha256':hashlib.sha256((ROOT/'stores'/kind/'metrics.db').read_bytes()).hexdigest(),'bytes':(ROOT/'stores'/kind/'metrics.db').stat().st_size} for kind in ['sealed','ready','head','scrape','empty','edge','wide']}}
 (DATA/'pilot.json').write_text(json.dumps(pilot,indent=2)+'\n');(DATA/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
 rows=[]
 with (DATA/'timings.jsonl').open('w') as f:
  for pass_id in range(a.passes):
   print(f'Quiet timing pass {pass_id+1}/{a.passes}',flush=True)
   for index,case in enumerate(CASES):
    variants=MAIN+(ABLATIONS if case in ABLATION_CASES else [])
    shift=(pass_id+index)%len(variants);order=variants[shift:]+variants[:shift]
    if pass_id%2:order=order[::-1]
    digests=[]
    for variant in order:
     row=run_one(variant,case,'bench',env,cpus,counts[case],0 if case in ['maintain_ready','expire_all'] else 32)
     row.update(variant=variant,pass_id=pass_id+1,fixture=fixture_for(case),utc=dt.datetime.now(dt.timezone.utc).isoformat())
     rows.append(row);f.write(json.dumps(row)+'\n');f.flush()
     if 'go_crossread' in row:digests.append(row['go_crossread'])
    assert len(set(digests))<=1,(case,digests)
 summary=[]
 for case in CASES:
  values={v:[r['ns_per_op'] for r in rows if r['case']==case and r['variant']==v] for v in VARIANTS}
  values={k:v for k,v in values.items() if v};medians={v:statistics.median(xs) for v,xs in values.items()}
  summary.append({'case':case,'iterations':counts[case],'median_ns':medians,'passes_ns':values,'new_vs_previous':medians['rust_previous']/medians['rust_new'],'pgo_vs_previous':medians['rust_previous']/medians['rust_pgo'],'go_over_pgo':medians['go']/medians['rust_pgo']})
 (DATA/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
 memory=[]
 for case in ['read_head_full8','read_sealed_full16','read_wide64','stream_wide64','aggregate_cut_sum','aggregate_wide_cut_sum']:
  for pass_id in range(3):
   for variant in ['go','rust_previous','rust_new','rayon2','rust_pgo']:
    row=run_one(variant,case,'memory',env,cpus,16,0);row.update(variant=variant,pass_id=pass_id+1);memory.append(row)
 (DATA/'memory.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in memory))
 kernels=[]
 for mode in ['kernel-residuals','kernel-huffman']:
  for pass_id in range(5):
   for name,mask in [('scalar',0),('words',1)]:
    row=run_one('rust_control','read_sealed_full8',mode,env,cpus,2048,32,override=('bin/rust-default',flags(mask)))
    row.update(variant=name,pass_id=pass_id+1,mode=mode);kernels.append(row)
 (DATA/'kernels.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in kernels))
 assert sources()==hashes,'sources changed during measurement'
 assert binary_hashes()==binaries,'binaries changed during measurement'
 metadata['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat();(DATA/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
 print(f'Finished {len(rows)} timings, {len(memory)} memory runs, {len(kernels)} kernel processes',flush=True)

if __name__=='__main__':main()
