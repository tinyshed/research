#!/usr/bin/env python3
"""Records research: prepare/verify separately from a coordinated quiet run."""
import argparse, datetime as dt, hashlib, json, os, platform, shutil, statistics, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent
WORK=Path('/work/records-native')
DATA=ROOT.parent/'reports/data/records-native-2026-10-08'
COMMIT='e307c48a40126aad0e2873b6bf3aaedef8115483'
ENV=os.environ.copy();ENV.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1')
ENV.pop('GOGC',None)
BIN={'go':WORK/'go-records','rust':WORK/'rust-records','rust-alloc':WORK/'rust-alloc-records'}
CASES=['scan_full','scan_filtered','scan_search','scan_trace','scan_page','scan_newest','follow']
def command(args,check=True):
 p=subprocess.run([str(a) for a in args],env=ENV,text=True,capture_output=True)
 if check and p.returncode:raise RuntimeError(f'{args}\n{p.stdout}\n{p.stderr}')
 return p
def rows(lang,mode,db=None,case=None,iterations=None,extra=(),timed=False):
 args=[BIN[lang],'--mode',mode]
 if db:args+=['--db',db]
 if case:args+=['--case',case]
 if iterations is not None:args+=['--iterations',str(iterations)]
 args+=list(extra)
 if timed:args=['taskset','-c',str(min(os.sched_getaffinity(0))),*args]
 p=command(args)
 return [json.loads(s) for s in p.stdout.splitlines()]
def write(name,value):
 DATA.mkdir(parents=True,exist_ok=True);(DATA/name).write_text(json.dumps(value,indent=2)+'\n')
def fresh(source,destination):
 if destination.exists():shutil.rmtree(destination)
 destination.mkdir(parents=True);shutil.copyfile(source,destination/'records.db');return destination/'records.db'
def prepare():
 WORK.mkdir(exist_ok=True);DATA.mkdir(parents=True,exist_ok=True)
 fixtures=WORK/'fixtures';fixtures.mkdir(exist_ok=True)
 for corpus in ['typed','apache']:
  extra=[] if corpus=='typed' else ['--corpus',WORK/'corpus/Apache/Apache_2k.log']
  trace=WORK/(corpus+'.jsonl');rows('go','export',extra=['--output',trace,'--count','4096',*extra])
  for kind in ['go-head','go-sealed','native-head','native-sealed','empty']:
   directory=fixtures/(corpus+'-'+kind)
   if directory.exists():shutil.rmtree(directory)
   directory.mkdir();db=directory/'records.db'
   count='4096' if kind.startswith('go-') else '0'
   rows('go','fixture',db,'sealed' if kind=='go-sealed' else 'head',extra=['--count',count,*extra])
   if kind.startswith('native-'):rows('rust','fixture',db,'sealed' if kind=='native-sealed' else 'head',extra=['--input',trace])
 verification=[]
 for corpus in ['typed','apache']:
  for kind in ['go-head','native-head','native-sealed']:
   db=fixtures/(corpus+'-'+kind)/'records.db';a=rows('go','verify',db);b=rows('rust','verify',db)
   assert a==b,(corpus,kind,a,b)
   verification.append({'fixture':corpus+'-'+kind,'results':a,'sha256':hashlib.sha256(db.read_bytes()).hexdigest()})
  # Unsupported production column encodings are a scope boundary, never silently reinterpreted.
  p=command([BIN['rust'],'--mode','verify','--db',fixtures/(corpus+'-go-sealed')/'records.db'],check=False)
  assert p.returncode!=0 and 'unsupported' in p.stderr,(p.stdout,p.stderr)
  verification.append({'fixture':corpus+'-go-sealed','native_decoder':'unsupported','reason':p.stderr.strip()})
 a=rows('go','kernel-verify',case='all');b=rows('rust','kernel-verify',case='all');assert a==b,'kernel outputs differ'
 for c in a:
  original=c['case'].replace('/decode_word','/decode').replace('sort/arrival_key','sort/stable')
  assert c['hex']==next(x['hex'] for x in a if x['case']==original)
 write('kernels-verification.json',[{'case':v['case'],'units':v['units'],'output_bytes':len(v['hex'])//2,'output_sha256':hashlib.sha256(bytes.fromhex(v['hex'])).hexdigest()} for v in a])
 for lang in ['go','rust']:rows(lang,'guards',fixtures/'typed-empty'/'records.db')
 import sqlite3
 late=fresh(fixtures/'typed-native-head/records.db',fixtures/'typed-late-native-sealed')
 rows('go','late',late);conn=sqlite3.connect(late);late_heads=conn.execute('select sum(count) from heads where late=1').fetchone()[0];conn.close();assert late_heads==4
 rows('rust','seal-now',late)
 conn=sqlite3.connect(late);segments=conn.execute('select id,first_at,last_at,count from segments order by id').fetchall();conn.close()
 assert any(b[1]<a[1] for a,b in zip(segments,segments[1:])),segments
 for kind in ['go-head','native-sealed','late-native-sealed']:
  db=fixtures/('typed-'+kind)/'records.db';a=rows('go','verify-walk',db);b=rows('rust','verify-walk',db);assert a==b,(kind,a,b)
  assert a[0]['count']==4096+(4 if kind.startswith('late') else 0)
  verification.append({'fixture':'typed-'+kind,'walks':a,'late_segments':segments if kind.startswith('late') else []})
 corrupt=[]
 for kind,table in [('go-head','heads'),('native-sealed','blocks')]:
  for guard in ['crc','index']:
   db=fresh(fixtures/('typed-'+kind)/'records.db',WORK/'corrupt');conn=sqlite3.connect(db)
   if guard=='crc':
    rowid,body=conn.execute(f'select id,body from {table} order by id limit 1').fetchone();body=bytearray(body);body[-1]^=1;conn.execute(f'update {table} set body=? where id=?',(body,rowid))
   else:conn.execute(f'update {table} set count=count-1 where id=(select min(id) from {table})')
   conn.commit();conn.close()
   for lang in ['go','rust']:
    p=command([BIN[lang],'--mode','verify','--db',db],check=False);assert p.returncode!=0,(kind,guard,lang)
    corrupt.append({'fixture':kind,'guard':guard,'reader':lang,'rejected':True,'error':p.stderr.splitlines()[0]})
 verification.append({'negative_probes':corrupt})
 write('verification.json',verification)
 print('Verified head cross-reads and identical native sealed artifacts, 21 kernels, guards',flush=True)
def hashes():
 return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(ROOT.rglob('*')) if p.is_file() and p.suffix not in ['.pyc'] and '__pycache__' not in str(p)}
def metadata():
 return {'tinystore_commit':COMMIT,'harness_status':'exploratory uncommitted collection; user waived harness-before-measurement commit; exact source and binary hashes retained','started_utc':dt.datetime.now(dt.timezone.utc).isoformat(),'platform':platform.platform(),'go':command(['go','version']).stdout.strip(),'rustc':command(['rustc','--version','--verbose']).stdout.strip(),'cpu':command(['lscpu']).stdout,'affinity':min(os.sched_getaffinity(0)),'cgroup_cpu_max':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'cgroup_memory_max':Path('/sys/fs/cgroup/memory.max').read_text().strip(),'source_sha256':hashes(),'binary_sha256':{k:hashlib.sha256(p.read_bytes()).hexdigest() for k,p in BIN.items()},'native_sqlite_build':json.loads((ROOT.parent/'sqlite-bench/generated/native-build.json').read_text()),'native_library_sha256':hashlib.sha256((ROOT.parent/'sqlite-bench/generated/lib/libsqlite3.a').read_bytes()).hexdigest(),'go_gomaxprocs':1,'gogc':'default (100)','corpus_sha256':{str(p.relative_to(WORK/'corpus')):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((WORK/'corpus').rglob('*')) if p.is_file()}}
def measure(passes):
 assert passes>=6 and passes%2==0
 info=metadata();info['passes']=passes;write('environment.json',info)
 fixtures=WORK/'fixtures';raw=[]
 with (DATA/'timings.jsonl').open('w') as out:
  for run in range(1,passes+1):
   for lang in (['go','rust'] if run%2 else ['rust','go']):
    print(f'pass {run}/{passes}: {lang}',flush=True)
    for case in [v['case'] for v in json.loads((DATA/'kernels-verification.json').read_text())]:
     row=rows(lang,'kernel-bench',case=case,iterations=2048,timed=True)[0];row.update(pass_id=run,track='kernel');raw.append(row);out.write(json.dumps(row)+'\n');out.flush()
    for corpus in ['typed','apache']:
     for kind in ['go-head','native-sealed']:
      for case in CASES:
       if case=='follow' and kind=='go-head':continue
       db=fresh(fixtures/(corpus+'-'+kind)/'records.db',WORK/'active');row=rows(lang,'bench',db,case,32, timed=True)[0];row.update(pass_id=run,track='engine',fixture=corpus+'-'+kind);raw.append(row);out.write(json.dumps(row)+'\n');out.flush()
     for case,kind,iterations in [('append','empty',16),('seal','go-head',1)]:
      db=fresh(fixtures/(corpus+'-'+kind)/'records.db',WORK/'active');extra=['--corpus',WORK/'corpus/Apache/Apache_2k.log'] if lang=='go' and corpus=='apache' else ['--input',WORK/(corpus+'.jsonl')] if lang=='rust' else []
      row=rows(lang,'bench',db,case,iterations,extra,timed=True)[0];row.update(pass_id=run,track='writer',fixture=corpus+'-'+kind);raw.append(row);out.write(json.dumps(row)+'\n');out.flush()
 with (DATA/'allocations.jsonl').open('w') as out:
  for lang in ['go','rust-alloc']:
   for case in [v['case'] for v in json.loads((DATA/'kernels-verification.json').read_text())]:
    row=rows(lang,'kernel-bench',case=case,iterations=256,timed=True)[0];row.update(track='kernel');out.write(json.dumps(row)+'\n')
   for kind in ['go-head','native-sealed']:
    for case in ['scan_full','scan_filtered','scan_page','follow']:
     if case=='follow' and kind=='go-head':continue
     db=fresh(fixtures/('typed-'+kind)/'records.db',WORK/'active');row=rows(lang,'bench',db,case,8,timed=True)[0];row.update(track='engine',fixture='typed-'+kind);out.write(json.dumps(row)+'\n')
 with (DATA/'memory.jsonl').open('w') as out:
  for run in range(1,4):
   for lang in (['go','rust'] if run%2 else ['rust','go']):
    db=fresh(fixtures/'typed-native-sealed/records.db',WORK/'active');p=command(['/usr/bin/time','-f','%M',BIN[lang],'--mode','memory','--db',db,'--case','scan_full','--iterations','64']);row=json.loads(p.stdout);row.update(pass_id=run,peak_rss_kib=int(p.stderr.strip().splitlines()[-1]));out.write(json.dumps(row)+'\n');out.flush()
 summary=[]
 keys=sorted({(r['track'],r.get('fixture',''),r['case']) for r in raw})
 for track,fixture,case in keys:
  samples={lang:[r['ns_per_op'] for r in raw if (r['track'],r.get('fixture',''),r['case'],r['language'])==(track,fixture,case,lang)] for lang in ['go','rust']}
  medians={k:statistics.median(v) for k,v in samples.items()};summary.append({'track':track,'fixture':fixture,'case':case,'go_ns':medians['go'],'rust_ns':medians['rust'],'ratio_go_over_rust':medians['go']/medians['rust'],'passes_ns':samples})
 write('summary.json',summary)
 storage=[]
 for db in sorted(fixtures.glob('*/records.db')):
  p=rows('go','storage',db)[0] if False else rows('go','storage',db)
  import sqlite3
  conn=sqlite3.connect(db);payload={table:conn.execute(f'select coalesce(sum(length(body)),0) from {table}').fetchone()[0] for table in ['heads','segments','blocks']};conn.close();storage.append({'fixture':db.parent.name,'file_bytes':db.stat().st_size,'payload_bytes':payload,'dbstat':p[0]})
 write('storage.json',storage)
 assert hashes()==info['source_sha256'],'source changed during collection'
 info['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat();write('environment.json',info)
 print(f'Complete: {len(summary)} comparisons',flush=True)
def main():
 p=argparse.ArgumentParser();p.add_argument('--prepare',action='store_true');p.add_argument('--measure',action='store_true');p.add_argument('--passes',type=int,default=6);a=p.parse_args()
 if a.prepare:prepare()
 if a.measure:measure(a.passes)
if __name__=='__main__':main()
