#!/usr/bin/env python3
"""Six balanced same-session public-Go / previous-native / optimized comparisons."""
import argparse,datetime as dt,hashlib,json,math,os,platform,shutil,statistics,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent;WORK=Path('/work/records-opt');DATA=ROOT.parent/'reports/data/records-optimization-2026-10-09';FIXTURES=Path('/work/records-native/fixtures')
env=os.environ.copy();env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1');env.pop('GOGC',None)
READS=['scan_full','scan_filtered','scan_search','scan_trace','scan_page','scan_newest','scan_none','scan_no_context']
LABELS=['go','previous','opt']
ABLATIONS=[('typed-go-head','scan_filtered'),('typed-go-head','scan_page'),('typed-native-sealed','scan_full'),('typed-native-sealed','scan_filtered'),('typed-native-sealed','scan_page'),('typed-native-sealed','scan_none'),('typed-native-sealed','follow'),('typed-native-sealed','follow_walk')]
def ablation_labels(case):return [v for v in ['opt','prune_off','heap_off','sharing_off','cache_raw','cache_off','strict'] if v=='opt' or v=='prune_off' and case.startswith('scan_') or v=='heap_off' and case in ['scan_full','scan_page'] or v=='sharing_off' and case in ['scan_full','scan_filtered','follow'] or v.startswith('cache_') and case.startswith('follow') or v=='strict' and case in ['scan_none','scan_filtered']]
def call(args):return subprocess.run([str(a) for a in args],env=env,check=True,text=True,capture_output=True)
def sources():return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(ROOT.rglob('*')) if p.is_file() and '__pycache__' not in str(p)}
def binary(label,alloc=False):
 if label=='archive':return Path('/work/records-native/rust-records')
 if label=='go_archive':return Path('/work/records-native/go-records')
 return WORK/('go-records' if label=='go' else 'rust-alloc-records' if alloc else 'rust-records')
def args(label,fixture,case,iterations,warm=4,cold=False,alloc=False,mode='bench'):
 out=[binary(label,alloc),'--mode',mode,'--db',WORK/'active/records.db','--case',case,'--iterations',iterations]
 if label not in ['archive','go_archive']:
  out+=['--warm',warm]
  if cold:out+=['--cold',1]
  if label!='go':out+=['--variant',label]
 return out
def fresh(fixture):
 d=WORK/'active'
 if d.exists():shutil.rmtree(d)
 d.mkdir();shutil.copyfile(FIXTURES/fixture/'records.db',d/'records.db')
def row(label,fixture,case,iterations,warm=4,cold=False,alloc=False,mode='bench'):
 fresh(fixture);p=call(['taskset','-c',min(os.sched_getaffinity(0)),*args(label,fixture,case,iterations,warm,cold,alloc,mode)])
 value=json.loads(p.stdout);value.update(label=label,fixture=fixture,cache_state='cold_handle' if cold else 'warm_repeated_cursor' if case=='follow' else 'warm_advancing_walk' if case=='follow_walk' else 'warm_reads');return value
def write(name,v):DATA.mkdir(parents=True,exist_ok=True);(DATA/name).write_text(json.dumps(v,indent=2)+'\n')
def metadata():
 old=json.loads((ROOT.parent/'reports/data/records-native-2026-10-08/environment.json').read_text());return {'started_utc':dt.datetime.now(dt.timezone.utc).isoformat(),'tinystore_commit':old['tinystore_commit'],'harness_status':'exploratory uncommitted collection; prior user waiver persists; exact sources/binaries retained','source_sha256':sources(),'binary_sha256':{label:hashlib.sha256(binary(label).read_bytes()).hexdigest() for label in ['go','previous','archive','go_archive']},'rust_alloc_binary_sha256':hashlib.sha256(binary('opt',True).read_bytes()).hexdigest(),'fixture_sha256':{p.parent.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in FIXTURES.glob('*/records.db')},'go':call(['go','version']).stdout.strip(),'rustc':call(['rustc','--version','--verbose']).stdout.strip(),'platform':platform.platform(),'cpu':call(['lscpu']).stdout,'affinity':min(os.sched_getaffinity(0)),'cgroup_cpu_max':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'cgroup_memory_max':Path('/sys/fs/cgroup/memory.max').read_text().strip(),'gomaxprocs':1,'gogc':'default (100)','native_sqlite_build':old['native_sqlite_build'],'native_library_sha256':hashlib.sha256((ROOT.parent/'sqlite-bench/generated/lib/libsqlite3.a').read_bytes()).hexdigest(),'cold_definition':'new engine and connections before each measured operation; opening/destruction excluded; caches of this handle cold but OS filesystem cache uncontrolled','coordination':'no competing research build/test/timing; background desktop and VM work not independently controlled'}
def measure():
 info=metadata();info['timing_count_method']='ONE identical count per fixture/case across all variants; ceil(150ms/fastest pilot), floor8 cap32768; fixed across6passes; cold calls8';write('environment.json',info);raw=[];counts={};pilots=[]
 plans={}
 for fixture in ['typed-go-head','typed-native-sealed','apache-go-head','apache-native-sealed']:
  for case in READS+(['follow','follow_walk'] if fixture.endswith('sealed') else []):plans[(fixture,case)]=set(LABELS)
 for case in ['scan_full','scan_filtered','scan_page','follow']:plans[('typed-native-sealed',case)].update(['archive','go_archive'])
 for fixture,case in ABLATIONS:plans[(fixture,case)].update(ablation_labels(case))
 for (fixture,case),labels in plans.items():
  sample=[]
  for label in sorted(labels):
   p=row(label,fixture,case,1 if case=='follow_walk' else 8);sample.append(p);pilots.append(p)
  count=min(32768,max(8,math.ceil(150000000/min(p['ns_per_op'] for p in sample))));counts[(fixture,case)]=count
  for p in sample:p['selected_iterations']=count
  write('calibration.json',pilots)
 def calibrated(label,fixture,case,warm=4):
  return counts[(fixture,case)]
 orders=[['go','previous','opt'],['previous','opt','go'],['opt','go','previous'],['opt','previous','go'],['go','opt','previous'],['previous','go','opt']]
 with (DATA/'timings.jsonl').open('w') as out:
  for pass_id,order in enumerate(orders,1):
   print(f'main pass {pass_id}/6',flush=True)
   for fixture in ['typed-go-head','typed-native-sealed','apache-go-head','apache-native-sealed']:
    for case in READS+(['follow','follow_walk'] if fixture.endswith('sealed') else []):
     for label in order:
      iterations=calibrated(label,fixture,case)
      r=row(label,fixture,case,iterations);r.update(pass_id=pass_id,track='main');raw.append(r);out.write(json.dumps(r)+'\n');out.flush()
   for case in ['follow','follow_walk']:
    for label in order:
     r=row(label,'typed-native-sealed',case,8,warm=0,cold=True);r.update(pass_id=pass_id,track='cold');raw.append(r);out.write(json.dumps(r)+'\n');out.flush()
   # Archived binaries establish whether the extended driver changes controls.
   for case in ['scan_full','scan_filtered','scan_page','follow']:
    for label in (['archive','go_archive'] if pass_id%2 else ['go_archive','archive']):
     r=row(label,'typed-native-sealed',case,calibrated(label,'typed-native-sealed',case));r.update(pass_id=pass_id,track='archive');raw.append(r);out.write(json.dumps(r)+'\n');out.flush()
 with (DATA/'ablations.jsonl').open('w') as out:
  variants=['opt','prune_off','heap_off','sharing_off','cache_raw','cache_off','strict']
  for pass_id in range(1,7):
   print(f'ablation pass {pass_id}/6',flush=True)
   for fixture,case in ABLATIONS:
    selected=ablation_labels(case)
    if pass_id%2==0:selected.reverse()
    for label in selected:
     r=row(label,fixture,case,calibrated(label,fixture,case));r.update(pass_id=pass_id,track='ablation');raw.append(r);out.write(json.dumps(r)+'\n');out.flush()
 with (DATA/'allocations.jsonl').open('w') as out:
  for fixture,case in [('typed-go-head','scan_full'),('typed-go-head','scan_filtered'),('typed-native-sealed','scan_full'),('typed-native-sealed','scan_filtered'),('typed-native-sealed','scan_page'),('typed-native-sealed','follow'),('typed-native-sealed','follow_walk')]:
   for label in ['go','previous','opt','sharing_off','cache_raw']:
    r=row(label,fixture,case,8,alloc=True);out.write(json.dumps(r)+'\n')
 with (DATA/'memory.jsonl').open('w') as out:
  for pass_id,order in enumerate([['go','previous','opt','sharing_off'],['sharing_off','opt','previous','go'],['previous','go','sharing_off','opt']],1):
   for label in order:
    fresh('typed-native-sealed');p=call(['/usr/bin/time','-f','%M',*args(label,'typed-native-sealed','scan_full',64,mode='memory')]);r=json.loads(p.stdout);r.update(label=label,pass_id=pass_id,peak_rss_kib=int(p.stderr.strip().splitlines()[-1]));out.write(json.dumps(r)+'\n');out.flush()
 summary=[]
 for key in sorted({(r['track'],r['fixture'],r['case'],r['cache_state']) for r in raw}):
  group=[r for r in raw if (r['track'],r['fixture'],r['case'],r['cache_state'])==key];values={label:[r['ns_per_op'] for r in group if r['label']==label] for label in sorted({r['label'] for r in group})};summary.append(dict(zip(['track','fixture','case','cache_state'],key),median_ns={label:statistics.median(v) for label,v in values.items()},passes_ns=values))
 write('summary.json',summary);assert sources()==info['source_sha256'],'sources changed during run'
 for fixture,sha in info['fixture_sha256'].items():assert hashlib.sha256((FIXTURES/fixture/'records.db').read_bytes()).hexdigest()==sha,fixture
 info['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat();write('environment.json',info);print('All records comparisons complete; source/fixture hashes unchanged',flush=True)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--measure',action='store_true');a=p.parse_args()
 if a.measure:measure()
