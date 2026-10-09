#!/usr/bin/env python3
"""Explicit staged layout experiment; no heavy work at import time.

All files under --work are disposable Linux-volume fixtures. Raw reports carry
relative dataset/variant names, never host paths. Never mutate an input DB.
"""
import argparse,hashlib,json,os,platform,shutil,sqlite3,statistics,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent
def call(args):
 p=subprocess.run([str(x) for x in args],text=True,capture_output=True)
 if p.returncode:raise RuntimeError(f"{args}\n{p.stdout}\n{p.stderr}")
 return json.loads(p.stdout)
def sha(path):
 h=hashlib.sha256()
 with open(path,'rb') as f:
  while b:=f.read(1<<20):h.update(b)
 return h.hexdigest()
def physical(db,go):
 c=sqlite3.connect(f'file:{db}?immutable=1',uri=True)
 names=['name','type','pages','bytes','cell_payload','unused','cells','max_cell_payload']
 objects=[dict(zip(names,r)) for r in c.execute('select name,pagetype,count(*),sum(pgsize),sum(payload),sum(unused),sum(ncell),max(mx_payload) from dbstat group by name,pagetype order by name,pagetype')]
 audit=call([go,'--mode','physical','--db',db]);expected={r['Name']:r['Pages'] for r in audit}
 actual={}
 for r in objects:actual[r['name']]=actual.get(r['name'],0)+r['pages']
 assert actual==expected,(actual,expected)
 page=c.execute('pragma page_size').fetchone()[0];free=c.execute('pragma freelist_count').fetchone()[0]
 assert sum(r['bytes'] for r in objects)+page*free==db.stat().st_size
 c.close();return {'reader_sqlite_version':sqlite3.sqlite_version,'objects':objects,'go_dbstat_audit':audit,'file_sha256':sha(db),'file_bytes':db.stat().st_size,'freelist_bytes':page*free}
def write(path,obj):path.write_text(json.dumps(obj,indent=2)+'\n')
def main():
 p=argparse.ArgumentParser();p.add_argument('stage',choices=['baseline','matrix','pilot','timing','retention','publication','crossread','rss','provenance']);p.add_argument('--work',default='/work/metrics-layout');p.add_argument('--input',default='/work/metrics-storage-input');p.add_argument('--raw',default=str(ROOT.parent/'reports/data/metrics-layout-2026-10-09'));p.add_argument('--datasets',nargs='*',default=['tsbs','regular','irregular','nonsparse','edge']);p.add_argument('--variants',nargs='*');a=p.parse_args()
 work=Path(a.work);inputs=Path(a.input);raw=Path(a.raw);raw.mkdir(parents=True,exist_ok=True);binary=work/'layout';go=work/'go-input'
 if a.stage=='provenance':
  result={'utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'uname':platform.uname()._asdict(),'go':subprocess.check_output(['go','version'],text=True).strip(),'rustc':subprocess.check_output(['/work/cargo/bin/rustc','--version','--verbose'],text=True).strip(),'python':platform.python_version(),'sqlite_scan_version':sqlite3.sqlite_version,'go_binary_sha256':sha(go),'rust_binary_sha256':sha(binary),'production_source':'e307c48a40126aad0e2873b6bf3aaedef8115483','harness_source_hashes':{str(f.relative_to(ROOT)):sha(f) for f in ROOT.rglob('*') if f.is_file() and f.suffix in ['.rs','.go','.py','.toml','.lock'] and '/target/' not in str(f)},'native_sqlite':json.loads((ROOT.parent/'sqlite-bench/generated/native-build.json').read_text()),'cgroup_cpu_max':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'cgroup_cpuset':Path('/sys/fs/cgroup/cpuset.cpus.effective').read_text().strip(),'cpu':subprocess.check_output(['lscpu'],text=True)}
  write(raw/'provenance.json',result);return
 if a.stage=='baseline':
  for name in a.datasets:
   d=inputs/name;db=d/'metrics.db'
   if not db.exists():
    args=[go,'--out',d,'--dataset',name]
    if name in ['tsbs','alibaba']:args+=['--corpus',inputs/'corpus'/f'{name}-series.jsonl']
    elif name=='edge':args+=['--series','8','--samples','721']
    manifest=call(args)
   else:manifest=json.loads((d/'manifest.json').read_text())
   checked=call([binary,'--mode','verify','--db',db]);exported=call([binary,'--mode','export','--db',db,'--out',d]);census=call([binary,'--mode','census','--db',db]);census['result']['physical']=physical(db,go)
   write(raw/f'{name}-baseline.json',{'manifest':manifest,'verify':checked,'export':exported,'census':census});print(f'BASELINE {name}: {manifest["sample_count"]} samples {db.stat().st_size}B',flush=True)
  return
 if a.stage=='matrix':
  configs=[('exact',4096,0,32,16,0),('rowid',4096,1,32,16,0),('page1024',1024,0,32,16,0),('page2048',2048,0,32,16,0),('native_control',4096,0,32,16,1),('cap8',4096,0,8,16,1),('cap16',4096,0,16,16,1),('inline0',4096,0,32,0,1),('inline32',4096,0,32,32,1),('inline64',4096,0,32,64,1),('inline128',4096,0,32,128,1),('rowid_native_control',4096,1,32,16,1),('rowid_inline0',4096,1,32,0,1),('rowid_inline32',4096,1,32,32,1),('rowid_inline64',4096,1,32,64,1),('rowid_inline128',4096,1,32,128,1),('rowid_page1024',1024,1,32,16,0),('rowid_page2048',2048,1,32,16,0),('rowid_cap16',4096,1,16,16,1),('rowid_cap16_inline64',4096,1,16,64,1),('rowid_cap16_inline128',4096,1,16,128,1)]
  for name in a.datasets:
   for variant,page,rowid,cap,inline,reencode in configs:
    if a.variants and variant not in a.variants:continue
    dest=work/name/f'{variant}.db';manifest=inputs/name/'manifest.json'
    t=time.monotonic_ns();row=call([binary,'--mode','rewrite','--source',inputs/name/'metrics.db','--db',dest,'--page',page,'--rowid',rowid,'--cap',cap,'--inline',inline,'--reencode',reencode]);row['build_wall_ns']=time.monotonic_ns()-t
    row['config']={'variant':variant,'page':page,'rowid':rowid,'cap':cap,'inline':inline,'reencode':reencode};row['verify']=call([binary,'--mode','verify','--db',dest,'--manifest',manifest]);row['result']['physical']=physical(dest,go)
    if json.loads(manifest.read_text())['dataset']!='edge':row['summary_check']=call([binary,'--mode','check_summary','--db',dest,'--manifest',manifest])
    write(raw/f'{name}-{variant}.json',row);print(f'MATRIX {name}/{variant}: {dest.stat().st_size}B',flush=True)
  return
 if a.stage=='publication':
  variants=a.variants or ['exact','rowid'];rows=[]
  for name in a.datasets:
   for passno in range(6):
    for variant in (variants if passno%2==0 else list(reversed(variants))):
     cfg=json.loads((raw/f'{name}-{variant}.json').read_text())['config'];dest=work/name/f'publication-{variant}-{passno}.db'
     row=call([binary,'--mode','rewrite','--source',inputs/name/'metrics.db','--db',dest,'--page',cfg['page'],'--rowid',cfg['rowid'],'--cap',cfg['cap'],'--inline',cfg['inline'],'--reencode',cfg['reencode']]);row.update(dataset=name,variant=variant,pass_no=passno);rows.append(row);dest.unlink()
   print(f'PUBLICATION {name} six balanced passes',flush=True)
  write(raw/'publication.json',rows);return
 if a.stage=='crossread':
  variants=a.variants or ['rowid','cap16','page1024','native_control'];rows=[]
  for name in a.datasets:
   for variant in variants:
    d=work/'go-crossread'/name/variant;d.mkdir(parents=True,exist_ok=True);shutil.copyfile(work/name/f'{variant}.db',d/'metrics.db');shutil.copyfile(inputs/name/'manifest.json',d/'manifest.json')
    checked=call([go,'--mode','verify','--out',d]);rows.append({'dataset':name,'variant':variant,'go_public_api_bitwise_verified':True,'samples':checked['sample_count'],'verify_ns':checked['verify_ns']})
  write(raw/'go-crossread.json',rows);return
 if a.stage=='rss':
  variants=a.variants or ['exact','rowid'];rows=[]
  for name in a.datasets:
   for variant in variants:
    for case in ['point','range','whole_summary','cut_summary']:
     peak=work/'rss.txt';row=call(['/usr/bin/time','-f','%M','-o',peak,binary,'--mode','bench','--db',work/name/f'{variant}.db','--manifest',inputs/name/'manifest.json','--case',case,'--iterations','512']);row.update(dataset=name,variant=variant,peak_rss_kib=int(peak.read_text().strip()));rows.append(row)
  write(raw/'rss.json',rows);return
 if a.stage=='pilot':
  variants=a.variants or ['exact','rowid'];rows=[];counts={};projected=0
  for name in a.datasets:
   counts[name]={}
   for case in ['point','range','whole_summary','cut_summary']:
    subset=[]
    for variant in variants:
     row=call([binary,'--mode','bench','--db',work/name/f'{variant}.db','--manifest',inputs/name/'manifest.json','--case',case,'--iterations','32']);row.update(dataset=name,variant=variant);rows.append(row);subset.append(row)
    assert len({r['result']['result_sha256'] for r in subset})==1
    slow=max(r['result']['elapsed_ns']/32 for r in subset)
    count=max(1,min(4096,int(120_000_000/max(1,slow)),max(1,int(1_000_000_000/max(1,slow)))))
    counts[name][case]=count;projected+=sum(r['result']['elapsed_ns']/32*count*6 for r in subset)/1e9
  write(raw/'pilots.json',{'rows':rows,'counts':counts,'projected_read_seconds':projected});print(json.dumps({'projected_read_seconds':projected,'counts':counts}),flush=True);return
 if a.stage=='timing':
  variants=a.variants or ['exact','rowid','page2048'];rows=[]
  for name in a.datasets:
   if name=='edge':continue
   for case in ['point','range','whole_summary','cut_summary']:
    # Slowest pilot controls one shared count. Handles are new each pass; OS
    # file cache is warm. This is never described as a cold-device experiment.
    count=json.loads((raw/'pilots.json').read_text())['counts'][name][case]
    for passno in range(6):
     order=variants if passno%2==0 else list(reversed(variants))
     for variant in order:
      row=call([binary,'--mode','bench','--db',work/name/f'{variant}.db','--manifest',inputs/name/'manifest.json','--case',case,'--iterations',count]);row.update(dataset=name,variant=variant,pass_no=passno,utc=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()));rows.append(row)
     print(f'TIMING {name}/{case} pass{passno+1}/6 count={count}',flush=True)
    subset=[r for r in rows if r['dataset']==name and r['result']['case']==case];assert len({r['result']['result_sha256'] for r in subset})==1
  write(raw/'timings.json',rows);return
 if a.stage=='retention':
  variants=a.variants or ['exact','rowid','page2048'];rows=[]
  for name in a.datasets:
   m=json.loads((inputs/name/'manifest.json').read_text());low=min(s['from'] for s in m['series']);high=max(s['to'] for s in m['series']);cutoff=low+(high-low)*3//5
   for passno in range(6):
    for variant in (variants if passno%2==0 else list(reversed(variants))):
     src=work/name/f'{variant}.db';dest=work/name/f'retain-{variant}-{passno}.db';shutil.copyfile(src,dest)
     row=call([binary,'--mode','retain','--db',dest,'--manifest',inputs/name/'manifest.json','--cutoff',cutoff]);row.update(dataset=name,variant=variant,pass_no=passno)
     rows.append(row)
     if passno!=0:dest.unlink()
   for row in [r for r in rows if r['dataset']==name and r['pass_no']==0]:
    dest=work/name/f'retain-{row["variant"]}-0.db';row['result']['physical']['physical']=physical(dest,go);row['vacuum']=call([binary,'--mode','vacuum','--db',dest]);row['vacuum']['result']['physical']=physical(dest,go);dest.unlink()
   print(f'RETENTION {name} cutoff={cutoff}',flush=True)
  write(raw/'retention.json',rows);return
if __name__=='__main__':main()
