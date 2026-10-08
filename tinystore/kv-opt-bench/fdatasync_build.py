#!/usr/bin/env python3
"""Isolated Linux fdatasync ablation, same frozen Rust source and SQLite source."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
from build import ROOT,WORK,files,env
SYNC=Path('/work/kv-opt-fdatasync')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def parse_readback(binary,db,role):
 text=subprocess.check_output([str(binary),str(db),role],text=True);out={'compile_options':[]}
 for line in text.splitlines():
  k,v=line.split('\t',1)
  if k=='compile_option':out['compile_options'].append(v)
  else:out[k]=int(v)if v.lstrip('-').isdigit()else v
 return out
def main():
 SYNC.mkdir(parents=True,exist_ok=True);generated=ROOT.parent/'sqlite-bench/generated';base=json.loads((generated/'native-build.json').read_text());c=generated/'sqlite-amalgamation-3530400/sqlite3.c';h=generated/'sqlite-amalgamation-3530400/sqlite3.h';assert sha(c)==base['sqlite3_c_sha256'];assert sha(h)==base['sqlite3_h_sha256'];flags=base['flags']+['-DHAVE_FDATASYNC=1'];lib=SYNC/'lib';lib.mkdir(exist_ok=True);subprocess.run(['cc',*flags,'-c',str(c),'-o',str(lib/'sqlite3.o')],check=True);subprocess.run(['ar','rcs',str(lib/'libsqlite3.a'),str(lib/'sqlite3.o')],check=True)
 crate=SYNC/'crate'
 if crate.exists():shutil.rmtree(crate)
 shutil.copytree(ROOT/'rust',crate,ignore=shutil.ignore_patterns('target'));e=env();e.update(CARGO_TARGET_DIR='/work/kv-opt-fdatasync-target',SQLITE3_LIB_DIR=str(lib));subprocess.run(['cargo','test','--locked'],cwd=crate,env=e,check=True);subprocess.run(['cargo','build','--release','--locked'],cwd=crate,env=e,check=True);shutil.copy2(Path(e['CARGO_TARGET_DIR'])/'release/tinystore-kv-opt-bench',SYNC/'rust-kv');shutil.copy2(WORK/'go-kv',SYNC/'go-kv')
 assert files(crate)==files(ROOT/'rust')
 for label,archive in [('fsync',generated/'lib/libsqlite3.a'),('fdatasync',lib/'libsqlite3.a')]:subprocess.run(['cc','-O2','-I',str(h.parent),str(ROOT/'sync-readback.c'),str(archive),'-lm','-ldl','-pthread','-o',str(SYNC/('readback-'+label))],check=True)
 subprocess.run(['go','build','-o',str(SYNC/'go-readback'),'./readback'],cwd=ROOT/'go',env=e,check=True)
 # Run the exact full oracle against the candidate, with output isolation.
 import check
 check.WORK=SYNC;check.ROOT=SYNC/'check-source';check.ROOT.mkdir(exist_ok=True);os.environ['KV_TUNING']='3';check.main()
 config={}
 for label in ['fsync','fdatasync']:config[label]={role:parse_readback(SYNC/('readback-'+label),SYNC/'fixture/kv.db',role)for role in ['writer','reader']}
 config['go']=json.loads(subprocess.check_output([str(SYNC/'go-readback'),str(SYNC/'fixture')],text=True));assert config['fsync']==config['fdatasync'];assert config['fsync']['writer']['synchronous']==2 and config['fsync']['writer']['wal_autocheckpoint']==1000;assert config['go']['writer']['synchronous']==2 and config['go']['writer']['wal_autocheckpoint']==1000
 candidate=json.loads(subprocess.check_output([str(SYNC/'rust-kv'),'--tuning','3','--mode','metadata','--dir',str(SYNC/'fixture')],text=True));baseline=json.loads(subprocess.check_output([str(WORK/'rust-kv'),'--tuning','3','--mode','metadata','--dir',str(SYNC/'fixture')],text=True));assert candidate==baseline
 report={'isolated_platform_ablation':'Linux HAVE_FDATASYNC=1, no other native build flag changed','flags':flags,'added_flags':['-DHAVE_FDATASYNC=1'],'sqlite_source_sha256':sha(c),'sqlite_header_sha256':sha(h),'archive_sha256':sha(lib/'libsqlite3.a'),'baseline_archive_sha256':sha(generated/'lib/libsqlite3.a'),'binary_sha256':sha(SYNC/'rust-kv'),'baseline_binary_sha256':sha(WORK/'rust-kv'),'crate_files':files(crate),'source_equal':files(crate)==files(ROOT/'rust'),'metadata_equal':candidate==baseline,'readback':config,'candidate_metadata':candidate,'source_evidence':{'file':'sqlite3.c','line_range':'43928–44041','default':'fdatasync aliases fsync unless HAVE_FDATASYNC is true','linux_selection':'full_fsync calls fdatasync; sync frequency and synchronous=FULL unchanged'},'compiler':subprocess.check_output(['cc','--version'],text=True).splitlines()[0]};(SYNC/'build.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
