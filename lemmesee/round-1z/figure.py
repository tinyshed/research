"""Round 1z: F1 on plain A against the bytes of a model.

    python -I figure.py <scores.json> <sizes.json> <out folder>

<scores.json> is score.py's; <sizes.json> gives each model's bytes
({"name": bytes}, written by report.py from the fits and the runtime's
listing). Trees, networks and local language models stand at their bytes on
a logarithmic axis; the remote language models' weights are not published,
so they stand in a band at the right edge, off the axis. A model's bar is
its 95 % interval. A filled mark is the protocol's reading of a model's
replies; where the reading that forgives two faults of form (score.py's
`form mended`) differs, it is drawn hollow above, joined by a dotted line.
Writes size-f1.pdf and size-f1.png.
"""

import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

INK, MUTED, GRID = "#1f1f1e", "#6b6a64", "#e4e3de"
KINDS = {  # colour and marker a kind: shape carries the kind where colour does not
    "tree": ("#2a78d6", "o", "trees (lemmekey)"),
    "network": ("#eb6834", "s", "neural pair scorers"),
    "local": ("#1baf7a", "^", "local language models"),
    "remote": ("#eda100", "D", "remote language models"),
}


def human(b: float) -> str:
    for unit, k in (("GB", 1e9), ("MB", 1e6), ("kB", 1e3)):
        if b >= k:
            return f"{b / k:.3g} {unit}"
    return f"{b:.0f} B"


def draw(scores: dict, sizes: dict, labels: dict, out: Path) -> None:
    models = scores["plain-a"]["models"]
    fig, (ax, side) = plt.subplots(1, 2, figsize=(9.2, 5.2), sharey=True, gridspec_kw={"width_ratios": [5, 1.25], "wspace": 0.04})
    seen = set()
    remote = sorted(((m["f1"], n, m) for n, m in models.items() if m.get("kind") == "remote"), reverse=True)
    for name, m in models.items():
        kind = "tree" if name == "lemmekey" else m.get("kind")
        if kind in (None, "remote") or name not in sizes or name == "lemmekey, scored again":
            continue
        colour, marker, legend = KINDS[kind]
        lo, hi = m["f1_95"]
        ax.plot([sizes[name]] * 2, [lo, hi], color=colour, lw=1.2, alpha=0.6, zorder=2)
        ax.plot(sizes[name], m["f1"], marker, ms=8.5, color=colour, mew=1.6,
                label=legend if kind not in seen else None, zorder=3)
        fm = m.get("form mended")
        if fm and abs(fm["f1"] - m["f1"]) > 0.005:
            ax.plot([sizes[name]] * 2, [m["f1"], fm["f1"]], color=colour, lw=1, ls=":", zorder=2)
            ax.plot(sizes[name], fm["f1"], marker, ms=8.5, color=colour, mfc="white", mew=1.6, zorder=3)
            if name in labels and "mended" in labels[name]:
                ax.annotate(labels[name]["mended"], (sizes[name], fm["f1"]), xytext=(8, 0), textcoords="offset points",
                            va="center", fontsize=8, color=MUTED)
        seen.add(kind)
        dx, dy, ha = labels.get(name, {}).get("offset", (8, 0, "left"))
        ax.annotate(labels.get(name, {}).get("text", name), (sizes[name], m["f1"]), xytext=(dx, dy),
                    textcoords="offset points", ha=ha, va="center", fontsize=8.5, color=INK)
    colour, marker, legend = KINDS["remote"]
    for k, (f, name, m) in enumerate(remote):
        x = 0.25
        side.plot([x, x], m["f1_95"], color=colour, lw=1.2, alpha=0.6, zorder=2)
        side.plot(x, f, marker, ms=7.5, color=colour, label=legend if k == 0 else None, zorder=3)
        fm = m.get("form mended")
        if fm and abs(fm["f1"] - f) > 0.005:
            side.plot([x - 0.12, x - 0.12], [f, fm["f1"]], color=colour, lw=1, ls=":", zorder=2)
            side.plot(x - 0.12, fm["f1"], marker, ms=7.5, color=colour, mfc="white", mew=1.4, zorder=3)
        side.annotate(labels.get(name, {}).get("text", name), (x, f), xytext=(8, labels.get(name, {}).get("dy", 0)),
                      textcoords="offset points", va="center", fontsize=8, color=INK)
    ours = models["lemmekey"]["f1"]
    for a in (ax, side):
        a.axhline(ours, color=KINDS["tree"][0], lw=0.8, ls=(0, (4, 3)), zorder=1)
        a.grid(axis="y", color=GRID, lw=0.8)
        a.set_axisbelow(True)
        for s in ("top", "right"):
            a.spines[s].set_visible(False)
        a.tick_params(colors=MUTED, labelsize=9)
        for s in a.spines.values():
            s.set_color(GRID)
    ax.set_xscale("log")
    ax.set_xlim(4e5, 3e10)
    ax.set_ylim(-0.02, 0.93)
    ticks = [1e6, 1e7, 1e8, 1e9, 1e10]
    ax.set_xticks(ticks, [human(t) for t in ticks])
    ax.set_xlabel("size of the model on disk (logarithmic)", color=INK, fontsize=10)
    ax.set_ylabel("F1 over every declared key, plain A", color=INK, fontsize=10)
    side.set_xlim(0, 1.6)
    side.set_xticks([0.25], ["weights not\npublished"])
    side.spines["left"].set_visible(False)
    side.tick_params(left=False)
    h1, l1 = ax.get_legend_handles_labels()
    h2, l2 = side.get_legend_handles_labels()
    ax.legend(h1 + h2, l1 + l2, loc="center left", bbox_to_anchor=(0.05, 0.28), frameon=False, fontsize=9, labelcolor=INK)
    ax.set_title("Filled: the protocol's reading of the replies. Hollow: two faults of form forgiven. Bar: 95 % interval",
                 loc="left", fontsize=8.5, color=MUTED)
    out.mkdir(parents=True, exist_ok=True)
    fig.savefig(out / "size-f1.pdf", bbox_inches="tight")
    fig.savefig(out / "size-f1.png", dpi=200, bbox_inches="tight")


if __name__ == "__main__":
    scores = json.load(open(sys.argv[1], encoding="utf-8"))
    cfg = json.load(open(sys.argv[2], encoding="utf-8"))
    draw(scores, cfg["bytes"], cfg.get("labels", {}), Path(sys.argv[3]))
