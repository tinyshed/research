#!/usr/bin/env python3
"""Fixed-input ablations: production Go, baseline Rust, optimized Rust, Rayon."""
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

ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/metrics-optimization-2026-10-08'
WIDE=['read_wide64','stream_wide64']+['aggregate_wide_'+s for s in ['sum','cut_sum','cut_avg','cut_count','grouped_cut_sum']]
CASES=previous.CASES+WIDE
VARIANTS={
 'go':('bin/go-metrics-bench',[]),
 'rust_base':('bin/rust-baseline',[]),
 'rust_serial':('bin/rust-optimized',['--threads','0','--fast-exact','1','--fast-codec','1']),
 'rayon1':('bin/rust-optimized',['--threads','1','--fast-exact','1','--fast-codec','1']),
 'rayon2':('bin/rust-optimized',['--threads','2','--fast-exact','1','--fast-codec','1']),
 'rust_refactor':('bin/rust-optimized',['--threads','0','--fast-exact','0','--fast-codec','0']),
 'rust_exact':('bin/rust-optimized',['--threads','0','--fast-exact','1','--fast-codec','0']),
 'rust_codec':('bin/rust-optimized',['--threads','0','--fast-exact','0','--fast-codec','1']),
 'rayon2_shared_cache':('bin/rust-optimized',['--threads','2','--fast-exact','1','--fast-codec','1']),
}
MAIN=['go','rust_base','rust_serial','rayon1','rayon2']
ABLATIONS=['rust_refactor','rust_exact','rust_codec','rayon2_shared_cache']
ABLATION_CASES=['read_sealed_full16','read_wide64','aggregate_count','aggregate_cut_sum','aggregate_cut_avg','aggregate_wide_cut_sum','aggregate_wide_grouped_cut_sum','maintain_ready']

def call(args,**kwargs):
 r=subprocess.run(args,text=True,capture_output=True,**kwargs)
 if r.returncode:raise RuntimeError(f'{args}: {r.returncode}\n{r.stdout}\n{r.stderr}')
 return r.stdout

def fixture_for(case):
 return 'wide' if case in WIDE else previous.fixture_for(case)

def select_cpus():
 available=sorted(os.sched_getaffinity(0))
 first=available[0]
 cache=lambda cpu:Path(f'/sys/devices/system/cpu/cpu{cpu}/cache/index0/shared_cpu_list').read_text().strip()
 second=next((cpu for cpu in available[1:] if cache(cpu)!=cache(first)),available[1])
 return [first,second]+[cpu for cpu in available if cpu not in [first,second]]

def affinity_for(variant,cpus):
 return [cpus[0],cpus[-1]] if variant=='rayon2_shared_cache' else cpus[:2] if variant=='rayon2' else cpus[:1]

def run_one(variant,case,mode,env,cpus,iterations=1,warm=0,force=False):
 binary,flags=VARIANTS[variant]
 with tempfile.TemporaryDirectory(prefix='opt-',dir=ROOT/'stores') as d:
  db=Path(d)/'metrics.db';shutil.copyfile(ROOT/'stores'/fixture_for(case)/'metrics.db',db)
  args=[str(ROOT/binary),'--db',str(db),'--case',case,'--mode',mode,'--iterations',str(iterations),'--warm',str(warm),*flags]
  if case=='expire_all':args+=['--now',str(previous.NORMAL_NOW+30*24*60*60*1000+5000)]
  if force and variant.startswith('rayon'):args+=['--force-parallel','1']
  resource=Path(d)/'rss'
  if mode=='memory':args=[shutil.which('time',path=env['PATH']),'-f','%M','-o',str(resource),*args]
  affinity=','.join(map(str,affinity_for(variant,cpus)))
  row=json.loads(call(['taskset','-c',affinity,*args],env=env))
  if mode=='memory':row['max_rss_kib']=int(resource.read_text())
  if case in previous.WRITE_CASES and mode in ['bench','verify']:
   row['go_crossread']=json.loads(call([str(ROOT/'bin/go-metrics-bench'),'--db',str(db),'--mode','digest'],env=env))['final_digest']
  return row

def sources():
 paths=[ROOT/'opt_run.py',ROOT/'run.py']
 for folder in ['go','rust','rust-baseline-src']:
  paths += [p for p in (ROOT/folder).rglob('*') if p.is_file() and 'target' not in p.parts and (p.suffix in ['.rs','.go'] or p.name in ['Cargo.toml','Cargo.lock','go.mod','go.sum'])]
 return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}

def main():
 p=argparse.ArgumentParser();p.add_argument('--verify-only',action='store_true');p.add_argument('--skip-verify',action='store_true');p.add_argument('--passes',type=int,default=5);a=p.parse_args()
 DATA.mkdir(parents=True,exist_ok=True)
 env=os.environ.copy();env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1');env.pop('GOGC',None)
 cpus=select_cpus();assert len(cpus)>=2
 hashes=sources();binaries={str(b):hashlib.sha256((ROOT/b).read_bytes()).hexdigest() for b,_ in VARIANTS.values()}
 if not a.skip_verify:
  verification=[]
  for case in CASES+previous.EDGE_CASES:
   n=1 if case in ['maintain_ready','expire_all'] or case in previous.EDGE_CASES else 4
   outputs={v:run_one(v,case,'verify',env,cpus,n,force=True) for v in VARIANTS}
   norm=lambda r:{k:v for k,v in r.items() if k!='implementation'}
   expected=norm(outputs['go'])
   assert all(norm(x)==expected for x in outputs.values()),(case,outputs)
   verification.append({'case':case,'forced_parallel':True,'result':expected})
   print('Verified '+case,flush=True)
  guards={}
  for mode,case in [('guards','read_sealed_full8'),('edge-guards','read_edge')]:
   outputs={v:run_one(v,case,mode,env,cpus,force=True) for v in VARIANTS}
   assert all(x==outputs['go'] for x in outputs.values()),(mode,outputs)
   assert all(v for k,v in outputs['go'].items() if k!='final_digest')
   guards[mode]=outputs
  plans=[]
  for case in previous.READ_CASES+previous.AGGREGATE_CASES+WIDE:
   if case.startswith('stream_'):continue
   outputs={v:run_one(v,case,'plan',env,cpus) for v in MAIN}
   assert all(x==outputs['go'] for x in outputs.values()),('plans',case,outputs)
   plans.append({'case':case,**outputs['go']})
  (DATA/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
  (DATA/'guards.json').write_text(json.dumps(guards,indent=2)+'\n')
  (DATA/'plans.json').write_text(json.dumps(plans,indent=2)+'\n')
 assert sources()==hashes
 if a.verify_only:return
 counts={};pilot=[]
 for case in CASES:
  if case in ['maintain_ready','expire_all']:counts[case]=1;continue
  row=run_one('rust_base',case,'bench',env,cpus,16,32);pilot.append(row)
  cap=16 if case=='ingest_register8' else 512 if case=='ingest_scrape100' else 4096 if case in previous.WRITE_CASES else 16384
  counts[case]=max(1,min(cap,math.ceil(0.12e9/row['ns_per_op'])))
 metadata={'started_utc':dt.datetime.now(dt.timezone.utc).isoformat(),'tinystore_commit':call(['git','rev-parse','HEAD'],cwd=ROOT.parent/'source').strip(),'source_sha256':hashes,'binary_sha256':binaries,'go':call(['go','version'],env=env).strip(),'rustc':call(['rustc','--version'],env=env).strip(),'platform':platform.platform(),'cpu':call(['lscpu']).strip(),'cpu_quota':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'memory_limit':Path('/sys/fs/cgroup/memory.max').read_text().strip(),'cpus':cpus,'affinity':{v:affinity_for(v,cpus) for v in VARIANTS},'cache_topology':{str(cpu):{index.name:(index/'shared_cpu_list').read_text().strip() for index in Path(f'/sys/devices/system/cpu/cpu{cpu}/cache').glob('index*')} for cpu in cpus},'variants':VARIANTS,'gomaxprocs':1,'warm':32,'iterations':counts,'passes':a.passes,'harness_status':'uncommitted local prototype; not a published formal benchmark','baseline_note':'Original Rust algorithms + only CLI additions for wide workloads; baseline source copy included','fixtures':{kind:{'sha256':hashlib.sha256((ROOT/'stores'/kind/'metrics.db').read_bytes()).hexdigest(),'bytes':(ROOT/'stores'/kind/'metrics.db').stat().st_size} for kind in ['sealed','ready','head','scrape','empty','edge','wide']}}
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
  values={k:v for k,v in values.items() if v}
  medians={v:statistics.median(xs) for v,xs in values.items()}
  summary.append({'case':case,'iterations':counts[case],'median_ns':medians,'passes_ns':values,'serial_vs_base':medians['rust_base']/medians['rust_serial'],'rayon2_vs_serial':medians['rust_serial']/medians['rayon2'],'rayon2_vs_base':medians['rust_base']/medians['rayon2'],'go_over_rayon2':medians['go']/medians['rayon2']})
 (DATA/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
 memory=[]
 for case in ['read_sealed_full16','read_wide64','stream_wide64','aggregate_cut_sum','aggregate_wide_cut_sum']:
  for pass_id in range(3):
   for variant in MAIN if pass_id%2==0 else MAIN[::-1]:
    row=run_one(variant,case,'memory',env,cpus,16,0);row.update(variant=variant,pass_id=pass_id+1);memory.append(row)
 (DATA/'memory.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in memory))
 assert sources()==hashes,'sources changed while measuring'
 assert binaries=={str(b):hashlib.sha256((ROOT/b).read_bytes()).hexdigest() for b,_ in VARIANTS.values()},'binaries changed while measuring'
 metadata['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat();(DATA/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
 print(f'Finished {len(rows)} timings + {len(memory)} memory runs',flush=True)

if __name__=='__main__':main()
