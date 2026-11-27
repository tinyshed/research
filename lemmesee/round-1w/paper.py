"""Round 1w: the paper's numbers for `eleven--wide` that need its pairs built
again, each through the code that measured it first for the earlier models.

    python -I paper.py model     # the frozen model's files in one folder, as the earlier rounds load a model
    python -I paper.py perm      # round 1s's refit.score: SchemaPile-Perm's test and validation,
                                 # the pair task, related or not, the threshold grid, the external sets
    python -I paper.py nexus     # round 1p's nexus.py: TPC-H, TPC-DS, BIRD's development set
    python -I paper.py owner     # the owner's database A, counts only

Each runs the earlier round's function as it is. Those functions build a
model's pairs through `widen.build` and `widen.build_all` and read its
second stage through `graph.graph_features`; here the first two give version
11's pairs (round 1v's ten.py, the people of a schema on) and the third
gives the wide stage's features (round 1v's wide.py), which need the number
of tables of each schema the pairs were built from: a build keeps it on the
list of pairs it returns. One job at a time: building pairs takes the
machine's memory. Writes under corpus/round-1w/paper/.
"""

import hashlib
import json
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1v"))

import first as first_stage  # noqa: E402  (round 1v's: the earlier rounds on the path, the trigrams sorted)

import graph  # noqa: E402
import keys  # noqa: E402
import letters  # noqa: E402
import ten  # noqa: E402
import wide  # noqa: E402
import widen  # noqa: E402

MODEL = "eleven--wide"
CORPUS = HERE.parent / "corpus"
V = CORPUS / "round-1v"
OUT = CORPUS / "round-1w" / "paper"
DIR = CORPUS / "round-1w" / "model" / MODEL
graph.WORKERS = 8


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def model() -> Path:
    """The frozen model's two stages and its threshold in one folder, checked against round 1v's frozen.json."""
    frozen = json.load(open(V / "frozen.json", encoding="utf-8"))["models"][MODEL]
    first = MODEL.split("--")[0]
    src = {"keys-first.ubj": V / "first" / first / "keys-first.ubj",
           "keys-second.txt": V / "second" / MODEL / "keys-second.txt",
           "second.json": V / "second" / MODEL / "second.json"}
    DIR.mkdir(parents=True, exist_ok=True)
    for name, p in src.items():
        assert sha(p) == frozen[name], f"{name} changed since {MODEL} was frozen"
        if not (DIR / name).exists() or sha(DIR / name) != frozen[name]:
            shutil.copy(p, DIR / name)
    result = json.load(open(DIR / "second.json", encoding="utf-8"))
    assert result["kind"] == "wide" and first_stage.FITS[first].get("people")
    (DIR / "graph-final.json").write_text(json.dumps(
        {"threshold": result["threshold"], "variant": "eleven", "graph": "wide", "frozen": frozen}, indent=1) + "\n",
        encoding="utf-8")
    return DIR


class Pairs(list):
    """A build's pairs, with the number of tables of each schema they were built from."""
    sizes: dict


def use_eleven() -> None:
    assert letters.is_sorted()
    plain = graph.graph_features
    inside = []

    def build(job):
        x, meta = ten.build((*job[:3], True))
        meta = Pairs(meta)
        meta.sizes = {s["id"]: len(s["tables"]) for s in job[0]}
        return x, meta

    def build_all(pool, schemas, split, guesses, variant=None):
        x, meta = ten.build_all(pool, schemas, split, guesses, True)
        meta = Pairs(meta)
        meta.sizes = {s["id"]: len(s["tables"]) for s in schemas if s["split"] == split and s["fk"]}
        return x, meta

    def features(meta, scores):
        if inside:  # the wide stage reads round 1b's nineteen through this name too
            return plain(meta, scores)
        inside.append(1)
        try:
            return wide.KINDS["wide"](meta, scores, meta.sizes)
        finally:
            inside.pop()

    widen.build, widen.build_all, graph.graph_features = build, build_all, features


def perm() -> None:
    sys.path.insert(0, str(HERE.parent / "round-1s"))
    import refit  # noqa: E402
    d = model()
    use_eleven()
    (OUT / "external").mkdir(parents=True, exist_ok=True)
    for f in ("hidden.jsonl", "guesses.jsonl", "bench.jsonl", "t5-negatives.jsonl"):
        shutil.copy(CORPUS / "round-1s" / "external" / f, OUT / "external" / f)
    refit.scorings = lambda round_1p, out: {MODEL: (d, "eleven", "sorted", None, ("validation", "test", "external"))}
    refit.score(CORPUS / "round-1b", CORPUS / "round-1p", OUT, MODEL)


def nexus() -> None:
    import nexus as nx  # noqa: E402  (round 1p's)
    d = model()
    use_eleven()
    OUT.mkdir(parents=True, exist_ok=True)
    nx.main(d, CORPUS / "round-1p", CORPUS, "eleven")


def owner() -> None:
    """Database A as round 1q's `owner.py draw` read it, counts only (round
    1t's paper.py owner): the keys drawn, those declared, those no
    declaration names, and of these how many were among the 36 round 1q's
    reading of the app's code judged."""
    sys.path.append(str(HERE.parent / "round-1q"))
    import external  # noqa: E402
    import prod  # noqa: E402
    d = model()
    use_eleven()
    snap = json.load(open(CORPUS / "prod" / "aimee" / "snapshot.json", encoding="utf-8"))
    s = prod.hidden(snap, "aimee")
    run, g = external.scorer(d, widen.build)
    thr = json.load(open(d / "graph-final.json", encoding="utf-8"))["threshold"]
    sc, meta = run([s], {})
    kept = keys.one_target(sc, [(m[g.SID], m[g.TABLE], m[g.COLUMN]) for m in meta])
    drawn = {(m[g.TABLE], m[g.COLUMN], m[g.TARGET]) for m, k in zip(meta, kept) if k >= thr}
    declared = {tuple(k) for k in s["fk"]}
    read = {tuple(k) for k in json.load(open(CORPUS / "round-1q" / "owner-aimee-drawn.json", encoding="utf-8"))["undeclared"]}
    undeclared = drawn - declared
    tp = len(drawn & declared)
    result = {"database": "A", "model": MODEL, "tables": len(s["tables"]),
              "declared one-column keys": len(declared), "drawn": len(drawn),
              "drawn and declared": tp, "drawn, no declaration": len(undeclared),
              "of them among the 36 round 1q's reading judged": len(undeclared & read),
              "of them not read": len(undeclared - read),
              "F1 over the declared keys": round(2 * tp / (len(drawn) + len(declared)), 4) if drawn else 0.0,
              "precision, declared only": round(tp / len(drawn), 4) if drawn else 0.0,
              "recall": round(tp / len(declared), 4) if declared else 0.0}
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "owner-A.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    {"model": model, "perm": perm, "nexus": nexus, "owner": owner}[sys.argv[1]]()
