#!/usr/bin/env python3
"""Post-timing diagnostics against the frozen binary; never a new benchmark."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess as sp
import tempfile

ROOT = Path(__file__).resolve().parent
DATA = ROOT.parent / 'reports/data/search-2026-10-16'
BINARY = Path('/target/release/search-lab')
records = [json.loads(s) for s in (DATA / 'retrieval.jsonl').read_text().splitlines()]
samples = {}
for engine in ('fts', 'tant'):
    rows = [r for r in records if r['label'] == f'mixed-{engine}-auto']
    assert len(rows) == 2
    assert rows[0]['result']['query_samples'] == rows[1]['result']['query_samples']
    samples[engine] = {s['key']: s['ndcg10'] for s in rows[0]['result']['query_samples']}
assert samples['fts'].keys() == samples['tant'].keys()
differences = [samples['fts'][k] - samples['tant'][k] for k in sorted(samples['fts'])]
rng = random.Random(20261016)
means = sorted(sum(rng.choice(differences) for _ in differences) / len(differences)
               for _ in range(2000))
bootstrap = {
    'metric': 'paired mixed auto collection nDCG@10: FTS5 minus Tantivy',
    'queries': len(differences), 'resamples': 2000, 'seed': 20261016,
    'observed_difference': sum(differences) / len(differences),
    'percentile_95_interval': [means[49], means[1949]],
    'scope': 'Query resampling on the fixed judged-passage pool, not all-language population inference',
}

# All raw terms must have a namespace disjoint from internal stem aliases.
docs = [
    {'id': 1, 'key': 'literal-ru', 'title': 's_ru_кошк', 'body': ''},
    {'id': 2, 'key': 'real-ru', 'title': 'Кошки', 'body': ''},
    {'id': 3, 'key': 'literal-en', 'title': 's_en_run', 'body': ''},
    {'id': 4, 'key': 'real-en', 'title': 'running', 'body': ''},
]
queries = [
    {'key': 'ru', 'text': 'кошка', 'relevant': [2],
     'judged': {str(i): int(i == 2) for i in range(1, 5)}},
    {'key': 'en', 'text': 'run', 'relevant': [4],
     'judged': {str(i): int(i == 4) for i in range(1, 5)}},
]
fixture = {'docs': docs, 'queries': queries}
sha = hashlib.sha256(BINARY.read_bytes()).hexdigest()
assert sha == records[0]['binary_sha256']
probes = []
with tempfile.TemporaryDirectory(prefix='search-audit-', dir='/data') as scratch:
    fixture_path = Path(scratch) / 'namespace.json'
    fixture_path.write_text(json.dumps(fixture, ensure_ascii=False))
    for engine in ('fts', 'tant'):
        command = [str(BINARY), 'run', engine, 'auto', str(fixture_path)]
        p = sp.run(command, env=dict(os.environ, BENCH_DB=str(Path(scratch) / engine)),
                   capture_output=True, text=True, timeout=120)
        assert p.returncode == 0, p.stderr
        result = json.loads(p.stdout.strip().splitlines()[-1])
        probes.append({'engine': engine, 'exit': p.returncode,
                       'command': f'BENCH_DB=<scratch>/{engine} <bin> run {engine} auto <fixture>/namespace.json',
                       'result': result})
out = {
    'utc': dt.datetime.now(dt.timezone.utc).isoformat(),
    'audit_revision': sp.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'measured_binary_source_revision': records[0]['revision'], 'binary_sha256': sha,
    'timings': 'Diagnostic only: excludes these tiny-fixture timings from performance claims',
    'quality_bootstrap': bootstrap, 'namespace_fixture': fixture, 'namespace_probes': probes,
}
(DATA / 'postflight-audit.json').write_text(json.dumps(out, ensure_ascii=False, indent=2) + '\n')
print(json.dumps({'quality_bootstrap': bootstrap,
                  'namespace_results': {p['engine']: p['result']['query_samples'] for p in probes}},
                 ensure_ascii=False))
