"""How far TinyStore's ten agree with compact's on the sample: the same ten
in the same order, the same ten in any order, and the share of documents
the two tens hold in common, query by query.

    python -I agree.py <answers dir>   # reads tinystore-sample.jsonl.gz and compact-sample.jsonl.gz
"""
import gzip
import json
import sys


def read(path):
    with gzip.open(path, 'rt', encoding='utf-8') as lines:
        return {row['qid']: [pid for pid, _ in row['ten']] for row in map(json.loads, lines)}


folder = sys.argv[1]
tiny = read(f'{folder}/tinystore-sample.jsonl.gz')
compact = read(f'{folder}/compact-sample.jsonl.gz')
qids = sorted(set(tiny) & set(compact))
same_order = sum(tiny[q] == compact[q] for q in qids)
same_set = sum(set(tiny[q]) == set(compact[q]) for q in qids)
shared = sum(len(set(tiny[q]) & set(compact[q])) for q in qids)
held = sum(len(compact[q]) for q in qids)
print(f'| queries | same ten, same order | same ten | documents in common |')
print(f'|---|---|---|---|')
print(f'| {len(qids)} | {same_order} | {same_set} | {shared} of {held}, {100 * shared / max(held, 1):.1f} % |')
