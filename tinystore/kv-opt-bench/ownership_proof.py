#!/usr/bin/env python3
"""A copied-input SetEntry mutant must fail the borrow/ownership regression."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
from build import env,ROOT,WORK
def main():
 d=WORK/'ownership-mutant'
 if d.exists():shutil.rmtree(d)
 shutil.copytree(ROOT/'rust',d,ignore=shutil.ignore_patterns('target'));p=d/'src/main.rs';s=p.read_text();old='Ok(WrittenEntry {\n            key,\n            value,\n';assert s.count(old)==1;s=s.replace(old,'Ok(WrittenEntry {\n            key,\n            value:Box::leak(Box::new(value.clone())),\n');p.write_text(s)
 e=env();e['CARGO_TARGET_DIR']='/work/kv-opt-ownership-target';subprocess.run(['cargo','build','--release','--locked'],cwd=d,env=e,check=True)
 store=WORK/'ownership-store'
 if store.exists():shutil.rmtree(store)
 store.mkdir();shutil.copyfile(WORK/'fixture/kv.db',store/'kv.db');binary=Path(e['CARGO_TARGET_DIR'])/'release/tinystore-kv-opt-bench';r=subprocess.run([str(binary),'--mode','write-contract','--dir',str(store)],text=True,capture_output=True);assert r.returncode!=0 and 'SetEntry must borrow original input' in r.stderr
 out={'mutation':'temporary mutant leaks a deep-cloned SetEntry value instead of borrowing caller input','mutated_source_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'mutated_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'exit_code':r.returncode,'stderr':r.stderr.replace(str(d),'<mutant>'),'ownership_regression_rejected_mutation':True};(WORK/'ownership-proof.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
