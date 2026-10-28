"""The pipeline on a living database, for its owner to judge: the pool as
round 1d builds it, its statements run through a read-only session on the
database's own host, the look taught on the 93 public databases alone, and
three fives shown blind side by side.

    python -I live.py pool <see dir> <fixture> <ssh target> <container>
    python -I live.py page <see dir> <fixture>...

<see dir>/<fixture> holds round 1's snapshot.json and runs.json of that
database. Every statement runs as `docker exec <container> psql` on the
host, with default_transaction_read_only on, a statement timeout of half a
second and a lock timeout of one; a statement past it is a panel not run.
Nothing is learned from the database and nothing read leaves this machine:
pools, scores, the page and its key stay in <see dir>/look, which is not
committed. No host, table or column is named here.
"""

from __future__ import annotations

import json
import math
import random
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import local  # noqa: E402

KEY = r"C:\Users\{user}\.ssh\id_ed25519"
NULL = "<<null>>"
SEED = 20261104


def remote(ssh: str, container: str, role: str, database: str):
    """see.run_sql for a database on its own host, read only."""
    import os
    key = KEY.format(user=os.environ.get("USERNAME", ""))
    opts = "-c default_transaction_read_only=on -c statement_timeout=500 -c lock_timeout=1000"

    # every word quoted for the remote shell: an unquoted <<null>> is a here-document there
    import shlex
    line = (f"docker exec -i -e {shlex.quote('PGOPTIONS=' + opts)} {shlex.quote(container)} psql "
            f"-U {shlex.quote(role)} -d {shlex.quote(database)} -At -F $'\\t' -P {shlex.quote('null=' + NULL)} "
            f"-v ON_ERROR_STOP=1 -f -")

    def run_sql(engine: str, _database: str, sql: str):
        t = time.perf_counter()
        cmd = ["ssh.exe", "-i", key, "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", ssh, line]
        p = subprocess.run(cmd, input=sql, capture_output=True, text=True, encoding="utf-8", errors="replace")
        ms = (time.perf_counter() - t) * 1000
        if p.returncode != 0:
            return [], (p.stderr.strip().splitlines() or ["error"])[-1][:300], ms
        rows = [[None if v in (NULL, "NULL") else v for v in line.split("\t")]
                for line in p.stdout.splitlines() if line.strip()]
        return rows, None, ms

    return run_sql


def pool(see_dir: Path, fixture: str, ssh: str, container: str) -> None:
    look1d = local.module("live_look1d", ROOT / "round-1d" / "look.py")
    snap = json.loads((see_dir / fixture / "snapshot.json").read_text(encoding="utf-8"))
    look1d.see.run_sql = remote(ssh, container, snap["role"], snap["database"])
    look1d.pool(ROOT / "corpus" / "round-1c" / "learn", ROOT / "corpus" / "round-1d", see_dir, see_dir / "look",
                [fixture], 10)


def look_scores(see_dir: Path, fixtures: list[str]) -> dict:
    """The baseline look of round 1j, taught on all 93 public databases by
    their teachers' mean, scoring each fixture's panels."""
    sys.argv = [sys.argv[0]]
    joint = local.module("live_joint", ROOT / "round-1j" / "joint.py")
    dg = local.module("live_diagnose", ROOT / "round-1g" / "diagnose.py")
    look1d, look1e = dg.look1d, dg.look1e
    frozen = ROOT / "corpus" / "round-1j"
    meta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    A = dict(np.load(frozen / "data.npz", allow_pickle=False))
    names = meta["numeric_names"]
    look1d.QUESTION[:] = names[len(look1d.RESULT) + 2:len(names) - len(look1e.MORE)]
    out = {}
    for fe in fixtures:
        _, qfeat = look1e.questions(see_dir, fe)
        panels = json.loads((see_dir / "look" / fe / "pool.json").read_text(encoding="utf-8"))["panels"]
        mine = [p for p in panels if "round 1" not in p["in"]]
        for p in mine:
            p["question"] = qfeat.get(p["title"], {})
            p["more"] = look1e.more(p)
        X = np.vstack((A["base"], np.asarray([look1e.vector(p) for p in mine], dtype=np.float32)))
        y = np.r_[A["y"], np.full(len(mine), np.nan)]
        train = np.r_[np.ones(len(A["base"]), dtype=bool), np.zeros(len(mine), dtype=bool)]
        pred, _ = joint.fit_predict("baseline", X, y, train, ~train, joint.SEED)
        ids, _ = look1e.questions(see_dir, fe)
        out[fe] = {"panels": panels, "mine": mine, "scores": pred.tolist(),
                   "ids": [ids.get(p["title"], ("?", p["title"])) for p in mine]}
    return out, dg


def _num(v) -> float | None:
    try:
        x = float(v)
        return x if math.isfinite(x) else None
    except (TypeError, ValueError):
        return None


def _esc(s) -> str:
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def _fmt(x: float) -> str:
    a = abs(x)
    if a >= 1e9:
        return f"{x / 1e9:.1f}B"
    if a >= 1e6:
        return f"{x / 1e6:.1f}M"
    if a >= 1e4:
        return f"{x / 1e3:.0f}k"
    return f"{x:.0f}" if a >= 100 or x == int(x) else f"{x:.2f}"


def chart(p: dict) -> str:
    """A panel drawn as its shape asks: a series as a 2px line, a breakdown
    as horizontal bars, a distribution as columns, one value as a figure;
    one series, so one colour and no legend; every mark carries a tooltip."""
    rows = p.get("rows") or []
    if not rows:
        return "<div class='more'>no rows</div>"
    if len(rows) == 1 and (len(rows[0]) == 1 or p.get("bucket") is None and len(rows[0]) == 1):
        v = _num(rows[0][-1])
        return f"<div class='hero'>{_esc(_fmt(v) if v is not None else rows[0][-1])}</div>"
    labels = [("∅" if r[0] is None else str(r[0])) if len(r) > 1 else "" for r in rows]
    vals = [_num(r[-1]) for r in rows]
    pts = [(lab, v) for lab, v in zip(labels, vals) if v is not None]
    if not pts:
        return "<div class='more'>no numbers to draw</div>"
    W, H = 320, 132
    if p.get("bucket") is not None:
        pts.sort(key=lambda t: t[0])
        lo, hi = min(0.0, min(v for _, v in pts)), max(v for _, v in pts) or 1.0
        L, R, T, B = 36, 8, 8, 18
        xs = [L + (W - L - R) * (i / max(1, len(pts) - 1)) for i in range(len(pts))]
        ys = [T + (H - T - B) * (1 - (v - lo) / ((hi - lo) or 1)) for _, v in pts]
        d = " ".join(f"{'M' if i == 0 else 'L'}{x:.1f},{y:.1f}" for i, (x, y) in enumerate(zip(xs, ys)))
        hit = "".join(f"<circle cx='{x:.1f}' cy='{y:.1f}' r='7' fill='transparent'><title>{_esc(lab)}: {_esc(_fmt(v))}</title></circle>"
                      for (lab, v), x, y in zip(pts, xs, ys))
        base = T + (H - T - B) * (1 - (0 - lo) / ((hi - lo) or 1))
        return (f"<svg viewBox='0 0 {W} {H}' role='img' aria-label='{_esc(p['title'])}'>"
                f"<line x1='{L}' x2='{W - R}' y1='{base:.1f}' y2='{base:.1f}' class='axis'/>"
                f"<text x='{L - 4}' y='{T + 8}' class='tick' text-anchor='end'>{_esc(_fmt(hi))}</text>"
                f"<text x='{L - 4}' y='{base:.1f}' class='tick' text-anchor='end'>{_esc(_fmt(lo))}</text>"
                f"<path d='{d}' class='line'/>"
                f"<circle cx='{xs[-1]:.1f}' cy='{ys[-1]:.1f}' r='4' class='end'/>"
                f"<text x='{L}' y='{H - 4}' class='tick'>{_esc(pts[0][0][:10])}</text>"
                f"<text x='{W - R}' y='{H - 4}' class='tick' text-anchor='end'>{_esc(pts[-1][0][:10])}</text>{hit}</svg>")
    if p.get("template") == "T5":
        n = len(pts)
        hi = max(v for _, v in pts) or 1.0
        L, B, T = 4, 18, 8
        band = (W - 2 * L) / n
        bw = min(24.0, band - 2)
        bars = []
        for i, (lab, v) in enumerate(pts):
            h = max(0.0, (H - T - B) * v / hi)
            x, y = L + i * band + (band - bw) / 2, H - B - h
            r = min(4.0, h, bw / 2)
            bars.append(f"<path d='M{x:.1f},{H - B} V{y + r:.1f} Q{x:.1f},{y:.1f} {x + r:.1f},{y:.1f} H{x + bw - r:.1f} "
                        f"Q{x + bw:.1f},{y:.1f} {x + bw:.1f},{y + r:.1f} V{H - B} Z' class='bar'><title>{_esc(lab)}: {_esc(_fmt(v))}</title></path>")
        return (f"<svg viewBox='0 0 {W} {H}' role='img' aria-label='{_esc(p['title'])}'>{''.join(bars)}"
                f"<line x1='{L}' x2='{W - L}' y1='{H - B}' y2='{H - B}' class='axis'/>"
                f"<text x='{L}' y='{H - 4}' class='tick'>{_esc(pts[0][0][:14])}</text>"
                f"<text x='{W - L}' y='{H - 4}' class='tick' text-anchor='end'>{_esc(pts[-1][0][:14])}</text></svg>")
    pts = pts[:12]
    hi = max(max(v for _, v in pts), 0) or 1.0
    L, R, row = 112, 44, 18
    h = row * len(pts) + 4
    bars = []
    for i, (lab, v) in enumerate(pts):
        y = 2 + i * row
        w = max(0.0, (W - L - R) * max(v, 0) / hi)
        bh = row - 4
        r = min(4.0, w, bh / 2)
        x0, x1 = L, L + w
        shape = (f"<path d='M{x0},{y} H{x1 - r:.1f} Q{x1:.1f},{y} {x1:.1f},{y + r:.1f} V{y + bh - r:.1f} "
                 f"Q{x1:.1f},{y + bh} {x1 - r:.1f},{y + bh} H{x0} Z' class='bar'><title>{_esc(lab)}: {_esc(_fmt(v))}</title></path>"
                 if w > 0 else "")
        bars.append(f"<text x='{L - 6}' y='{y + bh - 3}' class='lab' text-anchor='end'>{_esc(lab[:18])}</text>{shape}"
                    f"<text x='{x1 + 4:.1f}' y='{y + bh - 3}' class='tick'>{_esc(_fmt(v))}</text>")
    return (f"<svg viewBox='0 0 {W} {h}' role='img' aria-label='{_esc(p['title'])}'>"
            f"<line x1='{L}' x2='{L}' y1='0' y2='{h}' class='axis'/>{''.join(bars)}</svg>")


def page(see_dir: Path, fixtures: list[str]) -> None:
    sets = local.module("live_sets", ROOT / "round-1g" / "sets.py")
    scored, dg = look_scores(see_dir, fixtures)
    rng = random.Random(SEED)
    key, sections = {}, []
    for fe in fixtures:
        s = scored[fe]
        chooser = dg.look1e.Chooser(s["ids"])
        fives = {"the look, each axis once": [s["mine"][i] for i in chooser.choose(s["scores"], sets.AXIS_ONCE)],
                 "round 1c's ranker": [p for p in s["mine"] if "round 1c" in p["in"]][:5],
                 "round 1's rules": [p for p in s["panels"] if "round 1" in p["in"]][:5]}
        order = list(fives)
        rng.shuffle(order)
        key[fe] = {}
        blocks = []
        for letter, system in zip("XYZ", order):
            key[fe][letter] = system
            panels = rng.sample(fives[system], len(fives[system]))
            cards = []
            for k, p in enumerate(panels, 1):
                rows = p.get("rows") or []
                body = "".join("<tr>" + "".join(f"<td>{'∅' if v is None else str(v)[:40]}</td>" for v in r) + "</tr>"
                               for r in rows[:12])
                more = f"<div class='more'>… {len(rows) - 12} more rows</div>" if len(rows) > 12 else ""
                err = f"<div class='err'>not run: {str(p['error'])[:100]}</div>" if p.get("error") else ""
                pid = f"{fe}/{letter}{k}"
                cards.append(f"<div class='card'><div class='t'>{letter}{k}. {_esc(p['title'])}</div>{chart(p)}"
                             f"<details><summary>rows</summary><table>{body}</table>{more}</details>{err}"
                             f"<label><input type='checkbox' data-k='{pid}/worth'> worth a place</label> "
                             f"<label><input type='checkbox' data-k='{pid}/repeats'> repeats another</label></div>")
            blocks.append(f"<div class='five'><h3>Five {letter}</h3>{''.join(cards)}"
                          f"<label class='best'><input type='radio' name='{fe}-best' data-k='{fe}/best' value='{letter}'> "
                          f"the five I would keep</label></div>")
        sections.append(f"<section><h2>{fe}</h2><div class='row'>{''.join(blocks)}</div></section>")
    html = f"""<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Your databases' fives</title><style>
:root{{--bg:#fff;--fg:#0b0b0b;--mut:#52514e;--line:#e5e5ea;--card:#fcfcfb;--s1:#2a78d6}}
@media (prefers-color-scheme: dark){{:root{{--bg:#111;--fg:#f0efec;--mut:#c3c2b7;--line:#333;--card:#1a1a19;--s1:#3987e5}}}}
svg{{width:100%;height:auto;display:block;margin:4px 0}} .line{{fill:none;stroke:var(--s1);stroke-width:2;stroke-linejoin:round;stroke-linecap:round}}
.bar{{fill:var(--s1)}} .bar:hover{{opacity:.8}} .end{{fill:var(--s1);stroke:var(--card);stroke-width:2}}
.axis{{stroke:var(--line);stroke-width:1}} .tick{{fill:var(--mut);font-size:10px}} .lab{{fill:var(--fg);font-size:10px}}
.hero{{font-size:48px;font-weight:600;margin:8px 0}} summary{{font-size:12px;color:var(--mut);cursor:pointer}}
body{{background:var(--bg);color:var(--fg);font:14px/1.4 system-ui,sans-serif;margin:16px}}
.row{{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}}
.five{{border:1px solid var(--line);border-radius:8px;padding:8px}}
.card{{background:var(--card);border-radius:6px;padding:8px;margin:8px 0}}
.t{{font-weight:600;margin-bottom:4px}} table{{border-collapse:collapse;font-size:12px}}
td{{border-bottom:1px solid var(--line);padding:2px 8px 2px 0}} .more,.err{{color:var(--mut);font-size:12px}}
label{{font-size:12px;color:var(--mut);margin-right:8px}} .best{{display:block;margin-top:8px;color:var(--fg)}}
button{{margin:16px 0;padding:8px 12px}} textarea{{width:100%;height:120px}}</style></head><body>
<h1>Your databases' fives</h1>
<p>Three fives for each database, shuffled, unnamed. Mark each panel worth a place on a first screen, each that
repeats another of its five, and the five you would keep. Then copy your answers and send them back.</p>
{''.join(sections)}
<button onclick="out.value=JSON.stringify(Object.fromEntries([...document.querySelectorAll('input:checked')].map(e=>[e.dataset.k+(e.type==='radio'?'':''), e.type==='radio'?e.value:true])),null,1);out.select();try{{document.execCommand('copy')}}catch(e){{}}">Copy my answers</button>
<textarea id="out"></textarea></body></html>"""
    out = see_dir / "look"
    out.mkdir(parents=True, exist_ok=True)
    (out / "judge.html").write_text(html, encoding="utf-8")
    (out / "judge-key.json").write_text(json.dumps(key, indent=1), encoding="utf-8")
    print(f"{out / 'judge.html'}: {sum(len(v) for v in key.values())} fives over {len(key)} databases")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *args = sys.argv[1:]
    if cmd == "pool":
        pool(Path(args[0]), args[1], args[2], args[3])
    elif cmd == "page":
        page(Path(args[0]), args[1:])
