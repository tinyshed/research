"""What the Rust core must say, said by lemmesee's Python: for each snapshot,
every question round 1c's generator makes — its title, identity, shape and
statement for the snapshot's engine — the vetoes, tier N's points, the
features of the first few hundred, the score of round 1l's ranker of Luna
and DeepSeek, round 1c's five and round 1m's screen.

    python -I golden.py <out dir> <snapshot.json>...

writes <out dir>/<database>.json, one a snapshot.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
LEMMESEE = HERE.parent.parent / "lemmesee"
for d in ("round-1c", "round-1m"):
    sys.path.insert(0, str(LEMMESEE / d))

import numpy as np  # noqa: E402

import learn  # noqa: E402
import rank  # noqa: E402
import roles  # noqa: E402
from aq import generate, identity, load  # noqa: E402

FEATURES = 300


def ident(i) -> list:
    base, (agg, cols), time, by = i
    return [base, agg, sorted([list(c) for c in cols]), list(time) if time else None, list(by) if by else None]


def num(x: float):
    return None if isinstance(x, float) and math.isnan(x) else x


def main(out: Path, snaps: list[Path]) -> None:
    import lightgbm as lgb
    model = lgb.Booster(model_file=str(LEMMESEE / "corpus" / "round-1l" / "learn" / "luna-and-deepseek" / "ranker.txt"))
    names = model.feature_name()
    mw = roles.money_words()
    out.mkdir(parents=True, exist_ok=True)
    for path in snaps:
        snap = json.loads(path.read_text(encoding="utf-8"))
        schema = load(snap, True)
        all_qs = generate(schema)
        vetoes = [rank.veto(schema, q) for q in all_qs]
        qs = [q for q, v in zip(all_qs, vetoes) if v is None]
        feats = [rank.features(schema, q) for q in qs]
        row = {"features": feats, "slots": [rank.slots(schema, q) for q in qs]}
        X, _, _ = learn.matrix_free(row, names, None)
        raw = model.predict(X) if len(qs) else np.zeros(0)
        points = [rank.stats_points(schema, q) for q in qs]
        s = raw + np.array(points) if len(qs) else raw
        ids = [identity(schema, q) for q in qs]
        doc = {
            "snapshot": str(path.parent.name),
            "engine": snap.get("engine", ""),
            "tables": [{"name": t.name, "role": t.role, "timed": t.timed, "subject": t.subject, "person": t.person,
                        "label": t.label, "out": [list(x) for x in t.out]} for t in schema.tables],
            "generated": [{"title": q.title(schema), "shape": q.shape, "veto": v} for q, v in zip(all_qs, vetoes)],
            "questions": [{"title": q.title(schema), "ident": ident(i), "shape": q.shape, "sql": q.sql(schema),
                           "points": p, "raw": float(r), "slots": sl}
                          for q, i, p, r, sl in zip(qs, ids, points, raw, row["slots"])],
            "features": [{k: num(v) for k, v in f.items()} for f in feats[:FEATURES]],
            "five": rank.choose(schema, qs, list(s)) if qs else [],
            "screen": roles.screen(schema, qs, ids, s, row["slots"], mw) if qs else [],
        }
        (out / f"{path.parent.name}.json").write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
        print(path.parent.name, len(all_qs), "generated,", len(qs), "kept", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), [Path(p) for p in sys.argv[2:]])
