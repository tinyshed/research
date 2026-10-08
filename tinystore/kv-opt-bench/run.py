#!/usr/bin/env python3
"""Quiet-host balanced same-session Go/old/normalized/optimized collection."""
import datetime,hashlib,json,math,os,platform,shutil,sqlite3,subprocess
from pathlib import Path
from build import ROOT,WORK,OLD,files
from check import snapshot
OUT=ROOT.parent/'reports/data/kv-optimization-2026-10-09'
VARIANTS={'go':(OLD/'go-kv',[]),'old':(OLD/'rust-kv',[]),'normalized':(WORK/'rust-kv',['--tuning','0']),'paths':(WORK/'rust-kv',['--tuning','1']),'transactions':(WORK/'rust-kv',['--tuning','2']),'optimized':(WORK/'rust-kv',['--tuning','3']),'go-extended':(WORK/'go-kv',[])}
BASE={'get_bytes':40000,'get_spill':40000,'get_json':40000,'has':50000,'scan100':4000,'set_bytes':64,'set_spill':64,'cas':64,'take_cycle':32,'delete_cycle':32,'counter_add':64,'sql_get':50000}
EXTRA={'set_64k':64,'setentry_bytes':64,'setentry_spill':64}
MUTATIONS={'set_bytes','set_spill','cas','take_cycle','delete_cycle','counter_add',*EXTRA}
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def clone(d):
 if d.exists():shutil.rmtree(d)
 d.mkdir();shutil.copyfile(WORK/'fixture/kv.db',d/'kv.db')
def invoke(v,case,n,diagnostic=None,extra=None):
 binary,args=VARIANTS[v];binary=diagnostic or binary;d=WORK/'run-store';clone(d);cmd=[str(binary),*args,'--mode','bench','--case',case,'--iterations',str(n),'--dir',str(d)]
 timer=WORK/'process-time.json';fmt='{"user_s":%U,"system_s":%S,"maxrss_kib":%M,"minor_faults":%R,"major_faults":%F,"voluntary_context_switches":%w,"involuntary_context_switches":%c}';started=utc();prefix=extra or ['/usr/bin/time','-f',fmt,'-o',str(timer)];r=subprocess.run([*prefix,*cmd],text=True,capture_output=True)
 if r.returncode:raise RuntimeError(f'{cmd}\n{r.stdout}\n{r.stderr}')
 row=json.loads(r.stdout);row.update(variant=v,started_utc=started,finished_utc=utc(),command=[x.replace(str(WORK),'<work>').replace(str(OLD),'<previous-work>')for x in cmd]);
 if extra:row['diagnostic_stderr']=r.stderr
 else:row['process']=json.loads(timer.read_text())
 if case in MUTATIONS:row['committed_snapshot']=snapshot(d)
 return row
def validate(rows):
 assert len({r['checksum']for r in rows})==1,rows
 if rows[0]['case']in MUTATIONS:assert len({json.dumps(r['committed_snapshot'],sort_keys=True)for r in rows})==1,rows
def main():
 OUT.mkdir(exist_ok=True,parents=True);meta=json.loads((WORK/'build.json').read_text());assert files(ROOT.parent/'kv-native-bench')==meta['original_source_files'];meta.update(started_utc=utc(),machine=subprocess.check_output(['sh','-c','uname -a; lscpu'],text=True),affinity=subprocess.check_output(['taskset','-pc',str(os.getpid())],text=True),python=platform.python_version(),gogc=os.environ.get('GOGC'),gomaxprocs=os.environ.get('GOMAXPROCS'),coordination='parent and records paused research builds/tests/measurement by explicit RUN handshake; other desktop/background activity uncontrolled',variant_definitions={k:{'binary':str(v[0]).replace(str(WORK),'<work>').replace(str(OLD),'<previous-work>'),'args':v[1]}for k,v in VARIANTS.items()},cgroup={n:(Path('/sys/fs/cgroup')/n).read_text().strip()for n in ['cpu.max','memory.max','cpuset.cpus.effective','pids.max']})
 for name in ['go','old','optimized']:
  b,a=VARIANTS[name];meta[name+'_sqlite']=json.loads(subprocess.check_output([str(b),*a,'--mode','metadata','--dir',str(WORK/'fixture')],text=True))
 for p in WORK.glob('correctness*.json'):shutil.copyfile(p,OUT/p.name)
 for n in ['trace-go.jsonl','trace-rust.jsonl','go-kv-no-result.jsonl','rust-kv-no-result.jsonl','corruption-transcripts.json','page-bound-transcripts.json','guard-proof.json','ownership-proof.json']:shutil.copyfile(WORK/n,OUT/n)
 meta['fixture_sha256']=hashlib.sha256((WORK/'fixture/kv.db').read_bytes()).hexdigest();shutil.copyfile(ROOT.parent/'reports/data/kv-native-2026-10-08/storage.json',OUT/'storage.json')
 meta['initial_counts']={'base':dict(BASE),'extra':dict(EXTRA)}
 with(OUT/'calibration.jsonl').open('w')as f:
  for cases,variants in [(BASE,['go','old','normalized','paths','transactions','optimized']),(EXTRA,['go-extended','normalized','optimized'])]:
   for case,n in list(cases.items()):
    rows=[]
    for v in variants:
     row=invoke(v,case,n);row['calibration_only']=True;f.write(json.dumps(row)+'\n');f.flush();rows.append(row)
    validate(rows);scale=max(1,math.ceil(175000000/min(r['elapsed_ns']for r in rows)));cases[case]=min(n*scale,1000000)
 meta['fixed_counts']={'base':dict(BASE),'extra':dict(EXTRA)};print(json.dumps({'calibrated':utc(),'counts':meta['fixed_counts']}),flush=True)
 with(OUT/'runs.jsonl').open('w')as f:
  for passno in range(6):
   for cases,variants in [(BASE,['go','old','normalized','paths','transactions','optimized']),(EXTRA,['go-extended','normalized','optimized'])]:
    for case,n in cases.items():
     order=variants if passno%2==0 else variants[::-1];rows=[]
     for v in order:
      row=invoke(v,case,n);row['pass']=passno;f.write(json.dumps(row)+'\n');f.flush();rows.append(row)
     validate(rows)
   print(json.dumps({'pass_finished':passno,'utc':utc()}),flush=True)
 with(OUT/'allocations.jsonl').open('w')as f:
  for case in ['get_bytes','scan100','set_bytes','set_spill','counter_add','set_64k','setentry_bytes','setentry_spill']:
   for v in (['go','old','normalized','paths','transactions','optimized']if case in BASE else ['go-extended','normalized','optimized']):
    diagnostic=(OLD/'rust-kv-telemetry')if v=='old'else(WORK/'rust-kv-telemetry')if not v.startswith('go')else None;row=invoke(v,case,min(64,BASE.get(case,EXTRA.get(case,64))),diagnostic=diagnostic);row['diagnostic_only']=True;f.write(json.dumps(row)+'\n');f.flush()
 with(OUT/'phases.jsonl').open('w')as f:
  for case in ['set_bytes','set_spill','set_64k','counter_add']:
   for v in ['normalized','transactions','optimized']:
    row=invoke(v,case,128,diagnostic=WORK/'rust-kv-profile');row['diagnostic_only']=True;f.write(json.dumps(row)+'\n');f.flush()
 with(OUT/'syscalls.jsonl').open('w')as f:
  for case in ['set_bytes','set_spill','set_64k']:
   for v in (['go','old','normalized','optimized']if case in BASE else ['go-extended','normalized','optimized']):
    row=invoke(v,case,128,extra=['strace','-f','-c','-e','trace=fsync,fdatasync,pwrite64,pread64,ftruncate,fcntl']);row['diagnostic_only']=True;f.write(json.dumps(row)+'\n');f.flush()
 meta['finished_utc']=utc();meta['source_files_final']=files(ROOT);meta['original_source_files_final']=files(ROOT.parent/'kv-native-bench');meta['binaries_final']={p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in WORK.glob('*-kv*')if p.is_file()};meta['original_binaries_final']={n:hashlib.sha256((OLD/n).read_bytes()).hexdigest()for n in meta['original_binaries']};assert meta['source_files']==meta['source_files_final'];assert meta['original_source_files']==meta['original_source_files_final'];assert meta['binaries']==meta['binaries_final'];assert meta['original_binaries']==meta['original_binaries_final'];(OUT/'environment.json').write_text(json.dumps(meta,indent=2)+'\n');print(json.dumps({'finished':meta['finished_utc']}),flush=True)
if __name__=='__main__':main()
