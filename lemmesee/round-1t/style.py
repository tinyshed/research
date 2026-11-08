"""Round 1t: the graph stage read against the other columns of the schema
named alike, the schema's own house style read from the schema itself.

Looked into on LiveSQLBench on 8 November 2026 (`errors.py`): the model
misses 161 of its 299 keys and draws 33 false ones. A schema there names its
keys in a style of its own, `acct_mon`, `acct_node`, `acct_gate` and
`acct_anchor` all into `accounts`, `*_registry` into the table it registers;
each pair alone scores 0.46 to 0.72, under the threshold, and nothing reads
them together. These features do, from the first stage's scores and the
names alone, for a pair from column c to target t:

- c's *lead word*, its first word that is not a key word (`id`, `ref`, ...),
  as `words7` splits it: of the schema's other columns with the same lead
  word, the mean of their best score into t, and the share of them whose
  best target is t; and how many they are, over the schema's tables;
- c's *last word*, as it is (`ref`, `registry`, `anchor` ...): how many
  columns of the schema end in it, over its tables, and their mean best
  score over the schema's mean best (the graph stage's own mean is absolute);
- t: how many of the schema's columns have it as their best target, c
  aside, over the schema's tables.

On top of stack.py's relative stage, whose features come first.
"""

from collections import defaultdict
from functools import lru_cache

import numpy as np

import graph
import stack
import words7

STYLE = ["lead word: others into the target, mean best", "lead word: share whose best is the target",
         "lead word: columns, over tables", "last word: columns, over tables",
         "last word: mean best over the schema's mean best", "target: columns whose best it is, over tables"]


@lru_cache(maxsize=400_000)
def pieces(name: str) -> tuple:
    return tuple(w for tok in words7.tokens(name) for w in words7.split(tok))


# `style2`: a name of one word has no lead word. Looked into on CTU: in a
# logic program's tables (Mooney_Family, Dunur, SAT) every column is `name1`,
# `name2` or `tm`, the lead word the whole name, and the lead features read
# every column of the schema as alike; `seven-style` lost 117 of their keys
ONE_WORD_LEAD = True


@lru_cache(maxsize=400_000)
def lead_and_last(name: str, one_word_lead: bool = True) -> tuple:
    p = pieces(name)
    words = [w for w in p if w not in words7.KEY_WORDS and len(w) >= 2]
    lead = words[0] if words and (one_word_lead or len(words) > 1) else ""
    return lead, (p[-1] if p else "")


def style_features(meta, scores):
    out = np.zeros((len(meta), len(STYLE)), dtype=np.float32)
    start = 0
    while start < len(meta):
        end = start
        while end < len(meta) and meta[end][graph.SID] == meta[start][graph.SID]:
            end += 1
        into = {}  # (column, target): the column's best score into the target
        best, best_target = {}, {}
        for i in range(start, end):
            m = meta[i]
            k, t, sc = (m[graph.TABLE], m[graph.COLUMN]), m[graph.TARGET], float(scores[i])
            if into.get((k, t), -1.0) < sc:
                into[(k, t)] = sc
            if best.get(k, -1.0) < sc:
                best[k], best_target[k] = sc, t
        tables = max(len({k[0] for k in best}), 1)
        names = {k: lead_and_last(k[1], ONE_WORD_LEAD) for k in best}
        lead_n, last_n, last_sum = defaultdict(int), defaultdict(int), defaultdict(float)
        for k, (lead, last) in names.items():
            if lead:
                lead_n[lead] += 1
            last_n[last] += 1
            last_sum[last] += best[k]
        lead_into, lead_best = defaultdict(float), defaultdict(int)
        for (k, t), sc in into.items():
            lead = names[k][0]
            if lead:
                lead_into[(lead, t)] += sc
        for k, t in best_target.items():
            if names[k][0]:
                lead_best[(names[k][0], t)] += 1
        target_best = defaultdict(int)
        for t in best_target.values():
            target_best[t] += 1
        mean_best = float(np.mean(list(best.values()))) if best else 0.0
        for i in range(start, end):
            m = meta[i]
            k, t = (m[graph.TABLE], m[graph.COLUMN]), m[graph.TARGET]
            lead, last = names[k]
            mine = into[(k, t)]
            is_best = best_target[k] == t
            n = lead_n[lead] - 1 if lead else 0
            if n > 0:
                f0 = (lead_into[(lead, t)] - mine) / n
                f1 = (lead_best[(lead, t)] - is_best) / n
            else:
                f0 = f1 = -1.0
            ln = last_n[last] - 1
            f4 = ((last_sum[last] - best[k]) / ln / mean_best) if ln > 0 and mean_best > 0 else -1.0
            out[i] = [f0, f1, n / tables, ln / tables, f4, (target_best[t] - is_best) / tables]
        start = end
    return out


def graph_features(meta, scores):
    return np.hstack([stack.graph_features(meta, scores), style_features(meta, scores)])


def graph_features2(meta, scores):
    global ONE_WORD_LEAD
    ONE_WORD_LEAD = False
    try:
        return graph_features(meta, scores)
    finally:
        ONE_WORD_LEAD = True
