#!/usr/bin/env python3
"""Prove the failed-Take oracle rejects removal of native decode validation."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
from build import env,ROOT,WORK
def main():
 mutant=WORK/'guard-mutant'
 if mutant.exists():shutil.rmtree(mutant)
 shutil.copytree(ROOT/'rust',mutant,ignore=shutil.ignore_patterns('target'))
 p=mutant/'src/main.rs';source=p.read_text();needle='if bad && !matches!(v, Value::Integer(_)) {';assert source.count(needle)==1;p.write_text(source.replace(needle,'if false && bad && !matches!(v, Value::Integer(_)) {'))
 e=env();e['CARGO_TARGET_DIR']='/work/kv-guard-target';subprocess.run(['cargo','build','--release','--locked'],cwd=mutant,env=e,check=True)
 d=WORK/'guard-store'
 if d.exists():shutil.rmtree(d)
 d.mkdir();shutil.copyfile(WORK/'fixture/kv.db',d/'kv.db')
 tr=WORK/'guard.jsonl';tr.write_text(json.dumps({'Op':'take_bad','Bucket':'trace','Key':'bad'})+'\n'+json.dumps({'Op':'has','Bucket':'trace','Key':'bad'})+'\n')
 binary=Path(e['CARGO_TARGET_DIR'])/'release/tinystore-kv-native-bench';bad=subprocess.check_output([str(binary),'--mode','trace','--dir',str(d),'--trace',str(tr)],text=True)
 shutil.copyfile(WORK/'fixture/kv.db',d/'kv.db');good=subprocess.check_output([str(WORK/'rust-kv'),'--mode','trace','--dir',str(d),'--trace',str(tr)],text=True)
 b=[json.loads(x)for x in bad.splitlines()];g=[json.loads(x)for x in good.splitlines()];assert b[0]['error'] is None and not b[1]['found'];assert g[0]['error']['code']=='corrupt' and g[1]['found']
 out={'guard':'failed Take decode must rollback cell deletion','mutation':'disable typed integer decode validation in a temporary copy; original source unchanged','mutated_source_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'mutated_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'broken_transcript':b,'restored_transcript':g,'oracle_rejected_mutation':True}
 (WORK/'guard-proof.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
