#!/usr/bin/env python3
"""Retained semantic oracle: public Go fixtures, bidirectional cross-read/write."""
import base64,hashlib,json,os,shutil,sqlite3,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent;WORK=Path(os.environ.get('KV_WORK','/work/kv-opt'))
EPOCH=1800000000000
def call(binary,*args):return subprocess.check_output([str(WORK/binary),*(['--tuning',os.environ.get('KV_TUNING','0')]if binary.startswith('rust')else[]),*map(str,args)],text=True)
def run(binary,d,*args):return call(binary,'--dir',d,*args)
def b36(n):
 s='';a='0123456789abcdefghijklmnopqrstuvwxyz'
 while n:s=a[n%36]+s;n//=36
 return s
def raw(b):return {'Kind':2,'Bytes':base64.b64encode(b).decode(),'Int':0}
def clone(src,d):
 if d.exists():shutil.rmtree(d)
 d.mkdir();shutil.copyfile(src/'kv.db',d/'kv.db')
def snapshot(d):
 c=sqlite3.connect(d/'kv.db');out={}
 for table in ['buckets','meta','branches']:
  rows=c.execute('select * from '+table+' order by 1,2').fetchall();out[table]=repr(rows)
 rows=c.execute('select c.bucket,c.path,c.version,c.expires,c.value,c.spill,s.value from cells c left join spilled s on s.id=c.spill order by c.bucket,c.path').fetchall()
 out['cells_sha256']=hashlib.sha256(repr(rows).encode()).hexdigest();out['count']=len(rows);out['orphan_spills']=c.execute('select count(*)from spilled s where not exists(select 1 from cells c where c.spill=s.id)').fetchone()[0];c.close();return out
def commands(rev):
 t=[]
 def add(op,**kw):t.append({'Op':op,'Bucket':'trace',**kw})
 add('get',Bucket='shapes',Key='42');add('take_bad',Key='bad');add('has',Key='bad')
 add('set',Key='live',Value=raw(b'first'),Expiry=EPOCH+1000)
 add('get',Key='live');add('set',Key='live',Value=raw(b'conflict'),Version='1')
 add('set',Key='live',Value=raw(b'second'),Version=b36(rev+1))
 add('touch',Key='live',Expiry=EPOCH+2000,Version=b36(rev+2));add('get',Key='live')
 add('rollback_set',Key='rolled',Value=raw(b'rollback'));add('has',Key='rolled')
 add('set',Key='after-rollback',Value=raw(b'kept'))
 add('delete',Key='live',Version='1');add('take',Key='live',Version='1')
 add('take',Key='live',Version=b36(rev+2));add('has',Key='live')
 add('set',Key='live',Value=raw(b'reborn'),Expiry=EPOCH+1000);add('absent',Key='live',Value=raw(b'existing'))
 add('get',Key='live',Now=EPOCH+1000);add('has',Key='live');add('absent',Key='live',Value=raw(b'expired-reclaim'))
 add('scan',Limit=2);add('scan',Limit=2,After='bad');add('scan',Limit=1001);add('scan',Limit=-1,After='bad')
 add('set',Key='',Value=raw(b'invalid'));add('set',Key='x'*1024,Value=raw(b'invalid'))
 add('set',Key='zero\0key',Owners=['a\0b','42'],Value=raw(b'zero'))
 add('get',Key='zero\0key',Owners=['a\0b','42']);add('scan',Owners=['a\0b','42'],Limit=1)
 add('get',Bucket='shapes',Owners=['zero\0owner','深い','a','b','c','d','e'],Key='key\0x')
 add('clear',Bucket='clear',Owners=['parent']);add('get',Bucket='clear',Owners=['parent'],Key='k000001');add('scan',Bucket='clear',Owners=['parent'],Limit=2)
 add('set',Bucket='clear',Owners=['parent'],Key='k000001',Value=raw(b'new after clear'));add('get',Bucket='clear',Owners=['parent'],Key='k000001')
 add('clear',Bucket='small',Owners=['parent']);add('scan',Bucket='small',Owners=['parent'])
 add('counter_add',Bucket='counters',Key='one',N=9223372036854775807)
 add('counter_add',Bucket='counters',Key='one',N=1);add('counter_get',Bucket='counters',Key='one')
 add('counter_max',Bucket='counters',Key='negative',N=-1);add('counter_max',Bucket='counters',Key='negative',N=42)
 add('counter_get',Bucket='counters',Key='one',Now=EPOCH+2000)
 add('counter_add',Bucket='counters',Key='one',N=-5);add('counter_max',Bucket='counters',Key='one',N=-6)
 add('maintain');add('get',Bucket='clear',Owners=['parent'],Key='k000001');add('scan',Limit=100)
 add('set',Key='default-ttl',Value=raw(b'initial'),DefaultTTL=1000)
 add('set',Key='default-ttl',Value=raw(b'overwrite'),DefaultTTL=9000,Now=EPOCH+2200)
 add('get',Key='default-ttl',Now=EPOCH+3000)
 add('set',Key='default-ttl',Value=raw(b'recreate'),DefaultTTL=9000)
 add('get',Key='default-ttl');add('delete',Key='default-ttl');add('set',Key='default-ttl',Value=raw(b'redelete'),DefaultTTL=1000)
 return t
def main():
 WORK.mkdir(exist_ok=True,parents=True);fixture=WORK/'fixture';
 if fixture.exists():shutil.rmtree(fixture)
 print(run('go-kv',fixture,'--mode','fixture'),end='')
 c=sqlite3.connect(fixture/'kv.db');rev=c.execute("select value from meta where name='revision'").fetchone()[0];c.close()
 trace=ROOT/'trace.jsonl';trace.write_text(''.join(json.dumps(c,ensure_ascii=False)+'\n' for c in commands(rev)))
 gk=json.loads(call('go-kv','--mode','kernel-check'));rk=json.loads(call('rust-kv','--mode','kernel-check'));assert gk==rk,(gk,rk);assert gk['ttl_guard']==64
 g=WORK/'verify-go';r=WORK/'verify-rust';clone(fixture,g);clone(fixture,r)
 gt=run('go-kv',g,'--mode','trace','--trace',trace);rt=run('rust-kv',r,'--mode','trace','--trace',trace)
 (WORK/'trace-go.jsonl').write_text(gt);(WORK/'trace-rust.jsonl').write_text(rt)
 ga=[json.loads(l)for l in gt.splitlines()];ra=[json.loads(l)for l in rt.splitlines()]
 for i,(a,b)in enumerate(zip(ga,ra)):
  assert a==b,(i,commands(rev)[i],a,b)
 ttl=ga[-7:];assert ttl[0]['entry']['expiry']==EPOCH+3000 and ttl[1]['entry']['expiry']==EPOCH+3000 and not ttl[2]['found'];assert ttl[3]['entry']['expiry']==EPOCH+12000 and ttl[6]['entry']['expiry']==EPOCH+4000;assert int(ttl[6]['entry']['version'],36)>int(ttl[3]['entry']['version'],36)
 assert len(ga)==len(ra);gs=snapshot(g);rs=snapshot(r);assert gs==rs,(gs,rs);assert gs['orphan_spills']==0
 # Native-written file reopened through Go typed API, including signaling NaNs.
 typed_dir=WORK/'native-rewrite';clone(fixture,typed_dir);native_typed=json.loads(run('rust-kv',typed_dir,'--mode','verify'));typed=json.loads(run('go-kv',typed_dir,'--mode','verify'))
 # Reopen revisions after deleted/expired values and after an intentional rollback.
 reopen=ROOT/'reopen.jsonl';reopen.write_text(json.dumps({'Op':'set','Bucket':'trace','Key':'reopen','Value':raw(b'reopen')})+'\n')
 gr=json.loads(run('go-kv',g,'--mode','trace','--trace',reopen));rr=json.loads(run('rust-kv',r,'--mode','trace','--trace',reopen));assert gr==rr,(gr,rr)
 # A missing spilled row must be corruption; failed Take must leave the cell.
 corrupt=[]
 for binary in ['go-kv','rust-kv']:
  d=WORK/('corrupt-'+binary);clone(fixture,d);c=sqlite3.connect(d/'kv.db');c.execute('delete from spilled where id=(select spill from cells where bucket=(select id from buckets where name=\'spill\') limit 1)');c.commit();before=c.execute('select count(*)from cells').fetchone()[0];c.close()
  p=ROOT/'corrupt.jsonl';p.write_text(''.join(json.dumps({'Op':op,'Bucket':'spill','Owners':['tenant','42'],'Key':'k000000'})+'\n'for op in ['get','take']))
  tr=run(binary,d,'--mode','trace','--trace',p);rows=[json.loads(x)for x in tr.splitlines()];assert all(x['error']['code']=='corrupt' for x in rows),rows
  c=sqlite3.connect(d/'kv.db');assert c.execute('select count(*)from cells').fetchone()[0]==before;c.close();corrupt.append(rows)
 assert corrupt[0]==corrupt[1],corrupt
 (WORK/'corruption-transcripts.json').write_text(json.dumps(corrupt,indent=2)+'\n')
 # Scan's 4 MiB materialization cap uses whole owning values, then resumes.
 p=ROOT/'page-bound.jsonl';cmds=[{'Op':'set','Bucket':'page-bound','Key':str(i),'Value':raw(b'p'*(1<<20))}for i in range(5)]+[{'Op':'scan','Bucket':'page-bound','Limit':100},{'Op':'scan','Bucket':'page-bound','Limit':100,'After':'3'},{'Op':'set','Bucket':'page-bound','Key':'oversize','Value':raw(b'p'*((1<<20)+1))}];p.write_text(''.join(json.dumps(x)+'\n'for x in cmds))
 pages=[]
 for binary in ['go-kv','rust-kv']:
  d=WORK/('pages-'+binary);clone(fixture,d);rows=[json.loads(x)for x in run(binary,d,'--mode','trace','--trace',p).splitlines()];assert len(rows[-3]['entries'])==4 and rows[-3]['more'] and rows[-3]['after']=='3';assert len(rows[-2]['entries'])==1;assert rows[-1]['error']['code']=='limit';pages.append(rows)
 assert pages[0]==pages[1]
 def compact(x):
  if isinstance(x,list):return [compact(y)for y in x]
  if isinstance(x,dict):return {k:({'base64_length':len(v),'sha256':hashlib.sha256(v.encode()).hexdigest()}if k=='Bytes' and isinstance(v,str) and len(v)>1000 else compact(v))for k,v in x.items()}
  return x
 (WORK/'page-bound-transcripts.json').write_text(json.dumps(compact(pages),indent=2)+'\n')
 # Large generated input stays on /work; retain its deterministic recipe, not base64.
 p.unlink()
 result={'source':'e307c48a40126aad0e2873b6bf3aaedef8115483','fixture_revision':rev,'semantic_commands':len(ga),'transcripts_equal':True,'physical_snapshot_equal':True,'snapshot':gs,'reopen_equal':True,'typed_cross_read':typed,'native_typed_rewrite':native_typed,'missing_spill_take_rollback':True,'four_mib_page_and_one_mib_value_bounds':True,'kernel_truth_tables':gk,'fixture_sha256':hashlib.sha256((fixture/'kv.db').read_bytes()).hexdigest()}
 result['tuning']=os.environ.get('KV_TUNING','0')
 no_result=ROOT/'no-result.jsonl';no_result.write_text(''.join(json.dumps({**c,'Op':'set_void'if c['Op']=='set'else c['Op']},ensure_ascii=False)+'\n'for c in commands(rev)))
 outputs=[];states=[]
 for binary in ['go-kv','rust-kv']:
  d=WORK/('no-result-'+binary);clone(fixture,d);text=run(binary,d,'--mode','trace','--trace',no_result);(WORK/(binary+'-no-result.jsonl')).write_text(text);outputs.append([json.loads(x)for x in text.splitlines()]);states.append(snapshot(d))
 assert outputs[0]==outputs[1];assert states[0]==states[1]==gs
 d=WORK/'write-contract';clone(fixture,d);result['write_contract']=json.loads(run('rust-kv',d,'--mode','write-contract'));result['no_result_full_trace_and_snapshot_equal']=True
 (WORK/('correctness-tuning'+os.environ.get('KV_TUNING','0')+'.json')).write_text(json.dumps(result,indent=2)+'\n');(WORK/'correctness.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
