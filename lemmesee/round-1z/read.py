"""Round 1z: the networks' thresholds, their freezing, and their one reading of each set.

    python -I read.py threshold <name> [...]    the validation split's best threshold, round 1b's function
    python -I read.py freeze <name> [...]       by hash, before a clean set's candidates are built
    python -I read.py candidates <set>          plain-a or style3: lemmekey's candidates, built once (pairs.lock)
    python -I read.py external                  the kept pairs of Spider, BIRD, CTU and LiveSQLBench, no pair built
    python -I read.py read <set>                the frozen networks' keys; refuses a set read before

Runs in round 1b's environment. A network's scores of a set are made by
net.py in the round's own (`net.py score <name> sets/<set>.pkl
nets/<name>/scores/<set>.npy`); `read` takes every frozen network's, keeps
each column's best target (whole.one_target) and draws it at the
network's frozen threshold.

`candidates` builds a clean set's pairs with round 1v's own functions
(clean.raw_new: version 11's pairs, `eleven`'s first stage, the wide
second) and checks that the keys they draw are the ones round 1v's reading
kept. It refuses to run before the networks are frozen. Two tree readings
go beside the networks from the same rows: `eleven--wide` again (it must
give round 1v's counts) and `eleven` alone, the first stage under the same
decision at the threshold its validation scores choose.
"""

import hashlib
import importlib.util
import json
import pickle
import sys
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1v"))

import numpy as np  # noqa: E402

CORPUS = HERE.parent / "corpus"
V = CORPUS / "round-1v"
OUT = CORPUS / "round-1z"
NETS = OUT / "nets"
FIRST = V / "first" / "eleven"
TREES = ("eleven--wide", "eleven, first stage alone")
EXTERNAL = ("spider", "bird", "ctu", "livesqlbench-development")


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def round_1v():
    """Round 1v's clean.py, by its path (the earlier rounds come onto the path with it)."""
    spec = importlib.util.spec_from_file_location("clean_1v", HERE.parent / "round-1v" / "clean.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def threshold(names: list) -> None:
    import whole  # noqa: E402  (it puts round 1b on the path)
    import keys  # noqa: E402
    vrec = pickle.load(open(FIRST / "validation.pkl", "rb"))["pairs"]
    y = vrec["label"].astype(bool)
    for name in names:
        if name == "eleven-first":
            scores, dst = vrec["first"], OUT / "eleven-first-threshold.json"
        else:
            scores, dst = np.load(NETS / name / "validation.npy"), NETS / name / "threshold.json"
        kept = whole.one_target(scores.astype(np.float64), vrec)
        th = keys.best_threshold(y, kept)
        p, r, f = keys.at_threshold(y, kept, th)
        res = {"name": name, "threshold": th, "validation pairs": int(len(y)), "precision": float(p), "recall": float(r),
               "f1": float(f), "read at": now()}
        dst.write_text(json.dumps(res, indent=1) + "\n", encoding="utf-8")
        print(json.dumps(res))


def files(name: str) -> dict:
    d = NETS / name
    return {f: sha(d / f) for f in ("model.safetensors", "threshold.json", "fit.json")}


def freeze(names: list) -> None:
    path = OUT / "frozen.json"
    frozen = json.load(open(path, encoding="utf-8")) if path.exists() else {"models": {}}
    for which in ("plain-a", "style3"):
        assert not (OUT / "sets" / f"{which}.pkl").exists() or all(n in frozen["models"] for n in names), \
            "a clean set's candidates are built already: nothing more is frozen"
    for n in names:
        assert n not in frozen["models"], f"{n} is frozen already"
        frozen["models"][n] = {"files": files(n), "frozen at": now(),
                               "threshold": json.load(open(NETS / n / "threshold.json", encoding="utf-8"))["threshold"]}
    frozen["eleven, first stage alone"] = json.load(open(OUT / "eleven-first-threshold.json", encoding="utf-8"))
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(frozen, indent=1))


def check_frozen() -> dict:
    frozen = json.load(open(OUT / "frozen.json", encoding="utf-8"))
    for n, f in frozen["models"].items():
        assert files(n) == f["files"], f"{n} changed since it was frozen"
    return frozen


def candidates(which: str) -> None:
    frozen = check_frozen()
    assert frozen["models"], "no network is frozen"
    dst = OUT / "sets" / f"{which}.pkl"
    assert not dst.exists(), "this set's candidates are built already"
    clean = round_1v()
    import graph  # noqa: E402
    import letters  # noqa: E402
    assert letters.is_sorted()
    hidden = V / f"heldout-github-{which}" / "hidden.jsonl"
    was = json.load(open(V / "frozen.json", encoding="utf-8"))
    assert sha(hidden) == was[which], "the set is not the one round 1v froze"
    for n in ("eleven--wide",):
        assert clean.files(n) == was["models"][n], f"{n} changed since round 1v froze it"
    schemas = [json.loads(line) for line in open(hidden, encoding="utf-8")]
    cache = {}
    with letters.pool(graph.WORKERS) as pool:
        rec, raw, th = clean.raw_new(pool, "eleven--wide", schemas, cache)
    rec = dict(rec)
    rec["second"] = np.asarray(raw, dtype=np.float64)
    rec.pop("dense", None)
    dst.parent.mkdir(parents=True, exist_ok=True)
    pickle.dump({"set": which, "schemas": schemas, "pairs": rec, "built at": now(), "eleven--wide threshold": th},
                open(dst, "wb"), protocol=4)
    # the keys lemmekey draws from these rows against the keys round 1v's reading kept
    import second as second_stage  # noqa: E402
    import whole  # noqa: E402
    preds = second_stage.predictions(rec, schemas, whole.one_target(rec["second"], rec), th)
    mine = [sorted([a, c, g] for (a, c), g in p.items()) for p in preds]
    kept = json.load(open(V / f"heldout-{which}-drawn.json", encoding="utf-8"))["eleven--wide"]
    same = sum(m == sorted(k) for m, k in zip(mine, kept))
    print(f"{which}: {len(schemas)} schemas, {len(rec['label'])} pairs, {int(rec['label'].sum())} keys among them; "
          f"lemmekey draws round 1v's keys in {same} of {len(schemas)} schemas", flush=True)
    assert same == len(schemas), "the candidates are not the ones round 1v's reading scored"


def external() -> None:
    (OUT / "sets").mkdir(parents=True, exist_ok=True)
    th = json.load(open(V / "second" / "eleven--wide" / "second.json", encoding="utf-8"))["threshold"]
    for slug in EXTERNAL:
        data = pickle.load(open(V / "sets" / f"{slug}.pkl", "rb"))
        rec = pickle.load(open(FIRST / f"{slug}.pkl", "rb"))["pairs"]
        rec["second"] = np.load(V / "second" / "eleven--wide" / "sets" / f"{slug}.npy").astype(np.float64)
        rec.pop("dense", None)
        pickle.dump({"set": slug, "schemas": data["schemas"], "pairs": rec, "eleven--wide threshold": th},
                    open(OUT / "sets" / f"{slug}.pkl", "wb"), protocol=4)
        print(f"{slug}: {len(data['schemas'])} databases, {len(rec['label'])} pairs", flush=True)


def read(which: str) -> None:
    import second as second_stage  # noqa: E402
    import whole  # noqa: E402
    frozen = check_frozen()
    done = OUT / "read" / f"{which}.json"
    if done.exists():
        raise SystemExit("this set has been read once already")
    data = pickle.load(open(OUT / "sets" / f"{which}.pkl", "rb"))
    schemas, rec = data["schemas"], data["pairs"]
    truth = [{tuple(k) for k in s["fk"]} for s in schemas]
    arms = {"eleven--wide": (rec["second"], data["eleven--wide threshold"]),
            "eleven, first stage alone": (rec["first"].astype(np.float64), frozen["eleven, first stage alone"]["threshold"])}
    for n, f in frozen["models"].items():
        path = NETS / n / "scores" / f"{which}.npy"
        assert path.exists(), f"{n} has not scored {which}"
        s = np.load(path).astype(np.float64)
        assert len(s) == len(rec["label"])
        arms[n] = (s, f["threshold"])
    result = {"set": which, "read at": now(), "databases": [s["id"] for s in schemas],
              "tables": [len(s["tables"]) for s in schemas], "pairs": int(len(rec["label"])),
              "keys among the pairs": int(rec["label"].sum()), "models": {}}
    drawn = {}
    for n, (scores, th) in arms.items():
        preds = second_stage.predictions(rec, schemas, whole.one_target(scores, rec), th)
        per = np.array([[len(t & {(a, c, g) for (a, c), g in p.items()}),
                         len({(a, c, g) for (a, c), g in p.items()} - t),
                         len(t - {(a, c, g) for (a, c), g in p.items()})] for t, p in zip(truth, preds)])
        result["models"][n] = {"threshold": th, "per database": per.tolist()}
        drawn[n] = [[[a, c, g] for (a, c), g in sorted(p.items())] for p in preds]
        p, r, f = whole.prf(*per.sum(axis=0))
        print(f"{which} {n:28} P {p:.3f} R {r:.3f} F1 {f:.3f}", flush=True)
    done.parent.mkdir(parents=True, exist_ok=True)
    done.write_text(json.dumps(result) + "\n", encoding="utf-8")
    (OUT / "read" / f"{which}-drawn.json").write_text(json.dumps(drawn) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "threshold":
        threshold(sys.argv[2:])
    elif cmd == "freeze":
        freeze(sys.argv[2:])
    elif cmd == "candidates":
        candidates(sys.argv[2])
    elif cmd == "external":
        external()
    else:
        read(sys.argv[2])
