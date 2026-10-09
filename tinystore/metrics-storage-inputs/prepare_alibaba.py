#!/usr/bin/env python3
"""Reproduce the public Alibaba selection with retained Go corpus converters."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
WORK = Path(os.environ.get('METRICS_STORAGE_INPUT', '/work/metrics-storage-input'))
EXPECTED = '3e6ee87fd204bb85b9e234c5c75a5096580fdabc8f085b224033080090753a7a'
URL = 'https://clusterdata2018pubcn.oss-cn-beijing.aliyuncs.com/machine_usage.tar.gz'

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as source:
        while chunk := source.read(1 << 20): h.update(chunk)
    return h.hexdigest()

def main():
    corpus = WORK/'corpus'; corpus.mkdir(parents=True, exist_ok=True)
    archive = corpus/'machine_usage.tar.gz'
    if not archive.exists():
        partial = archive.with_suffix('.gz.part')
        subprocess.run(['curl','-fL','--retry','2','-C','-','-o',str(partial),URL],check=True)
        partial.replace(archive)
    assert sha(archive) == EXPECTED, 'public archive differs from historical corpus'
    binaries = WORK/'bin'; binaries.mkdir(exist_ok=True)
    for module, output in [('alibaba','alibaba-select'),('tsbs','normalize')]:
        subprocess.run(['go','build','-trimpath','-o',str(binaries/output),'.'],
                       cwd=ROOT/'tinystore/bench'/module,check=True)
    line_protocol = corpus/'alibaba.lp'
    normalized = corpus/'alibaba-series.jsonl'
    assert not normalized.exists(), 'refusing to overwrite a prepared corpus'
    # Keep the 9 GB CSV out of both the checkout and the persistent volume.
    unpack = subprocess.Popen(['tar','-xOzf',str(archive),'machine_usage.csv'],stdout=subprocess.PIPE)
    try:
        selected = subprocess.run([str(binaries/'alibaba-select'),'-in=/dev/stdin',
            '-out='+str(line_protocol),'-every=16','-until=172800','-base=1767225600000'],
            stdin=unpack.stdout,text=True,capture_output=True)
        unpack.stdout.close()
        assert unpack.wait() == 0, 'archive extraction failed'
        assert selected.returncode == 0, selected.stderr
    finally:
        if unpack.poll() is None: unpack.terminate(); unpack.wait()
    (corpus/'alibaba-selection.log').write_text(selected.stdout+selected.stderr)
    converted = subprocess.run([str(binaries/'normalize'),'-in='+str(line_protocol),
        '-corpus='+str(normalized)],text=True,capture_output=True)
    assert converted.returncode == 0, converted.stderr
    (corpus/'alibaba-normalization.log').write_text(converted.stdout+converted.stderr)
    manifest = {'source_url':URL,'archive_sha256':EXPECTED,'archive_bytes':archive.stat().st_size,
        'selection':{'machine_modulus':16,'trace_second_exclusive':172800,'base_milliseconds':1767225600000},
        'line_protocol_sha256':sha(line_protocol),'normalized_sha256':sha(normalized),
        'normalized_bytes':normalized.stat().st_size,
        'selector_sha256':sha(binaries/'alibaba-select'),'converter_sha256':sha(binaries/'normalize'),
        'source_files':{str(path.relative_to(ROOT)):sha(path) for path in [
            ROOT/'tinystore/bench/alibaba/main.go',ROOT/'tinystore/bench/alibaba/go.mod',
            ROOT/'tinystore/bench/tsbs/main.go',ROOT/'tinystore/bench/tsbs/go.mod']},
        'selection_output':selected.stdout+selected.stderr,
        'conversion_output':converted.stdout+converted.stderr,
        'note':'Input preparation only; elapsed time is not a storage-performance measurement. Corpus bytes are not committed.'}
    (corpus/'alibaba-provenance.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest,indent=2),flush=True)

if __name__ == '__main__': main()
