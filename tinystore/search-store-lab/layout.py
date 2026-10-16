#!/usr/bin/env python3
"""Untimed post-churn file attribution; run after all timing, against frozen CLI."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess as sp
import tempfile

ROOT = Path(__file__).resolve().parent
DATA = ROOT.parent / 'reports/data/search-2026-10-16'
BINARY = Path('/target/release/search-store-lab')
expected = json.loads((DATA / 'owned-store.jsonl').read_text().splitlines()[0])
sha = hashlib.sha256(BINARY.read_bytes()).hexdigest()
assert sha == expected['binary_sha256']
results = []
with tempfile.TemporaryDirectory(prefix='search-layout-', dir='/data') as scratch:
    for engine in ('fts', 'tant'):
        path = Path(scratch) / engine
        command = [str(BINARY), engine, str(ROOT.parent / 'search-lab/corpus/mixed.json'), '100000']
        p = sp.run(command, env=dict(os.environ, BENCH_DB=str(path)),
                   capture_output=True, text=True, timeout=600)
        assert p.returncode == 0, p.stderr
        a = json.loads(p.stdout.strip().splitlines()[-1])
        item = {'engine': engine, 'rows': a['rows'], 'exit': p.returncode,
                'command': f'BENCH_DB=<scratch>/{engine} <bin> {engine} <corpus>/mixed.json 100000',
                'stage': 'after the same 100-document SET/DELETE and restore phases',
                'backend_bytes_at_output': a['final_backend_bytes']}
        if engine == 'fts':
            with sqlite3.connect(f'file:{path}?mode=ro', uri=True) as c:
                item['objects'] = [{'name': name, 'pages': pages, 'bytes': size,
                                    'payload_bytes': payload, 'unused_bytes': unused}
                                   for name, pages, size, payload, unused in c.execute(
                    'select name,count(*),sum(pgsize),sum(payload),sum(unused) from dbstat group by name order by name')]
                item['freelist_pages'] = c.execute('pragma freelist_count').fetchone()[0]
                item['page_size'] = c.execute('pragma page_size').fetchone()[0]
                item['backend_bytes_after_close'] = path.stat().st_size
                assert sum(o['bytes'] for o in item['objects']) + item['freelist_pages'] * item['page_size'] == item['backend_bytes_after_close']
        else:
            item['files'] = [{'name': f.name, 'bytes': f.stat().st_size}
                             for f in sorted(path.iterdir()) if f.is_file()]
            item['backend_bytes_after_close'] = sum(f['bytes'] for f in item['files'])
        item['close_delta_bytes'] = item['backend_bytes_after_close'] - item['backend_bytes_at_output']
        results.append(item)
out = {'utc': dt.datetime.now(dt.timezone.utc).isoformat(),
       'audit_revision': sp.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
       'binary_source_revision': expected['revision'], 'binary_sha256': sha,
       'scope': 'One structural attribution at 100k after churn; timings deliberately excluded. Not a sustained write-amplification measurement or a million-row per-object extrapolation.',
       'sqlite_inspector_version': sqlite3.sqlite_version, 'results': results}
(DATA / 'storage-layout.json').write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps(out))
