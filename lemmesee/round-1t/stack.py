"""Round 1t's graph stage read relative to its schema, and fitted on scores
that have seen a house style shift.

Looked into on CTU on 8 November 2026: in a schema named in a style the
first stage has not seen, its scores fall together (Mooney_Family's keys
score 0.11 to 0.14, every false pair under 0.01): each column's best target
is right, every score is below any threshold set on SchemaPile. The graph
stage reads absolute scores; these features read each pair against its own
schema:

- the pair's score over the schema's best;
- the column's best over the mean of the schema's columns' bests;
- where the pair's score falls among the schema's columns' bests (0 lowest,
  1 highest);
- the pair's score over the column's next, as a log ratio;
- the schema's best and the mean of its columns' bests, as they are.

**Shift-aware stacking**: the second stage learns from first-stage scores
out of fold, as before; for the renamed training schemas, from a first stage
fitted on the same folds' original schemas only, which has not seen the
renaming. The second stage then meets, in training, schemas whose scores
fell together under a style it was not trained on.
"""

import math
from collections import defaultdict

import numpy as np

import graph

EXTRA = ["score over the schema's best", "column's best over the schema's mean best",
         "place among the schema's column bests", "log lead over the column's next",
         "the schema's best", "the schema's mean column best"]


def graph_features(meta, scores):
    base = graph.graph_features(meta, scores)
    extra = np.zeros((len(meta), len(EXTRA)), dtype=np.float32)
    start = 0
    while start < len(meta):
        end = start
        while end < len(meta) and meta[end][graph.SID] == meta[start][graph.SID]:
            end += 1
        by_column = defaultdict(list)
        for i in range(start, end):
            m = meta[i]
            by_column[(m[graph.TABLE], m[graph.COLUMN])].append(float(scores[i]))
        bests = {k: max(v) for k, v in by_column.items()}
        nexts = {k: (sorted(v, reverse=True)[1] if len(v) > 1 else 0.0) for k, v in by_column.items()}
        ordered = np.sort(np.fromiter(bests.values(), dtype=np.float64))
        top = float(ordered[-1]) if len(ordered) else 0.0
        mean = float(ordered.mean()) if len(ordered) else 0.0
        n = len(ordered)
        for i in range(start, end):
            m = meta[i]
            k = (m[graph.TABLE], m[graph.COLUMN])
            sc = float(scores[i])
            mine = bests[k]
            other = nexts[k] if sc >= mine else mine
            place = (np.searchsorted(ordered, sc, side="left") / (n - 1)) if n > 1 else 1.0
            extra[i] = [sc / top if top > 0 else 0.0, mine / mean if mean > 0 else 0.0, min(place, 1.0),
                        math.log((sc + 1e-3) / (other + 1e-3)), top, mean]
        start = end
    return np.hstack([base, extra])


def of(result: dict):
    """The graph stage a fit's graph-final.json names."""
    return graph_features if result.get("graph") == "relative" else graph.graph_features
