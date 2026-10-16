use crate::{analyzer::Analyzer,fts};
use rusqlite::{Connection,params};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet},time::Instant,path::Path};
#[derive(Clone,Deserialize,Serialize)]pub struct Doc {pub id:u64,#[serde(default)]pub key:String,pub title:String,pub body:String}
#[derive(Deserialize)]pub struct Query {pub key:String,pub text:String,pub relevant:Vec<u64>,pub judged:BTreeMap<String,i32>,#[serde(default)]pub language:String}
#[derive(Deserialize)]pub struct Corpus {pub docs:Vec<Doc>,pub queries:Vec<Query>}
fn us(t:Instant)->u64{t.elapsed().as_micros()as u64}
fn distribution(v:&mut[u64])->Value{v.sort_unstable();let p=|f:f64|if v.is_empty(){0}else{v[((v.len()-1)as f64*f).ceil()as usize]};json!({"n":v.len(),"p50_us":p(0.5),"p95_us":p(0.95),"p99_us":p(0.99),"max_us":p(1.)})}
fn memory()->Value{let s=std::fs::read_to_string("/proc/self/status").unwrap();let get=|k:&str|s.lines().find(|l|l.starts_with(k)).unwrap().split_whitespace().nth(1).unwrap().parse::<u64>().unwrap();json!({"rss_kib":get("VmRSS:"),"hwm_kib":get("VmHWM:")})}
fn cpu()->f64{unsafe{let mut r:libc::rusage=std::mem::zeroed();assert_eq!(libc::getrusage(libc::RUSAGE_SELF,&mut r),0);(r.ru_utime.tv_sec+r.ru_stime.tv_sec)as f64+(r.ru_utime.tv_usec+r.ru_stime.tv_usec)as f64/1e6}}
fn bytes(p:&str)->u64{std::fs::metadata(p).map(|m|m.len()).unwrap_or(0)}
fn directory(p:&Path)->u64{std::fs::read_dir(p).unwrap().map(|e|{let p=e.unwrap().path();if p.is_dir(){directory(&p)}else{p.metadata().unwrap().len()}}).sum()}
fn open(p:&str,mode:&str)->Connection{let c=Connection::open(p).unwrap();fts::register(&c,mode);c.execute_batch("pragma journal_mode=wal;pragma synchronous=FULL;pragma cache_size=-1024;pragma wal_autocheckpoint=0;pragma trusted_schema=off;pragma busy_timeout=10000;").unwrap();c}
fn schema(c:&Connection){c.execute_batch("create table documents(id integer primary key,key text unique,title text not null,body text not null,other integer default 0);").unwrap();}
fn create_index(c:&Connection,mode:&str){let tokenizer=if mode=="unicode"{"unicode61"}else{"tiny_auto_v1"};c.execute_batch(&format!("create virtual table search using fts5(title,body,content='documents',content_rowid='id',tokenize='{tokenizer}');
create trigger doc_i after insert on documents begin insert into search(rowid,title,body)values(new.id,new.title,new.body);end;
create trigger doc_d after delete on documents begin insert into search(search,rowid,title,body)values('delete',old.id,old.title,old.body);end;
create trigger doc_u after update on documents when old.id is not new.id or old.title is not new.title or old.body is not new.body begin
insert into search(search,rowid,title,body)values('delete',old.id,old.title,old.body);insert into search(rowid,title,body)values(new.id,new.title,new.body);end;")).unwrap();}
fn insert(c:&Connection,ds:&[Doc]){c.execute_batch("begin immediate").unwrap();{let mut s=c.prepare_cached("insert into documents(id,key,title,body) values(?1,?2,?3,?4)").unwrap();for d in ds{s.execute(params![d.id,d.key,d.title,d.body]).unwrap();}}c.execute_batch("commit").unwrap();}
fn search(c:&Connection,q:&str,mode:&str,all:bool)->Vec<u64>{let q=fts::query_string(q,mode);if q.is_empty(){return vec![]}
let sql=if all{"select rowid from search where search match ?1 order by rowid"}else{"select rowid from search where search match ?1 order by bm25(search),rowid limit 10"};c.prepare_cached(sql).unwrap().query_map([q],|r|r.get(0)).unwrap().map(Result::unwrap).collect()}
fn scores(q:&Query,ids:&[u64])->Value{let relevant:BTreeSet<_>=q.relevant.iter().copied().collect();let mut hit=0;let mut rr=0.;let mut dcg=0.;let mut judged=0;let mut negative=0;
for(i,id)in ids.iter().enumerate(){if q.judged.contains_key(&id.to_string()){judged+=1;}if q.judged.get(&id.to_string())==Some(&0){negative+=1;}if relevant.contains(id){hit+=1;if rr==0.{rr=1./(i+1)as f64;}dcg+=1./((i+2)as f64).log2();}}
let ideal=(0..relevant.len().min(10)).map(|i|1./((i+2)as f64).log2()).sum::<f64>();json!({"key":q.key,"language":q.language,"hits":hit,"returned":ids.len(),"relevant":relevant.len(),"recall10":if relevant.is_empty(){0.}else{hit as f64/relevant.len()as f64},"mrr10":rr,"ndcg10":if ideal==0.{0.}else{dcg/ideal},"judged_at10":judged,"known_negative_at10":negative,"ids":ids})}
fn aggregate(ss:&[Value])->Value{let avg=|k:&str|ss.iter().map(|s|s[k].as_f64().unwrap()).sum::<f64>()/ss.len()as f64;json!({"queries":ss.len(),"recall10":avg("recall10"),"mrr10":avg("mrr10"),"ndcg10":avg("ndcg10"),"known_negative_per_query":avg("known_negative_at10"),"judged_per_query":avg("judged_at10")})}
fn objects(c:&Connection)->Value{json!(c.prepare("select name,sum(pgsize) from dbstat group by name order by name").unwrap().query_map([],|r|Ok(json!({"name":r.get::<_,String>(0)?,"bytes":r.get::<_,u64>(1)?}))).unwrap().map(Result::unwrap).collect::<Vec<_>>())}
fn run(engine:&str,mode:&str,file:&str,scale:usize){
let idle=memory();let corpus:Corpus=serde_json::from_reader(std::fs::File::open(file).unwrap()).unwrap();let loaded=memory();let db=std::env::var("BENCH_DB").unwrap();let c=open(&db,mode);schema(&c);
let docs:Vec<_>=if scale==0{corpus.docs}else{(0..scale).map(|i|{let mut d=corpus.docs[i%corpus.docs.len()].clone();d.id=i as u64+1;d.key=format!("scale-{i}");d}).collect()};
let seed=Instant::now();for ds in docs.chunks(500){insert(&c,ds);}let seed_us=us(seed);c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();let source_objects=objects(&c);let source_bytes=bytes(&db);let base=memory();let cpu0=cpu();let t=Instant::now();
#[cfg(feature="text")]let mut external=None;
let dir=format!("{db}.tantivy");
if engine=="fts"{c.execute_batch("begin immediate").unwrap();create_index(&c,mode);c.execute_batch("insert into search(rowid,title,body)select id,title,body from documents order by id;commit;").unwrap();}
else{
#[cfg(feature="text")] {let mut e=crate::tant::TextIndex::new(&dir,mode);for d in &docs{e.add(d);}e.commit();external=Some(e);}
#[cfg(not(feature="text"))] panic!("text feature required");
}
let build_us=us(t);let build_memory=memory();let build_wal=bytes(&format!("{db}-wal"));let build_objects=objects(&c);let mut times=vec![];let mut samples=vec![];let mut set_checks=vec![];let begin_queries=Instant::now();
for q in &corpus.queries{let t=Instant::now();let ids=if engine=="fts"{search(&c,&q.text,mode,false)}else{
#[cfg(feature="text")] {external.as_ref().unwrap().search(&q.text,false)}
#[cfg(not(feature="text"))] {unreachable!()}
};times.push(us(t));samples.push(scores(q,&ids));}
let query_us=us(begin_queries);let query_memory=memory();
for q in corpus.queries.iter().take(12){let ids=if engine=="fts"{search(&c,&q.text,mode,true)}else{
#[cfg(feature="text")] {external.as_ref().unwrap().search(&q.text,true)}
#[cfg(not(feature="text"))] {unreachable!()}
};let h=ids.iter().fold(0xcbf29ce484222325u64,|h,x|(h^x).wrapping_mul(0x100000001b3));set_checks.push(json!({"key":q.key,"count":ids.len(),"hash":h}));}
let query_cpu=cpu()-cpu0;
// Same 100 source updates/commit; external native commit is explicitly separate.
c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();let mut write_times=vec![];let mut mutation=vec![];
for op in ["insert","update","unrelated","delete"]{let mut ws=vec![];let mut ext_us=0;for batch in 0..10{let t=Instant::now();c.execute_batch("begin immediate").unwrap();for i in 0..100{let original=&docs[(batch*100+i)%docs.len()];let mut d=original.clone();d.id=docs.len()as u64+1+(batch*100+i)as u64;d.key=format!("mutation-{}",d.id);match op{
"insert"=>{c.execute("insert into documents(id,key,title,body)values(?1,?2,?3,?4)",params![d.id,d.key,d.title,d.body]).unwrap();},
"update"=>{c.execute("update documents set title=title||' revised' where id=?1",[d.id]).unwrap();d.title.push_str(" revised");},
"unrelated"=>{c.execute("update documents set other=other+1 where id=?1",[d.id]).unwrap();},
_=>{c.execute("delete from documents where id=?1",[d.id]).unwrap();}}
#[cfg(feature="text")]if let Some(e)=&mut external{if op!="unrelated"{if op!="insert"{e.delete(d.id);}if op!="delete"{e.add(&d);}}}
}c.execute_batch("commit").unwrap();let sql_us=us(t);let te=Instant::now();
#[cfg(feature="text")]if let Some(e)=&mut external{if op!="unrelated"{e.commit();}}
ext_us+=us(te);ws.push(sql_us);write_times.push(sql_us);}
mutation.push(json!({"operation":op,"transactions":10,"rows_per_transaction":100,"source_commit":distribution(&mut ws),"external_commit_reload_us":ext_us,"wal_file_bytes":bytes(&format!("{db}-wal"))}));c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();}
let reopen=Instant::now();if engine=="fts"{c.execute("insert into search(search,rank)values('integrity-check',1)",[]).unwrap();let r=open(&db,mode);let q=&corpus.queries[0].text;assert_eq!(search(&c,q,mode,true),search(&r,q,mode,true));}else{
#[cfg(feature="text")] external.as_mut().unwrap().reopen(&dir);
}let reopen_us=us(reopen);let cpu_s=cpu()-cpu0;
println!("{}",json!({"engine":engine,"mode":mode,"rows":docs.len(),"source_seed_us":seed_us,"source_bytes":source_bytes,"source_objects":source_objects,"build_us":build_us,"build_rows_per_s":docs.len()as f64*1e6/build_us as f64,"build_wal_file_bytes":build_wal,"build_objects":build_objects,"external_bytes":if engine=="fts"{0}else{directory(Path::new(&dir))},"queries":distribution(&mut times),"query_per_s":corpus.queries.len()as f64*1e6/query_us as f64,"quality":aggregate(&samples),"query_samples":samples,"set_checks":set_checks,"mutations":mutation,"reopen_and_integrity_us":reopen_us,"idle_memory":idle,"loaded_memory":loaded,"source_memory":base,"build_memory":build_memory,"query_memory":query_memory,"end_memory":memory(),"cpu_through_queries_s":query_cpu,"cpu_total_s":cpu_s,"status":"pass"}));
}
fn analyze(mode:&str,file:&str){let c:Corpus=serde_json::from_reader(std::fs::File::open(file).unwrap()).unwrap();let idle=memory();let setup=Instant::now();let a=Analyzer::new(mode);let setup_us=us(setup);let ready=memory();let mut tokens=0;let mut bytes=0;let mut checksum=0;let cpu0=cpu();let t=Instant::now();for d in &c.docs {for text in [&d.title,&d.body]{let ts=a.tokens(text);tokens+=ts.len();bytes+=text.len();for x in ts{checksum^=x.text.len();}}}let elapsed=us(t);let probes=["Кошки спят на диване","Developers are running tests","我喜欢数据库系统","SQLite базы 数据库 running","Die Häuser stehen am Fluss","Les chevaux courent","реплицировать базу","こんにちは世界","ทุกสองสัปดาห์","Straße STRASSE","İSTANBUL"];
println!("{}",json!({"mode":mode,"setup_us":setup_us,"bytes":bytes,"tokens":tokens,"us":elapsed,"mib_per_s":bytes as f64*1e6/elapsed as f64/1048576.,"checksum":checksum,"cpu_s":cpu()-cpu0,"idle_memory":idle,"ready_memory":ready,"end_memory":memory(),"probes":probes.into_iter().map(|s|json!({"text":s,"tokens":a.tokens(s).into_iter().map(|t|json!({"text":t.text,"start":t.start,"end":t.end,"position":t.position})).collect::<Vec<_>>()})).collect::<Vec<_>>()}));}
pub fn main(){let a:Vec<_>=std::env::args().skip(1).collect();match a[0].as_str(){"run"=>run(&a[1],&a[2],&a[3],a.get(4).map(|s|s.parse().unwrap()).unwrap_or(0)),"analyze"=>analyze(&a[1],&a[2]),_=>panic!("run ENGINE MODE CORPUS [SCALE] | analyze MODE CORPUS")}}
