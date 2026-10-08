#!/usr/bin/env python3
"""SQLite adapter candidates against the retained optimized native Rust source."""
import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tempfile
import run as workloads

ROOT = Path(__file__).resolve().parent
WORK = Path(os.environ.get('ADAPTER_WORK', '/work'))
DATA = ROOT.parent / 'reports/data/sqlite-adapter-2026-10-08'
BASE = ['--fast-exact', '1', '--fast-codec', '1', '--tuning', '15', '--threads', '0']
VARIANTS = {'baseline': ('rust-baseline', None), 'control': ('rust-adapter', 0),
            'metadata': ('rust-adapter', 1), 'arena': ('rust-adapter', 2),
            'both': ('rust-adapter', 3), 'lookaside_off': ('rust-adapter', 4),
            'lookaside_512': ('rust-adapter', 8), 'both_lookaside_512': ('rust-adapter', 11)}
WIDE = ['read_wide64', 'stream_wide64'] + ['aggregate_wide_' + s for s in
        ['sum', 'cut_sum', 'cut_avg', 'cut_count', 'grouped_cut_sum']]
CASES = ['read_head_point', 'read_head_full8', 'read_sealed_full8', 'read_sealed_full16',
         'read_wide64', 'stream_wide64', 'aggregate_sum', 'aggregate_count',
         'aggregate_cut_sum', 'aggregate_wide_cut_sum', 'aggregate_wide_grouped_cut_sum',
         'ingest_scrape100', 'maintain_ready']

def call(args, **kw):
    result = subprocess.run(args, capture_output=True, text=True, **kw)
    if result.returncode:
        raise RuntimeError(f'{args}: {result.returncode}\n{result.stdout}\n{result.stderr}')
    return result.stdout.strip()

def sources():
    paths = [ROOT/'adapter_run.py', ROOT/'adapter_build.sh'] + sorted((ROOT/'rust/src').rglob('*.rs'))
    paths += [ROOT/'rust/Cargo.toml', ROOT/'rust/Cargo.lock']
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}

def fixture_for(case):
    return 'wide' if case in WIDE else workloads.fixture_for(case)

def run_one(variant, case, mode, iterations=1, warm=0, threads=0):
    binary, mask = VARIANTS[variant]
    if mode == 'profile': binary = 'rust-adapter-telemetry'
    with tempfile.TemporaryDirectory(prefix='adapter-', dir=WORK/'stores') as d:
        database = Path(d)/'metrics.db'
        shutil.copyfile(WORK/'stores'/fixture_for(case)/'metrics.db', database)
        flags = list(BASE)
        if mask is not None: flags += ['--adapter', str(mask)]
        if threads: flags += ['--threads', str(threads), '--force-parallel', '1']
        args = [str(WORK/'bin'/binary), '--db', str(database), '--case', case,
                '--mode', mode, '--iterations', str(iterations), '--warm', str(warm), *flags]
        if case == 'expire_all': args += ['--now', str(workloads.NORMAL_NOW + 30*24*60*60*1000+5000)]
        if mode == 'memory': args = ['/usr/bin/time', '-f', '%M', '-o', str(Path(d)/'rss'), *args]
        allowed = sorted(os.sched_getaffinity(0))
        affinity = ','.join(map(str, allowed[:2] if threads else allowed[:1]))
        row = json.loads(call(['taskset', '-c', affinity, *args]))
        if mode == 'memory': row['max_rss_kib'] = int((Path(d)/'rss').read_text())
        if case in workloads.WRITE_CASES and mode in ['bench', 'verify']:
            row['go_crossread'] = json.loads(call([str(WORK/'bin/go-reference'), '--db', str(database), '--mode', 'digest']))['final_digest']
        return row

def verify():
    verification = []
    cases = list(dict.fromkeys(workloads.CASES + WIDE + workloads.EDGE_CASES))
    for case in cases:
        n = 1 if case in ['maintain_ready', 'expire_all'] or case in workloads.EDGE_CASES else 4
        results = {v: run_one(v, case, 'verify', n) for v in VARIANTS}
        assert all(x == results['baseline'] for x in results.values()), (case, results)
        for variant in ['control', 'both']:
            assert run_one(variant, case, 'verify', n, threads=2) == results['baseline'], ('parallel', case, variant)
        verification.append({'case': case, 'result': results['baseline'], 'variants': list(VARIANTS), 'parallel': ['control', 'both']})
    for mode, case in [('guards', 'read_sealed_full8'), ('edge-guards', 'read_edge')]:
        results = {v: run_one(v, case, mode) for v in VARIANTS}
        assert all(x == results['baseline'] for x in results.values()), (mode, results)
        assert all(v for k, v in results['baseline'].items() if k != 'final_digest')
        verification.append({'mode': mode, 'result': results['baseline']})
    for case in workloads.READ_CASES + workloads.AGGREGATE_CASES + WIDE:
        results = {v: run_one(v, case, 'plan') for v in VARIANTS}
        assert all(x == results['baseline'] for x in results.values()), ('plan', case, results)
        verification.append({'case': case, 'plan': results['baseline']})
    DATA.mkdir(parents=True, exist_ok=True)
    (DATA/'verification.json').write_text(json.dumps(verification, indent=2)+'\n')
    (DATA/'verified-sources.json').write_text(json.dumps(sources(), indent=2)+'\n')
    print(f'Verified {len(cases)} traces, errors, plans, Go cross-reading and two-thread processing', flush=True)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--verify-only', action='store_true')
    parser.add_argument('--passes', type=int, default=6)
    args = parser.parse_args()
    if args.verify_only:
        verify()
        return
    # The user explicitly requested measurements without a harness commit.
    # Record exact source/binary hashes and do not present this as a formal
    # committed-harness round.
    revision = os.environ.get('RESEARCH_HARNESS_COMMIT', 'uncommitted, by explicit user request')
    assert json.loads((DATA/'verified-sources.json').read_text()) == sources(), 'rerun verification after any source change'
    started = dt.datetime.now(dt.timezone.utc).isoformat()
    counts = {}
    for case in CASES:
        if case == 'maintain_ready': counts[case] = 1; continue
        pilot = run_one('baseline', case, 'bench', 32, 32)
        cap = 128 if case in workloads.WRITE_CASES else 16384
        counts[case] = max(8, min(cap, math.ceil(0.20e9/pilot['ns_per_op'])))
    environment = {'harness_commit': revision, 'baseline_commit': '8955ce2', 'started_utc': started,
        'platform': platform.platform(), 'cpu': call(['lscpu']),
        'cpu_quota': Path('/sys/fs/cgroup/cpu.max').read_text().strip(),
        'memory_limit': Path('/sys/fs/cgroup/memory.max').read_text().strip(),
        'rustc': call(['/work/cargo/bin/rustc', '--version', '--verbose']),
        'go': call(['go', 'version']), 'source_sha256': sources(),
        'binary_sha256': {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (WORK/'bin').iterdir()},
        'iterations': counts, 'warm':32, 'passes':args.passes, 'variants':VARIANTS,
        'native_build':json.loads((ROOT.parent/'sqlite-bench/generated/native-build.json').read_text()),
        'note':'Local Docker Desktop/WSL2 experiment; compare ratios within this session, not bare-Linux latency or server throughput'}
    (DATA/'environment.json').write_text(json.dumps(environment,indent=2)+'\n')
    rows=[]
    with (DATA/'timings.jsonl').open('w') as output:
        for pass_id in range(args.passes):
            print(f'Quiet timing pass {pass_id+1}/{args.passes}', flush=True)
            for index,case in enumerate(CASES):
                order=list(VARIANTS); shift=(pass_id+index)%len(order); order=order[shift:]+order[:shift]
                if pass_id%2: order.reverse()
                digests=[]
                for variant in order:
                    row=run_one(variant,case,'bench',counts[case],0 if case=='maintain_ready' else 32)
                    row.update(variant=variant,pass_id=pass_id+1,utc=dt.datetime.now(dt.timezone.utc).isoformat())
                    rows.append(row);output.write(json.dumps(row)+'\n');output.flush()
                    if 'go_crossread' in row:digests.append(row['go_crossread'])
                assert len(set(digests))<=1,(case,digests)
    summary=[]
    for case in CASES:
        passes={v:[r['ns_per_op'] for r in rows if r['case']==case and r['variant']==v] for v in VARIANTS}
        medians={v:statistics.median(values) for v,values in passes.items()}
        summary.append({'case':case,'medians_ns':medians,'passes_ns':passes,
            'baseline_over_variant':{v:medians['baseline']/ns for v,ns in medians.items()},
            'control_over_variant':{v:medians['control']/ns for v,ns in medians.items()}})
    (DATA/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    profiles=[]
    for case in ['read_sealed_full16','read_wide64','aggregate_wide_cut_sum','aggregate_sum','ingest_scrape100']:
        for variant in list(VARIANTS)[1:]:
            row=run_one(variant,case,'profile',32,32);row.update(variant=variant);profiles.append(row)
    (DATA/'profiles.json').write_text(json.dumps(profiles,indent=2)+'\n')
    memory=[]
    for case in ['read_sealed_full16','read_wide64','stream_wide64']:
        for pass_id in range(3):
            for variant in ['baseline','control','both','lookaside_off','lookaside_512']:
                row=run_one(variant,case,'memory',16);row.update(variant=variant,pass_id=pass_id+1);memory.append(row)
    (DATA/'memory.json').write_text(json.dumps(memory,indent=2)+'\n')
    wrappers=[]
    for limit in [1,64,240]:
        for pass_id in range(args.passes):
            for variant in ['safe','raw'] if pass_id%2==0 else ['raw','safe']:
                row=run_one('control',f'{variant}_{limit}','wrapper',16384 if limit==1 else 2048,32)
                row.update(pass_id=pass_id+1);wrappers.append(row)
    (DATA/'wrappers.json').write_text(json.dumps(wrappers,indent=2)+'\n')
    environment['finished_utc']=dt.datetime.now(dt.timezone.utc).isoformat()
    (DATA/'environment.json').write_text(json.dumps(environment,indent=2)+'\n')
    print('Timing, allocation/counter profiles, RSS and wrapper probes complete',flush=True)

if __name__=='__main__':main()
