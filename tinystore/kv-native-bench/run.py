#!/usr/bin/env python3
"""Balanced quiet-host collection; binaries/fixtures must already be verified."""
import datetime,hashlib,json,os,platform,shutil,sqlite3,subprocess,time,math
from pathlib import Path
from check import snapshot
ROOT=Path(__file__).resolve().parent;WORK=Path(os.environ.get('KV_WORK','/work/kv-native'));OUT=ROOT.parent/'reports/data/kv-native-2026-10-08'
CASES={'get_bytes':30000,'get_raw':30000,'getentry':30000,'has':40000,'get_spill':20000,'get_json':20000,'scan100':500,'set_bytes':500,'set_spill':300,'cas':400,'take_cycle':300,'delete_cycle':300,'counter_add':500,'clear_small':1,'clear_mark':1,'expire':1,'sql_get':40000}
KERNELS={'key_loop':1000000,'key_search':1000000,'value_codec':1000000,'json_codec':100000,'scan_materialize':10000,'ttl_guard':10000000}
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def command(a):return subprocess.check_output(a,text=True)
def clone(d):
 if d.exists():shutil.rmtree(d)
 d.mkdir();shutil.copyfile(WORK/'fixture/kv.db',d/'kv.db')
def invoke(binary,mode,name,n,d):
 a=[str(WORK/binary),'--mode',mode,'--case',name,'--iterations',str(n),'--dir',str(d)]
 timer=WORK/'process-time.json';fmt='{"user_s":%U,"system_s":%S,"maxrss_kib":%M,"minor_faults":%R,"major_faults":%F,"voluntary_context_switches":%w,"involuntary_context_switches":%c}'
 start=utc();p=subprocess.run(['/usr/bin/time','-f',fmt,'-o',str(timer),*a],text=True,capture_output=True);end=utc()
 if p.returncode:raise RuntimeError(f'{a}\n{p.stdout}\n{p.stderr}')
 row=json.loads(p.stdout);row.update(binary=binary,mode=mode,started_utc=start,finished_utc=end,process=json.loads(timer.read_text()),command=['<bin>/'+binary,*a[1:]])
 row['command']=[x.replace(str(WORK),'<work>')for x in row['command']];return row
def main():
 OUT.mkdir(exist_ok=True,parents=True);metadata=json.loads((WORK/'build.json').read_text());metadata.update(started_utc=utc(),machine=command(['sh','-c','uname -a; lscpu']),container_os=platform.platform(),python=platform.python_version(),work_volume='Linux Docker volume',collection_order='six passes, Go then Rust on even passes and Rust then Go on odd passes; each case uses a fresh file copy; no builds, tests or other measurements concurrent',gogc=os.environ.get('GOGC','default 100'),gomaxprocs=os.environ.get('GOMAXPROCS','unset'),affinity=command(['taskset','-pc',str(os.getpid())]).strip(),native_archive_sha256=hashlib.sha256((ROOT.parent/'sqlite-bench/generated/lib/libsqlite3.a').read_bytes()).hexdigest())
 for binary in ['go-kv','rust-kv']:metadata[binary+'_sqlite']=json.loads(command([str(WORK/binary),'--mode','metadata','--dir',str(WORK/'fixture')]))
 for name in ['correctness.json','trace-go.jsonl','trace-rust.jsonl','corruption-transcripts.json','page-bound-transcripts.json','guard-proof.json']:shutil.copyfile(WORK/name,OUT/name)
 c=sqlite3.connect(WORK/'fixture/kv.db');payload=c.execute('select sum(case typeof(c.value)when \'blob\'then length(c.value)when \'integer\'then 8 else 0 end+coalesce(length(s.value),0))from cells c left join spilled s on s.id=c.spill').fetchone()[0];c.close()
 storage={'file_bytes':(WORK/'fixture/kv.db').stat().st_size,'logical_payload_bytes':payload,'fixture_sha256':hashlib.sha256((WORK/'fixture/kv.db').read_bytes()).hexdigest(),'objects':json.loads(command([str(WORK/'go-kv'),'--mode','dbstat','--dir',str(WORK/'fixture')]))};(OUT/'storage.json').write_text(json.dumps(storage,indent=2)+'\n')
 metadata['base_counts']={'kernel':dict(KERNELS),'bench':dict(CASES)}
 # Quiet-window calibration fixes equal counts before six balanced passes.
 # One-shot branch/expiry fixtures intentionally remain one call per fresh file.
 with (OUT/'calibration.jsonl').open('w')as f:
  for mode,cases in [('kernel',KERNELS),('bench',CASES)]:
   for name,n in list(cases.items()):
    if n==1:continue
    pair=[]
    for binary in ['go-kv','rust-kv']:
     d=WORK/'run-store';clone(d);row=invoke(binary,mode,name,n,d);row['calibration_only']=True;f.write(json.dumps(row)+'\n');f.flush();pair.append(row)
    assert pair[0]['checksum']==pair[1]['checksum']
    scale=max(1,math.ceil(100000000/min(x['elapsed_ns']for x in pair)));cases[name]=min(n*scale,200000000 if mode=='kernel' else 500000)
 metadata['effective_counts']={'kernel':dict(KERNELS),'bench':dict(CASES)}
 print(json.dumps({'calibration_finished':utc(),'counts':metadata['effective_counts']}),flush=True)
 rows=[]
 with (OUT/'runs.jsonl').open('w')as raw:
  for passno in range(6):
   order=['go-kv','rust-kv']if passno%2==0 else ['rust-kv','go-kv']
   for mode,cases in [('kernel',KERNELS),('bench',CASES)]:
    for name,n in cases.items():
     paired=[]
     for binary in order:
      d=WORK/'run-store';clone(d)
      row=invoke(binary,mode,name,n,d);row['pass']=passno
      if mode=='bench' and name in ['set_bytes','set_spill','cas','take_cycle','delete_cycle','counter_add','clear_small','clear_mark','expire']:row['committed_snapshot']=snapshot(d)
      raw.write(json.dumps(row)+'\n');raw.flush();rows.append(row);paired.append(row)
     assert paired[0]['checksum']==paired[1]['checksum'],paired
     if 'committed_snapshot' in paired[0]:assert paired[0]['committed_snapshot']==paired[1]['committed_snapshot'],paired
   # Wrapper control gets a same-pass sibling to the safe rusqlite SQL row.
   d=WORK/'run-store';clone(d);row=invoke('rust-kv','bench','sql_ffi_get',CASES['sql_get'],d);row['pass']=passno;raw.write(json.dumps(row)+'\n');raw.flush();rows.append(row)
   print(json.dumps({'pass_finished':passno,'utc':utc(),'rows':len(rows)}),flush=True)
 # Separate allocation telemetry: never use these elapsed times for ratios.
 with (OUT/'allocations.jsonl').open('w')as f:
  for mode,cases in [('kernel',KERNELS),('bench',CASES)]:
   for name,n in cases.items():
    for binary in ['go-kv','rust-kv-telemetry']:
     d=WORK/'run-store';clone(d);row=invoke(binary,mode,name,min(n,10000)if n>1 else 1,d);row['diagnostic_only']=True;f.write(json.dumps(row)+'\n');f.flush()
 metadata['finished_utc']=utc();metadata['run_rows']=len(rows);metadata['source_files_final']={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest()for p in sorted(ROOT.rglob('*'))if p.is_file()and not any(x in p.parts for x in ['target','__pycache__','bin'])};(OUT/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n');print(json.dumps({'finished':metadata['finished_utc'],'rows':len(rows)}),flush=True)
if __name__=='__main__':main()
