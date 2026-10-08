#!/usr/bin/env python3
"""Independent same-session sync-policy supplement; no previous timing reuse."""
import hashlib,json,math,os,shutil,subprocess
from pathlib import Path
from build import ROOT,WORK,files
import run
SYNC=Path('/work/kv-opt-fdatasync');OUT=ROOT.parent/'reports/data/kv-optimization-2026-10-09/fdatasync'
COUNTS={'set_bytes':512,'set_spill':512,'set_64k':256,'cas':512,'counter_add':512}
def main():
 OUT.mkdir(parents=True,exist_ok=True);run.OUT=OUT;run.VARIANTS={'go':(WORK/'go-kv',[]),'fsync':(WORK/'rust-kv',['--tuning','3']),'fdatasync':(SYNC/'rust-kv',['--tuning','3'])};build=json.loads((SYNC/'build.json').read_text());meta={'started_utc':run.utc(),'build':build,'counts_initial':dict(COUNTS),'source_files_initial':files(ROOT),'binary_hashes_initial':{k:hashlib.sha256(v[0].read_bytes()).hexdigest()for k,v in run.VARIANTS.items()},'method':'fresh identical fixture, 64warmup operations, six balanced paired triples; separate strace; same-session baselines, no main-session timing reuse','coordination':'parent/records pause acknowledged; desktop background activity uncontrolled','gomaxprocs':os.environ.get('GOMAXPROCS'),'gogc':os.environ.get('GOGC'),'affinity':subprocess.check_output(['taskset','-pc',str(os.getpid())],text=True)}
 assert build['source_equal'] and build['baseline_binary_sha256']==meta['binary_hashes_initial']['fsync'];assert build['binary_sha256']==meta['binary_hashes_initial']['fdatasync'];assert hashlib.sha256((WORK/'fixture/kv.db').read_bytes()).hexdigest()==hashlib.sha256((SYNC/'fixture/kv.db').read_bytes()).hexdigest()
 shutil.copyfile(SYNC/'build.json',OUT/'build.json');shutil.copyfile(SYNC/'correctness.json',OUT/'correctness.json')
 for p in SYNC.glob('*-result.jsonl'):shutil.copyfile(p,OUT/p.name)
 for name in ['trace-go.jsonl','trace-rust.jsonl','go-kv-no-result.jsonl','rust-kv-no-result.jsonl','page-bound-transcripts.json','corruption-transcripts.json']:shutil.copyfile(SYNC/name,OUT/name)
 with(OUT/'calibration.jsonl').open('w')as f:
  for case,n in list(COUNTS.items()):
   rows=[]
   for v in run.VARIANTS:
    row=run.invoke(v,case,n);row['calibration_only']=True;f.write(json.dumps(row)+'\n');f.flush();rows.append(row)
   run.validate(rows);COUNTS[case]=min(1000000,n*max(1,math.ceil(200000000/min(r['elapsed_ns']for r in rows))))
 meta['fixed_counts']=dict(COUNTS);meta['workload']={'initial_fixture':run.snapshot(WORK/'fixture'),'warmup_calls_each_process':64,'key_selection':'(iteration*997)&4095, spilled keys (iteration*997)&511','initial_wal':'absent; fresh closed fixture copy per process','wal_autocheckpoint':build['readback']['fsync']['writer']['wal_autocheckpoint'],'write_value_bytes':{'set_bytes':256,'set_spill':4096,'set_64k':65536,'cas':256,'counter_add':'int64 +1'},'counter_selection':'(iteration*997)&1023'};print(json.dumps({'calibration_finished':run.utc(),'counts':COUNTS}),flush=True)
 with(OUT/'runs.jsonl').open('w')as f:
  for passno in range(6):
   for case,n in COUNTS.items():
    order=list(run.VARIANTS)if passno%2==0 else list(run.VARIANTS)[::-1];rows=[]
    for v in order:
     row=run.invoke(v,case,n);row['pass']=passno;f.write(json.dumps(row)+'\n');f.flush();rows.append(row)
    run.validate(rows)
   print(json.dumps({'pass_finished':passno,'utc':run.utc()}),flush=True)
 with(OUT/'syscalls.jsonl').open('w')as f:
  for case in ['set_bytes','set_spill','set_64k']:
   for v in run.VARIANTS:
    row=run.invoke(v,case,512,extra=['strace','-f','-c','-e','trace=fsync,fdatasync,pwrite64,pread64,ftruncate,fcntl']);row['diagnostic_only']=True;f.write(json.dumps(row)+'\n');f.flush()
 meta['finished_utc']=run.utc();meta['source_files_final']=files(ROOT);meta['binary_hashes_final']={k:hashlib.sha256(v[0].read_bytes()).hexdigest()for k,v in run.VARIANTS.items()};assert meta['source_files_initial']==meta['source_files_final'];assert meta['binary_hashes_initial']==meta['binary_hashes_final'];(OUT/'environment.json').write_text(json.dumps(meta,indent=2)+'\n');print(json.dumps({'finished':meta['finished_utc']}),flush=True)
if __name__=='__main__':main()
