#!/usr/bin/env python3
"""Longer same-binary kernel samples, independently retained from the main run."""
import datetime as dt,hashlib,json,math,os,statistics,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/records-native-2026-10-08'
WORK=Path('/work/records-native')
baseline=json.loads((DATA/'environment.json').read_text())
samples=json.loads((DATA/'summary.json').read_text())
counts={v['case']:min(131072,max(2048,math.ceil(150000000/min(v['go_ns'],v['rust_ns'])))) for v in samples if v['track']=='kernel'}
env=os.environ.copy();env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1');env.pop('GOGC',None)
cpu=min(os.sched_getaffinity(0));binaries={lang:WORK/(lang+'-records') for lang in ['go','rust']}
def verify_hashes():
 for name,digest in baseline['source_sha256'].items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,('source changed',name)
 for name,path in binaries.items():assert hashlib.sha256(path.read_bytes()).hexdigest()==baseline['binary_sha256'][name],('binary changed',name)
verify_hashes()
metadata={'started_utc':dt.datetime.now(dt.timezone.utc).isoformat(),'method':'same fixed count for Go/Rust per case; ceil(150ms / fastest main median), floor2048 cap131072; six balanced passes','passes':6,'counts':counts,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'binary_sha256':{lang:baseline['binary_sha256'][lang] for lang in binaries},'main_runtime_source_hashes_verified':True,'affinity':cpu}
raw=[]
with (DATA/'kernel-recheck-timings.jsonl').open('w') as out:
 for run in range(1,7):
  print(f'long kernel pass {run}/6',flush=True)
  for case,count in counts.items():
   for lang in (['go','rust'] if run%2 else ['rust','go']):
    p=subprocess.run(['taskset','-c',str(cpu),str(binaries[lang]),'--mode','kernel-bench','--case',case,'--iterations',str(count)],env=env,check=True,text=True,capture_output=True)
    row=json.loads(p.stdout);row.update(pass_id=run,utc=dt.datetime.now(dt.timezone.utc).isoformat());row['elapsed_ms']=row['ns_per_op']*count/1e6;raw.append(row);out.write(json.dumps(row)+'\n');out.flush()
summary=[]
for case,count in counts.items():
 values={lang:[r['ns_per_op'] for r in raw if r['language']==lang and r['case']==case] for lang in binaries}
 medians={lang:statistics.median(v) for lang,v in values.items()};summary.append({'case':case,'iterations':count,'go_ns':medians['go'],'rust_ns':medians['rust'],'ratio_go_over_rust':medians['go']/medians['rust'],'passes_ns':values})
(DATA/'kernel-recheck-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
verify_hashes();metadata['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat();(DATA/'kernel-recheck-environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
print('Long kernels complete; original runtime/main-run sources and binaries unchanged',flush=True)
