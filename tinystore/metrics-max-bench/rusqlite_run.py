#!/usr/bin/env python3
"""Remeasure stock rusqlite and three small dependency patches together."""
import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import adapter_run as engine

ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/rusqlite-fork-2026-10-08'
VARIANTS={'baseline':('rust-baseline',None),'control':('rust-adapter',0),
          'fork_inline':('rust-fork-inline',0),'fork_count':('rust-fork-count',0),
          'both':('rust-fork-combined',0),
          'lookaside_512':('rust-adapter',8),'both_lookaside_512':('rust-adapter',11)}
MAIN=list(VARIANTS)[:5]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--verify-only',action='store_true');args=parser.parse_args()
    engine.VARIANTS=VARIANTS;engine.DATA=DATA
    engine.verify()
    if args.verify_only:return
    counts={}
    for case in engine.CASES:
        if case=='maintain_ready':counts[case]=1;continue
        pilot=engine.run_one('control',case,'bench',32,32)
        counts[case]=max(16,min(128 if case in engine.workloads.WRITE_CASES else 32768,math.ceil(0.4e9/pilot['ns_per_op'])))
    meta={'started_utc':dt.datetime.now(dt.timezone.utc).isoformat(),
        'harness_status':'uncommitted during collection, by user request',
        'baseline_revision':'8955ce2','variants':VARIANTS,'passes':6,'iterations':counts,'warm':32,
        'build':json.loads((DATA/'build.json').read_text()),
        'engine_source_sha256':engine.sources(),
        'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'parent_environment':'../sqlite-adapter-2026-10-08/environment.json',
        'affinity':sorted(os.sched_getaffinity(0))[:1]}
    (DATA/'environment.json').write_text(json.dumps(meta,indent=2)+'\n')
    rows=[]
    with (DATA/'timings.jsonl').open('w') as out:
        for pass_id in range(6):
            print(f'Rusqlite patch timing pass {pass_id+1}/6',flush=True)
            for index,case in enumerate(engine.CASES):
                order=list(VARIANTS) if case=='read_head_full8' else list(MAIN)
                shift=(pass_id+index)%len(order);order=order[shift:]+order[:shift]
                if pass_id%2:order.reverse()
                digests=[]
                for variant in order:
                    row=engine.run_one(variant,case,'bench',counts[case],0 if case=='maintain_ready' else 32)
                    row.update(variant=variant,pass_id=pass_id+1,utc=dt.datetime.now(dt.timezone.utc).isoformat())
                    rows.append(row);out.write(json.dumps(row)+'\n');out.flush()
                    if 'go_crossread' in row:digests.append(row['go_crossread'])
                assert len(set(digests))<=1,(case,digests)
    summary=[]
    for case in engine.CASES:
        passes={v:[r['ns_per_op'] for r in rows if r['variant']==v and r['case']==case] for v in VARIANTS}
        passes={v:xs for v,xs in passes.items() if xs};medians={v:statistics.median(xs) for v,xs in passes.items()}
        summary.append({'case':case,'medians_ns':medians,'passes_ns':passes,
            'control_over_variant':{v:medians['control']/n for v,n in medians.items()},
            'baseline_over_variant':{v:medians['baseline']/n for v,n in medians.items()}})
    (DATA/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    wrappers=[]
    for limit in [1,64,240]:
        for pass_id in range(6):
            variants=MAIN[1:] if pass_id%2==0 else MAIN[1:][::-1]
            for variant in variants:
                for method in ['safe','raw'] if pass_id%2==0 else ['raw','safe']:
                    row=engine.run_one(variant,f'{method}_{limit}','wrapper',65536 if limit==1 else 8192,64)
                    row.update(variant=variant,pass_id=pass_id+1);wrappers.append(row)
    (DATA/'wrappers.json').write_text(json.dumps(wrappers,indent=2)+'\n')
    meta['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat()
    (DATA/'environment.json').write_text(json.dumps(meta,indent=2)+'\n')
    print('Rusqlite engine and wrapper comparisons complete',flush=True)

if __name__=='__main__':main()
