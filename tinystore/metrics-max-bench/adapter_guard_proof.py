#!/usr/bin/env python3
"""Prove the reprepare regression detects stale counts, then restore the fork."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parent
WORK=Path(os.environ.get('ADAPTER_WORK','/work'))
DATA=ROOT.parent/'reports/data/rusqlite-fork-2026-10-08'

def main():
    source=WORK/'rusqlite-count/src/raw_statement.rs';original=source.read_bytes()
    checksum=hashlib.sha256(original).hexdigest()
    assert checksum==json.loads((DATA/'build.json').read_text())['count']['patched_sha256']['raw_statement.rs']
    env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(WORK/'test-target-count'),SQLITE3_STATIC='1',
        SQLITE3_LIB_DIR=str(ROOT.parent/'sqlite-bench/generated/lib'),
        SQLITE3_INCLUDE_DIR=str(ROOT.parent/'sqlite-bench/generated/sqlite-amalgamation-3530400'))
    env['PATH']=str(WORK/'cargo/bin')+':'+env['PATH']
    args=['cargo','test','--lib','--manifest-path',str(WORK/'rusqlite-count/Cargo.toml'),
          '--no-default-features','--features','cache,limits,modern_sqlite,hooks',
          'schema_reprepare_refreshes_count_and_keeps_invalid_index_errors']
    text=original.decode();needle='        self.row_column_count.set(None);'
    assert text.count(needle)==2
    try:
        source.write_text(text.replace(needle,'        // Deliberately broken for guard validation: no invalidation.'))
        broken=subprocess.run(args,env=env,text=True,capture_output=True)
        assert broken.returncode==101 and '1 failed' in broken.stdout and 'assertion' in broken.stdout
    finally:source.write_bytes(original)
    restored=subprocess.run(args,env=env,text=True,capture_output=True)
    assert restored.returncode==0 and '1 passed' in restored.stdout
    assert hashlib.sha256(source.read_bytes()).hexdigest()==checksum
    for label,result in [('broken',broken),('restored',restored)]:
        output=result.stdout+'\n'+result.stderr
        (DATA/('guard-'+label+'.txt')).write_text(output.replace('/work/','<tmp>/').replace('/src/','<repo>/'))
    (DATA/'guard-proof.json').write_text(json.dumps({'removed_invalidations':2,'broken_exit':broken.returncode,
        'restored_exit':restored.returncode,'restored_source_sha256':checksum,'source_matches_measured_fork':True},indent=2)+'\n')
    publication=json.loads((DATA/'publication.json').read_text())
    for path in DATA.glob('guard-*'):
        publication['files'][path.name]=hashlib.sha256(path.read_bytes()).hexdigest()
    (DATA/'publication.json').write_text(json.dumps(publication,indent=2)+'\n')
    print('Schema-reprepare guard fails with both invalidations removed, passes after exact source restoration')

if __name__=='__main__':main()
