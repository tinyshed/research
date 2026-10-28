"""Round 1k's comparison: does the look learn more of what the judges keep
when GitTables' panels join the 93 databases' in its training?

    python -I compare.py prepare <out>
    python -I compare.py run <out>        # in the Linux container, the harness committed

prepare builds the look's 127 numbers for every GitTables panel exactly as
round 1j's prepare built them for the 93 databases, and checks that path
against round 1j's frozen arrays on the 93 themselves; it fits nothing. run
fits round 1j's baseline look (150 trees of four leaves) over round 1j's
family folds and draws, the GitTables panels in every training fold and
never counted, and counts each configuration as round 1j counts it.

Configurations:
  old          the 93 alone, taught by Jev, DeepSeek and GLM, as round 1j's baseline
  new          the 93 alone, taught by Jev (fast) and Luna (a chance)
  new+gt       the same, with GitTables' panels taught by the same two
  old+gt       the 93 taught as in old, GitTables' by Jev (fast) and Luna
  old+head     old, with each place's head word read apart from its modifiers (ideas/head.py)
  old+spread   old, with a summed or averaged column's spread, zeros and magnitude (ideas/extensive.py)
  old+both     old, with both
"""

from __future__ import annotations

import hashlib
import json
import math
import platform
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import local  # noqa: E402  (module())

TEACHERS = ("jevfast", "lunap")
CONFIGS = ("old", "new", "new+gt", "old+gt", "old+head", "old+spread", "old+both")
# the ideas' numbers beside round 1j's, on the 93 alone (ideas/features.py)
FEATURES = ROOT / "corpus" / "round-1k" / "ideas" / "features.npz"


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def mean_keep(ans: dict, pid: str) -> float:
    got = [float(ans[j][pid]["p_keep"]) for j in TEACHERS if pid in ans[j] and "p_keep" in ans[j][pid]]
    return float(np.mean(got)) if got else math.nan


def prepare(out: Path) -> None:
    joint = local.module("compare_joint", ROOT / "round-1j" / "joint.py")
    dg = local.module("compare_diagnose", ROOT / "round-1g" / "diagnose.py")
    look1d, look1e = dg.look1d, dg.look1e
    frozen = ROOT / "corpus" / "round-1j"
    meta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    arrays = dict(np.load(frozen / "data.npz", allow_pickle=False))
    names = meta["numeric_names"]
    question = names[len(look1d.RESULT) + 2:len(names) - len(look1e.MORE)]

    def vectors(look: Path, see_dir: Path, fes: list[str]) -> tuple[list, list]:
        look1d.QUESTION[:] = question
        rows, ids = [], []
        for fe in fes:
            _, qfeat = look1e.questions(see_dir, fe)
            for p in json.loads((look / fe / "pool.json").read_text(encoding="utf-8"))["panels"]:
                if "round 1" in p["in"]:
                    continue
                p["question"] = qfeat.get(p["title"], {})
                p["more"] = look1e.more(p)
                rows.append(look1e.vector(p))
                ids.append(p["id"])
        return rows, ids

    # the path checked on the 93: the same numbers as round 1j's frozen ones
    by_id = {p["id"]: i for i, p in enumerate(meta["panels"])}
    worst = 0.0
    for d in ("d", "e", "f"):
        look = ROOT / "corpus" / f"round-1{d}" / "look"
        fes = sorted({p["db"] for p in meta["panels"]} & {q.parent.name for q in look.glob("*/pool.json")})[:3]
        rows, ids = vectors(look, ROOT / "corpus" / "see", fes)
        for r, pid in zip(rows, ids):
            a, b = np.asarray(r, dtype=np.float32), arrays["base"][by_id[pid]]
            same = (np.isnan(a) & np.isnan(b)) | (a == b)
            worst = max(worst, float(np.nanmax(np.abs(np.where(same, 0, a - b)))) if (~same).any() else 0.0)
    if worst > 1e-5:
        raise SystemExit(f"The GitTables path differs from round 1j's numbers by {worst}")

    ans93 = {j: {} for j in TEACHERS}
    for d in ("d", "e", "f"):
        for j in TEACHERS:
            ans93[j] |= look1d.labels(ROOT / "corpus" / f"round-1{d}" / "look", j)
    y_new = np.asarray([mean_keep(ans93, p["id"]) for p in meta["panels"]])

    # the judges as they now stand: Sonnet and Gemini answered all 93 on 29 October
    judged = {j: {} for j in dg.COUNTERS}
    for d in ("d", "e", "f"):
        for j in dg.COUNTERS:
            judged[j] |= look1d.labels(ROOT / "corpus" / f"round-1{d}" / "look", j)
    judges = np.asarray([[float(bool(judged[j][p["id"]]["keep"])) if p["id"] in judged[j] else np.nan
                          for j in dg.COUNTERS] for p in meta["panels"]])

    gt = ROOT / "corpus" / "round-1k" / "gt"
    fes = sorted(q.parent.name for q in (gt / "look").glob("*/pool.json"))
    rows, ids = vectors(gt / "look", gt / "see", fes)
    ans = {j: look1d.labels(gt / "look", j) for j in TEACHERS}
    y_gt = np.asarray([mean_keep(ans, pid) for pid in ids])
    out.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(out / "data.npz", base_gt=np.asarray(rows, dtype=np.float32), y_gt=y_gt,
                        db_gt=np.asarray([pid.split(":")[0] for pid in ids]), y_new=y_new, judges=judges)
    inputs = [frozen / "data.npz", frozen / "data.json"]
    inputs += sorted((gt / "look").glob("*/pool.json")) + sorted((gt / "see").glob("*/snapshot.json"))
    for d in ("d", "e", "f"):
        for j in ("jevfast", "lunap") + tuple(dg.COUNTERS):
            inputs += sorted((ROOT / "corpus" / f"round-1{d}" / "look").glob(f"{j}-*.json"))
    inputs += sorted((gt / "look").glob("jevfast-*.json")) + sorted((gt / "look").glob("lunap-*.json"))
    inputs += [FEATURES]
    manifest = {"environment": joint.environment(), "checked_against_round_1j": worst,
                "panels": {"93": len(y_new), "gittables": len(y_gt), "gittables tables": len(fes)},
                "judged panels": {j: int(np.isfinite(judges[:, k]).sum()) for k, j in enumerate(dg.COUNTERS)},
                "labelled": {"93": int(np.isfinite(y_new).sum()), "gittables": int(np.isfinite(y_gt).sum())},
                "inputs": {str(p.relative_to(ROOT)).replace("\\", "/"): sha(p) for p in inputs},
                "data_sha256": sha(out / "data.npz"), "script_sha256": sha(Path(__file__))}
    joint.write_json(out / "data.json", manifest)
    print(json.dumps({k: manifest[k] for k in ("checked_against_round_1j", "panels", "labelled")}))


def run(out: Path) -> None:
    if platform.system() != "Linux":
        raise SystemExit("Learning comparisons run in the Linux container.")
    if subprocess.check_output(["git", "-C", str(ROOT), "status", "--porcelain", "--", "round-1k"], text=True).strip():
        raise SystemExit("Commit the round-1k harness before measuring it.")
    joint = local.module("compare_joint", ROOT / "round-1j" / "joint.py")
    dg = local.module("compare_diagnose", ROOT / "round-1g" / "diagnose.py")
    sets = local.module("compare_sets", ROOT / "round-1g" / "sets.py")
    meta = json.loads((out / "data.json").read_text(encoding="utf-8"))
    if sha(out / "data.npz") != meta["data_sha256"] or sha(Path(__file__)) != meta["script_sha256"]:
        raise SystemExit("Inputs or harness changed; prepare again.")
    frozen = ROOT / "corpus" / "round-1j"
    jmeta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    A = dict(np.load(frozen / "data.npz", allow_pickle=False))
    G = dict(np.load(out / "data.npz", allow_pickle=False))
    base, gt = A["base"], G["base_gt"]
    X = np.vstack((base, gt))
    n93, ngt = len(base), len(gt)
    if meta["inputs"].get(str(FEATURES.relative_to(ROOT)).replace("\\", "/")) != sha(FEATURES):
        raise SystemExit("The ideas' features changed; prepare again.")
    F = dict(np.load(FEATURES, allow_pickle=False))
    extra = {"old+head": F["head"], "old+spread": F["spread"], "old+both": np.column_stack((F["head"], F["spread"]))}
    targets = {"old": (A["y"], np.full(ngt, np.nan)), "new": (G["y_new"], np.full(ngt, np.nan)),
               "new+gt": (G["y_new"], G["y_gt"]), "old+gt": (A["y"], G["y_gt"])}
    scores = {c: np.full((joint.DRAWS, n93), np.nan) for c in CONFIGS}
    for draw, fold, train, test in joint.folds(A["family"].tolist(), joint.DRAWS):
        tr = np.r_[train, np.ones(ngt, dtype=bool)]
        te = np.r_[test, np.zeros(ngt, dtype=bool)]
        # round 1j's seed, so that old repeats round 1j's baseline
        seed = joint.SEED + 100 * draw + fold
        for c in CONFIGS:
            if c in extra:
                pred, _ = joint.fit_predict("baseline", np.column_stack((base, extra[c])), A["y"], train, test, seed)
            else:
                pred, _ = joint.fit_predict("baseline", X, np.concatenate(targets[c]), tr, te, seed)
            scores[c][draw, test] = pred
        print(f"draw {draw + 1} fold {fold + 1}", flush=True)
    dbs = A["db"]
    db_indices = {db: np.flatnonzero(dbs == db) for db in sorted(set(dbs.tolist()))}
    family_by_db = {db: str(A["family"][ix[0]]) for db, ix in db_indices.items()}
    metrics = {}
    for c in CONFIGS:
        per = {}
        for j, judge in enumerate(jmeta["judges"]):
            for policy, penalties in (("plain", dg.NO_PENALTY), ("axes", sets.AXIS_ONCE)):
                for db, ix in db_indices.items():
                    # the judges' answers as prepare read them, not round 1j's frozen fifty
                    y = G["judges"][ix, j]
                    if not np.isfinite(y).all():
                        continue
                    chooser = dg.look1e.Chooser([jmeta["panels"][int(i)]["identity"] for i in ix])
                    aa, kk = [], []
                    for prediction in scores[c][:, ix]:
                        aa.append(joint.auc(y, prediction))
                        kk.append(float(y[chooser.choose(prediction, penalties)].sum()))
                    for name, vals in (("auc", aa), ("kept", kk)):
                        vals = [v for v in vals if math.isfinite(v)]
                        if vals:
                            per.setdefault(f"{judge}/{policy}/{name}", {})[db] = float(np.mean(vals))
        # Sonnet and Gemini together: the mean of the two, where both answered
        for key in [k for k in per if k.startswith("sonnet/")]:
            rest = key.split("/", 1)[1]
            both = {db: (v + per[f"gemini/{rest}"][db]) / 2 for db, v in per[key].items() if db in per.get(f"gemini/{rest}", {})}
            per[f"sonnet+gemini/{rest}"] = both
        metrics[c] = per
    result = {"environment": joint.environment(), "input_sha256": meta["data_sha256"],
              "panels": {"93": n93, "gittables": ngt}, "configs": CONFIGS,
              "scope": "development-family cross-validation on the 93; GitTables only ever trains"}
    for c in CONFIGS:
        result[c] = {k: {"mean": float(np.mean(list(v.values()))), "databases": len(v)} for k, v in metrics[c].items()}
    for c, against in (("new+gt", "new"), ("old+gt", "old"), ("new", "old"), ("old+head", "old"),
                       ("old+spread", "old"), ("old+both", "old")):
        for k, v in metrics[c].items():
            common = {db: x - metrics[against][k][db] for db, x in v.items() if db in metrics[against][k]}
            result[c][k][f"minus_{against}"] = joint.paired_interval(common, family_by_db)
    joint.write_json(out / "result.json", result)
    for c in CONFIGS:
        row = [f"{result[c][f'{j}/plain/kept']['mean']:.2f}" for j in list(jmeta["judges"]) + ["sonnet+gemini"]]
        print(c, "kept of five", row)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, out = sys.argv[1], Path(sys.argv[2])
    if cmd == "prepare":
        prepare(out)
    elif cmd == "run":
        run(out)
