#!/usr/bin/env python3
"""Check retained source hashes, paired traces and report links, without timing."""
import hashlib
import json
from pathlib import Path
import re

ROOT=Path(__file__).resolve().parents[2]
BASE=ROOT/'tinystore/reports/data'
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path): return json.loads(path.read_text())
def jsonl(path): return [json.loads(x) for x in path.read_text().splitlines() if x]

def main():
    checks={}
    layout=BASE/'metrics-layout-2026-10-09'
    metadata=read(layout/'provenance.json')
    for name,digest in metadata['harness_source_hashes'].items():
        assert sha(ROOT/'tinystore/metrics-layout-bench'/name)==digest,name
    timing=read(layout/'timings.json')
    groups={}
    for row in timing:
        key=(row['dataset'],row['result']['case'],row['variant'])
        groups.setdefault(key,[]).append(row)
    assert len(timing)==504 and all(len(rows)==6 for rows in groups.values())
    for dataset,case,_ in groups:
        rows=[r for r in timing if r['dataset']==dataset and r['result']['case']==case]
        assert len({r['result']['iterations'] for r in rows})==1
        assert len({r['result']['result_sha256'] for r in rows})==1
    for name in ['publication.json','retention.json']:
        rows=read(layout/name);assert len(rows)==126
        for dataset in ['tsbs','alibaba','irregular']:
            for variant in {r['variant'] for r in rows}:
                assert len([r for r in rows if r['dataset']==dataset and r['variant']==variant])==6
    assert len(read(layout/'go-crossread.json'))==15
    checks['layout']={'timing_rows':len(timing),'six_pass_groups':len(groups),
        'publication_rows':126,'retention_rows':126,'public_go_crossreads':15,
        'measured_source_hashes_checked':len(metadata['harness_source_hashes'])}
    payload=BASE/'metrics-payload-2026-10-09'
    for name in ['environment-density-format51.json','environment-performance.json']:
        env=read(payload/name)
        for file,digest in env['source_sha256'].items():
            assert sha(ROOT/'tinystore'/file)==digest,(name,file)
    previous=read(payload/'environment-density.json')
    snapshot=payload/'format50-source'
    preserved=0
    for path in snapshot.rglob('*'):
        if path.is_file():
            key='metrics-payload-bench/'+str(path.relative_to(snapshot)).replace('\\','/')
            assert previous['source_sha256'][key]==sha(path),key
            preserved+=1
    summaries=read(payload/'summary.json');rows=jsonl(payload/'timings.jsonl')
    assert len(rows)==720 and len(summaries)==120
    assert all(len(r['passes_ns'])==6 for r in summaries)
    assert len(jsonl(payload/'verification-bytes.jsonl'))==120
    assert len(jsonl(payload/'verification.jsonl'))==8
    for dataset in ['tsbs','alibaba']:
        for case in ['point','sparse','range','full','summary']:
            for cache in ['cold','warm']:
                pair=[r for r in rows if (r['dataset'],r['case'],r['cache'])==(dataset,case,cache)]
                assert len({r['iterations'] for r in pair})==1
                assert len({r['digest'] for r in pair})==1
    checks['payload']={'timing_rows':len(rows),'six_pass_groups':len(summaries),
        'encoded_byte_replays':120,'full_public_manifest_replays':8,
        'format50_runtime_snapshot_files':preserved,
        'format50_runner_note':'Original runner hash is retained; its later orchestration revisions are not asserted byte-identical.'}
    links=[]
    for name in ['metrics-layout-2026-10-09.md','metrics-payload-2026-10-09.md']:
        report=ROOT/'tinystore/reports'/name
        for target in re.findall(r'\]\(([^)]+)\)',report.read_text()):
            if '://' in target:continue
            assert (report.parent/target.split('#')[0]).exists(),(name,target)
            links.append((name,target))
    checks['report_links_checked']=len(links)
    for directory,key in [(layout,'layout'),(payload,'payload')]:
        out={'checks':checks[key],'report_links_checked':checks['report_links_checked'],
            'files':{str(p.relative_to(directory)).replace('\\','/'):sha(p) for p in sorted(directory.rglob('*'))
                if p.is_file() and p.name!='completion.json' and '__pycache__' not in p.parts}}
        (directory/'completion.json').write_text(json.dumps(out,indent=2)+'\n')
    print(json.dumps(checks,indent=2))

if __name__=='__main__':main()
