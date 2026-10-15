#!/usr/bin/env python3
"""Fetch a pinned public Russian treebank; never commit its corpus."""
import hashlib
import json
from pathlib import Path
import urllib.request

REV = 'c5266a6f6f4aaa94f4dfa9b6cc863b94a5358afa'
URL = f'https://raw.githubusercontent.com/UniversalDependencies/UD_Russian-SynTagRus/{REV}/ru_syntagrus-ud-train-a.conllu'
ROOT = Path(__file__).resolve().parent / 'corpus'
ROOT.mkdir(exist_ok=True)
raw = ROOT / 'syntagrus.conllu'
if not raw.exists():
    with urllib.request.urlopen(URL) as response:
        raw.write_bytes(response.read())
texts = [line[9:] for line in raw.read_text().splitlines() if line.startswith('# text = ')]
assert len(texts) > 10000
target = ROOT / 'russian.txt'
target.write_text('\n'.join(texts) + '\n')
manifest = {'source_url': URL, 'source_revision': REV, 'raw_sha256': hashlib.sha256(raw.read_bytes()).hexdigest(),
            'sentences': len(texts), 'text_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
            'license': 'CC BY-NC-SA 4.0 (upstream pinned README)',
            'purpose': 'public research-only source sentences; not committed; repeated/augmented explicitly by fixture'}
print(json.dumps(manifest, ensure_ascii=False, indent=2))
