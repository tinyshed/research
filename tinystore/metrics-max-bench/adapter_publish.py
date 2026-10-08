#!/usr/bin/env python3
"""Check retained trials and sanitize container paths in upstream test logs."""
import hashlib
import json
from pathlib import Path
import re
import adapter_run as engine
import adapter_tables

ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data'

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    adapter_tables.main()
    first=DATA/'sqlite-adapter-2026-10-08';second=DATA/'rusqlite-fork-2026-10-08'
    environment=json.loads((first/'environment.json').read_text())
    followup=json.loads((second/'environment.json').read_text())
    assert environment['source_sha256']==engine.sources(),'measured adapter source changed'
    assert followup['engine_source_sha256']==engine.sources(),'measured fork engine source changed'
    assert followup['runner_sha256']==sha(ROOT/'rusqlite_run.py')
    assert followup['build']['builder_sha256']==sha(ROOT/'rusqlite_fork.py')
    checks={}
    for directory,expected_count in [(first,624),(second,402)]:
        timings=[json.loads(line) for line in (directory/'timings.jsonl').read_text().splitlines()]
        assert len(timings)==expected_count,(directory,len(timings))
        for row in timings:assert row['iterations']>0 and row['ns_per_op']>0
        for case in engine.CASES:
            selected=[row for row in timings if row['case']==case]
            assert len({row['iterations'] for row in selected})==1
            assert len({row['warm'] for row in selected})==1
            for variant in {row['variant'] for row in selected}:
                assert len([row for row in selected if row['variant']==variant])==6
        checks[directory.name]={'complete_operation_samples':len(timings),'equal_counts_and_warm':True,
            'same_measured_engine_sources':True}
    sanitized={}
    for path in second.glob('*-tests.txt'):
        original=path.read_text();original_hash=sha(path)
        text=original.replace('/src/','<repo>/').replace('/work/','<tmp>/')
        path.write_text(text)
        sanitized[path.name]={'original_sha256':original_hash,'published_sha256':sha(path),
            'replacement':'container /src/ -> <repo>/; container /work/ -> <tmp>/'}
    stock=(second/'upstream-stock-tests.txt').read_text()
    failed=lambda text:sorted(re.findall(r'^---- (.*?) stdout ----$',text,re.M))
    assert len(failed(stock))==11 and '166 passed; 11 failed' in stock
    for label in ['inline','count','combined']:
        log=(second/(label+'-tests.txt')).read_text()
        assert failed(log)==failed(stock) and '168 passed; 11 failed' in log
    for directory in [first,second]:
        manifest={'checks':checks[directory.name],
            'files':{str(path.relative_to(directory)):sha(path) for path in sorted(directory.iterdir()) if path.is_file() and path.name!='publication.json'}}
        if directory==second:manifest['sanitized_logs']=sanitized
        (directory/'publication.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print('Verified 624 + 402 complete-operation samples, equal traces, source hashes and identical upstream failure sets')

if __name__=='__main__':main()
