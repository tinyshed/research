#!/usr/bin/env python3
"""Cross-reader contracts, cold corruption probes and ownership verification."""
import hashlib,json,os,shutil,sqlite3,subprocess,zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parent;WORK=Path('/work/records-opt');DATA=ROOT.parent/'reports/data/records-optimization-2026-10-09';OLD=ROOT.parent/'records-native-bench';FIXTURES=Path('/work/records-native/fixtures')
env=os.environ.copy();env.update(GOWORK='off',CGO_ENABLED='0',GOMAXPROCS='1')
variants=['previous','opt','prune_off','heap_off','sharing_off','cache_off','cache_raw','strict']
def call(label,mode,db,case=None,check=True,extra=()):
 args=[str(WORK/('go-records' if label=='go' else 'rust-records')),'--mode',mode,'--db',str(db)]
 if label!='go':args+=['--variant',label]
 if case:args+=['--case',case]
 args+=list(map(str,extra))
 p=subprocess.run(args,env=env,text=True,capture_output=True)
 if check:assert p.returncode==0,(args,p.stdout,p.stderr)
 return p
def rows(*args,**kwargs):return [json.loads(s) for s in call(*args,**kwargs).stdout.splitlines()]
def fresh(source):
 path=WORK/'verification'
 if path.exists():shutil.rmtree(path)
 path.mkdir();shutil.copyfile(source,path/'records.db');return path/'records.db'
def normalize_walk(v):return [{k:x for k,x in r.items() if k not in ['batches','expired']} for r in v]
def mutate_column(body,slot):
 b=bytearray(body)
 def uv(at):
  v=shift=0
  while True:
   x=b[at];at+=1;v|=(x&127)<<shift
   if x<128:return v,at
   shift+=7
 _,at=uv(1);lengths=[]
 # These fixed compatible fixtures have all nine slots: time/name/shape/
 # context/level/trace/span/raw attrs/body. The schema is checked by readers.
 for _ in range(9):n,at=uv(at);lengths.append(n)
 start=at+sum(lengths[:slot]);assert lengths[slot]>0;b[start]=255;b[-4:]=zlib.crc32(b[:-4]).to_bytes(4,'little');return b
def main():
 DATA.mkdir(exist_ok=True,parents=True);out=[]
 oldmeta=json.loads((ROOT.parent/'reports/data/records-native-2026-10-08/environment.json').read_text())
 for relative,expected in oldmeta['source_sha256'].items():assert hashlib.sha256((OLD/relative).read_bytes()).hexdigest()==expected,relative
 for label,filename in [('go','go-records'),('rust','rust-records'),('rust-alloc','rust-alloc-records')]:assert hashlib.sha256((Path('/work/records-native')/filename).read_bytes()).hexdigest()==oldmeta['binary_sha256'][label],label
 for fixture in ['typed-go-head','typed-native-sealed','apache-go-head','apache-native-sealed','typed-late-native-sealed']:
  db=fresh(FIXTURES/fixture/'records.db');baseline=rows('go','verify',db)
  for label in variants:assert rows(label,'verify',db)==baseline,(fixture,label)
  if fixture.startswith('typed'):
   walks=normalize_walk(rows('go','verify-walk',db))
   for label in ['previous','opt','strict','cache_raw','cache_off']:assert normalize_walk(rows(label,'verify-walk',db))==walks,(fixture,label,walks,rows(label,'verify-walk',db))
  out.append({'fixture':fixture,'variants':variants,'results':baseline,'sha256':hashlib.sha256(db.read_bytes()).hexdigest()})
 for kind in ['go-head','native-sealed']:
  db=fresh(FIXTURES/('typed-'+kind)/'records.db')
  for case in ['scan_full','scan_page','follow']:
   for label in ['go','previous','opt','sharing_off']:
    v=rows(label,'lifetime',db,case);out.append({'fixture':kind,'variant':label,'case':case,'lifetime':v})
 db=fresh(FIXTURES/'typed-native-sealed/records.db');out.append({'cache_guards':rows('opt','cache-guards',db)});rows('opt','guards',db)
 for segment,cursor_row in [(0,0),(1,1023),(1,1024),(1,2048),(1,9999),(2,2048),(3,0)]:
  extra=['--segment',segment,'--row',cursor_row,'--limit',127];expected=rows('go','cursor',db,extra=extra)
  for label in ['previous','opt','cache_raw','cache_off']:assert rows(label,'cursor',db,extra=extra)==expected,(segment,cursor_row,label)
  out.append({'cursor_probe':[segment,cursor_row],'expected':expected})
 for fixture,table in [('typed-go-head','heads'),('typed-native-sealed','blocks')]:
  for mutation in ['crc','index','body_column','context_column']:
   if table=='heads' and mutation.endswith('column'):continue
   db=fresh(FIXTURES/fixture/'records.db');conn=sqlite3.connect(db);id,body=conn.execute(f'select id,body from {table} order by id limit 1').fetchone()
   if mutation=='index':conn.execute(f'update {table} set count=count-1 where id=?',(id,))
   else:
    b=bytearray(body)
    if mutation=='crc':b[-1]^=1
    elif mutation=='body_column':b=mutate_column(body,8)
    else:b=mutate_column(body,3)
    conn.execute(f'update {table} set body=? where id=?',(b,id))
   conn.commit();conn.close()
   for case in ['scan_full','scan_no_context']:
    for label in ['go','previous','opt','strict']:
     p=call(label,'snapshot',db,case,False);rejected=p.returncode!=0
     expected=not(mutation=='body_column' and case=='scan_no_context' and label in ['go','opt'])
     assert rejected==expected,(fixture,mutation,case,label,p.stdout,p.stderr)
     out.append({'fixture':fixture,'mutation':mutation,'case':case,'variant':label,'rejected':rejected,'error':p.stderr.splitlines()[0] if p.stderr else ''})
 (DATA/'verification.json').write_text(json.dumps(out,indent=2)+'\n');print('Verified all variants, archive preservation, page/Follow walks, lifetime/thread tests and consumed-column corruption policy')
if __name__=='__main__':main()
