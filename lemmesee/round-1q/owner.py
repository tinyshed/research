"""Round 1q: the key models on one of the owner's production databases,
step by step, every count from one draw of the models' keys. Only counts
leave the server and only counts are written outside the ignored corpus.

    python -I owner.py draw <prod corpus> <round 1p out> <database> <corpus out>
    python -I owner.py orphans <prod corpus> <database> <ssh target> <container> <corpus out>
    python -I owner.py inside <prod corpus> <database> <ssh target> <container> <corpus out>
    python -I owner.py links <database> <verdicts> <guesses> <corpus out> <data out> <label>

`draw` scores the reported and the widened model on the snapshot, its
declarations hidden, and keeps both models' keys in
<corpus out>/owner-<database>-drawn.json, with names: the corpus is ignored.
`orphans` counts on the server, for each undeclared key drawn, the column's
values and distinct values and the distinct values its target's primary key
does not hold; `inside`, from `pg_stats` alone, the share of the column's
sampled values within the range of the target's primary key and of each of
its unique columns. Both read-only, at most fifteen seconds a statement.
`links` joins a reading of the app's code — <verdicts>, a JSON list of
{"n", "verdict", "really_refers_to"} for the numbered <guesses> file of
lines `table.column -> target` — and writes <data out>/owner-<label>.json,
counts only: for each model, keys drawn that are declared, real links by the
code through the target's primary key or another of its unique columns,
wrong, and not read, with and without the drop against any unique key.

The server is reached by `ssh` (TINYSHED_SSH, else `ssh` on the path) with
the key TINYSHED_SSH_KEY (else ~/.ssh/id_ed25519) and, when set,
TINYSHED_SSH_PROXY as its ProxyCommand.
"""

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import prod  # noqa: E402

DROP = 0.5
OPTS = "-c default_transaction_read_only=on -c statement_timeout=500 -c lock_timeout=1000"


def q(name: str) -> str:
    return '"' + name.replace('"', '""') + '"'


def lit(s: str) -> str:
    return "'" + s.replace("'", "''") + "'"


def psql(snap: dict, ssh_target: str, container: str, sql: list[str]) -> list[list[str]]:
    ssh = os.environ.get("TINYSHED_SSH") or shutil.which("ssh") or "ssh"
    key = os.environ.get("TINYSHED_SSH_KEY") or str(Path.home() / ".ssh" / "id_ed25519")
    cmd = [ssh, "-o", "BatchMode=yes", "-o", "ConnectTimeout=20", "-i", key]
    if os.environ.get("TINYSHED_SSH_PROXY"):
        cmd += ["-o", "ProxyCommand=" + os.environ["TINYSHED_SSH_PROXY"]]
    line = (f"docker exec -i -e 'PGOPTIONS={OPTS}' {container} psql -U {snap['role']} -d {snap['database']} "
            f"-At -F '|' -v ON_ERROR_STOP=0 -f -")
    p = subprocess.run(cmd + [ssh_target, line], input="\n".join(["SET statement_timeout = '15s';"] + sql),
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    failed = sum(1 for e in p.stderr.splitlines() if "ERROR" in e)
    rows = [r.split("|") for r in p.stdout.splitlines() if r.split("|")[0].isdigit()]
    return rows, failed


def draw(prod_dir: Path, data: Path, db: str, out: Path) -> None:
    import external  # noqa: E402
    import keys  # noqa: E402
    import widen  # noqa: E402
    snap = json.load(open(prod_dir / db / "snapshot.json", encoding="utf-8"))
    s = prod.hidden(snap, db)
    drawn = {}
    for label, folder, v in (("reported", "models-1524905", None), ("widened", "variants/both", "both")):
        models = data / folder
        run, graph = external.scorer(models, (lambda job, v=v: widen.build((*job, v))) if v else None)
        thr = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
        sc, meta = run([s], {})
        kept = keys.one_target(sc, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
        drawn[label] = sorted([m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]] for m, k in zip(meta, kept) if k >= thr)
    declared = sorted(list(k) for k in s["fk"])
    undeclared = sorted({tuple(k) for ks in drawn.values() for k in ks} - {tuple(k) for k in declared})
    out.mkdir(parents=True, exist_ok=True)
    (out / f"owner-{db}-drawn.json").write_text(json.dumps(
        {"declared": declared, "drawn": drawn, "undeclared": [list(k) for k in undeclared]}, indent=1), encoding="utf-8")
    print({k: len(v) for k, v in drawn.items()}, "declared", len(declared), "undeclared drawn by either", len(undeclared))


def unique_columns(snap: dict) -> dict:
    return {t["name"]: sorted({c["columns"][0] for c in t.get("constraints", [])
                               if c["kind"] in ("p", "u") and len(c["columns"]) == 1}) for t in snap["tables"]}


def orphans(prod_dir: Path, db: str, ssh_target: str, container: str, out: Path) -> None:
    snap = json.load(open(prod_dir / db / "snapshot.json", encoding="utf-8"))
    sch = {t["name"]: t["schema"] for t in snap["tables"]}
    pk = {t["name"]: next((c["columns"] for c in t.get("constraints", []) if c["kind"] == "p"), [])
          for t in snap["tables"]}
    und = json.load(open(out / f"owner-{db}-drawn.json", encoding="utf-8"))["undeclared"]
    sql = []
    for n, (t, c, r) in enumerate(und):
        if len(pk.get(r, [])) != 1:
            continue
        T, R, k = f"{q(sch[t])}.{q(t)}", f"{q(sch[r])}.{q(r)}", pk[r][0]
        sql.append(f"SELECT {n}, (SELECT count({q(c)}) FROM {T}), (SELECT count(DISTINCT {q(c)}) FROM {T}), "
                   f"(SELECT count(*) FROM (SELECT {q(c)}::text FROM {T} WHERE {q(c)} IS NOT NULL "
                   f"EXCEPT SELECT {q(k)}::text FROM {R}) z);")
    rows, failed = psql(snap, ssh_target, container, sql)
    counts = {int(r[0]): {"values": int(r[1]), "distinct": int(r[2]), "distinct outside the primary key": int(r[3])}
              for r in rows}
    (out / f"owner-{db}-orphans.json").write_text(json.dumps({"undeclared": len(und), "counted": counts,
                                                               "failed": failed}, indent=1), encoding="utf-8")
    print(len(und), "undeclared;", len(counts), "counted;", failed, "failed")


def inside(prod_dir: Path, db: str, ssh_target: str, container: str, out: Path) -> None:
    snap = json.load(open(prod_dir / db / "snapshot.json", encoding="utf-8"))
    sch = {t["name"]: t["schema"] for t in snap["tables"]}
    uniq = unique_columns(snap)
    d = json.load(open(out / f"owner-{db}-drawn.json", encoding="utf-8"))
    keys_ = sorted({tuple(k) for ks in d["drawn"].values() for k in ks})

    def vals(s_, t_, a_):
        return (f"SELECT unnest(coalesce(most_common_vals::text::text[], '{{}}') || "
                f"coalesce(histogram_bounds::text::text[], '{{}}')) v FROM pg_stats "
                f"WHERE schemaname = {lit(s_)} AND tablename = {lit(t_)} AND attname = {lit(a_)}")
    sql, which = [], []
    for n, (t, c, r) in enumerate(keys_):
        for u in uniq.get(r, []):
            which.append((n, u))
            sql.append(f"WITH c AS ({vals(sch[t], t, c)}), k AS ({vals(sch[r], r, u)}), "
                       f"num AS (SELECT bool_and(v ~ '^-?[0-9]+$') a FROM (SELECT v FROM c UNION ALL SELECT v FROM k) z) "
                       f"SELECT {len(which) - 1}, (SELECT count(*) FROM c), (SELECT count(*) FROM k), "
                       f"CASE WHEN (SELECT a FROM num) THEN (SELECT avg((v::numeric BETWEEN (SELECT min(v::numeric) FROM k) "
                       f"AND (SELECT max(v::numeric) FROM k))::int) FROM c) ELSE (SELECT avg((v BETWEEN (SELECT min(v) FROM k) "
                       f"AND (SELECT max(v) FROM k))::int) FROM c) END;")
    rows, failed = psql(snap, ssh_target, container, sql)
    pkeys = {t["name"]: next((c["columns"][0] for c in t.get("constraints", []) if c["kind"] == "p"
                              and len(c["columns"]) == 1), None) for t in snap["tables"]}
    result = {}
    for r in rows:
        n, u = which[int(r[0])]
        if r[3] == "":
            continue
        e = result.setdefault(n, {"against the primary key": None, "against any unique column": 0.0})
        share = float(r[3])
        e["against any unique column"] = max(e["against any unique column"], share)
        if u == pkeys.get(keys_[n][2]):
            e["against the primary key"] = share
    (out / f"owner-{db}-inside.json").write_text(json.dumps({"keys": len(keys_), "with a sample": result,
                                                              "failed": failed}, indent=1), encoding="utf-8")
    print(len(keys_), "keys drawn;", len(result), "with a sample on both sides;", failed, "failed")


def links(db: str, verdicts: Path, guesses: Path, out: Path, data_out: Path, label: str) -> None:
    d = json.load(open(out / f"owner-{db}-drawn.json", encoding="utf-8"))
    ins = json.load(open(out / f"owner-{db}-inside.json", encoding="utf-8"))["with a sample"]
    keys_ = sorted({tuple(k) for ks in d["drawn"].values() for k in ks})
    share = {keys_[int(n)]: v["against any unique column"] for n, v in ins.items()}
    listed = [tuple(re.match(r"(\S+)\.(\S+) -> (\S+)", ln).groups()) for ln in guesses.read_text().splitlines() if ln.strip()]
    read = {}
    for e in json.load(open(verdicts, encoding="utf-8")):
        g = listed[e["n"] - 1]
        if e["verdict"] == "yes":
            read[g] = "link through the primary key"
        elif e.get("really_refers_to") == g[2]:
            read[g] = "link through another unique column"
        else:
            read[g] = "wrong"
    declared = {tuple(k) for k in d["declared"]}
    result = {"guesses read in the code": len(read),
              "the reading": {v: sum(1 for x in read.values() if x == v) for v in sorted(set(read.values()))}}
    for model, ks in d["drawn"].items():
        ks = {tuple(k) for k in ks}
        for how, sel in (("alone", ks), ("drop against any unique column",
                                         {k for k in ks if not (k in share and share[k] < DROP)})):
            row = {"drawn": len(sel), "declared": len(sel & declared)}
            for v in ("link through the primary key", "link through another unique column", "wrong"):
                row[v] = sum(1 for k in sel - declared if read.get(k) == v)
            row["not read"] = sum(1 for k in sel - declared if k not in read)
            right = row["declared"] + row["link through the primary key"] + row["link through another unique column"]
            row["precision, counting links right"] = right / len(sel) if sel else 0.0
            row["precision, counting only declarations right"] = row["declared"] / len(sel) if sel else 0.0
            result[f"{model}, {how}"] = row
    wrong = {k for k, v in read.items() if v == "wrong"}
    linked = {k for k, v in read.items() if v != "wrong"}
    result["the drop removes"] = {"wrong guesses": sum(1 for k in wrong if k in share and share[k] < DROP),
                                  "wrong guesses with a sample": sum(1 for k in wrong if k in share),
                                  "links": sum(1 for k in linked if k in share and share[k] < DROP),
                                  "links with a sample": sum(1 for k in linked if k in share)}
    data_out.mkdir(parents=True, exist_ok=True)
    (data_out / f"owner-{label}.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    a = sys.argv
    if a[1] == "draw":
        draw(Path(a[2]), Path(a[3]), a[4], Path(a[5]))
    elif a[1] == "orphans":
        orphans(Path(a[2]), a[3], a[4], a[5], Path(a[6]))
    elif a[1] == "inside":
        inside(Path(a[2]), a[3], a[4], a[5], Path(a[6]))
    else:
        links(a[2], Path(a[3]), Path(a[4]), Path(a[5]), Path(a[6]), a[7])
