#!/usr/bin/env python3
"""Prove malformed selected fields cannot be deferred beyond a returned Row."""
import hashlib,json,os,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent;path=ROOT/'rust/src/codec/optimized.rs';original=path.read_bytes();needle=b'chunk.ensure_all()?';assert original.count(needle)==1
env=os.environ.copy();env.update(CARGO_TARGET_DIR='/work/records-opt-target',CARGO_HOME='/work/cargo',RUSTUP_HOME='/work/rustup',SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(ROOT.parent/'sqlite-bench/generated/lib'),SQLITE3_INCLUDE_DIR=str(ROOT.parent/'sqlite-bench/generated/sqlite-amalgamation-3530400'));env['PATH']='/work/cargo/bin:'+env['PATH']
command=['cargo','test','--release','--locked','--offline','codec::optimized::tests::returned_fields_validate_before_return']
try:
 path.write_bytes(original.replace(needle,b'if false {chunk.ensure_all()?;}'))
 broken=subprocess.run(command,cwd=ROOT/'rust',env=env,text=True,capture_output=True)
 assert broken.returncode!=0 and 'selected record deferred malformed body validation' in broken.stdout,broken
finally:path.write_bytes(original)
restored=subprocess.run(command,cwd=ROOT/'rust',env=env,text=True,capture_output=True);assert restored.returncode==0,restored;assert path.read_bytes()==original
def clean(p):return {'exit_code':p.returncode,'stdout':p.stdout.replace('/src','<repo>').replace('/work','<tmp>'),'stderr':p.stderr.replace('/src','<repo>').replace('/work','<tmp>')}
data={'guard':'all returned fields validated before returning an owning shared row','source_sha256_before':hashlib.sha256(original).hexdigest(),'source_sha256_after':hashlib.sha256(path.read_bytes()).hexdigest(),'broken_implementation':clean(broken),'restored_implementation':clean(restored)}
(ROOT.parent/'reports/data/records-optimization-2026-10-09/guard-proof.json').write_text(json.dumps(data,indent=2)+'\n');print('Returned-field guard mutation failed as expected; source restored and passed')
