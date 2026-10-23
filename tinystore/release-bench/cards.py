#!/usr/bin/env python3
"""Draws the README's cards from a results directory's summary.json, in the
form the landing page reads back (web/src/lib/content/landing.ts): a title at
16, its unit at 12 on y=56, then each row's label at x=20 and its bars.

    python3 cards.py results/<name> <tinystore>/.github/assets

Each card is one bordered SVG a GitHub theme, 400 wide, so that two sit on a
line of a README and a phone wraps them one to a line. Every figure is read
from summary.json, the median of its passes, and never typed in.
"""
import json
import sys
from pathlib import Path
from xml.sax.saxutils import escape

MIB = 1 << 20

THEMES = {
    "light": {"text": "#1f2328", "muted": "#59636e", "ours": "#23212B", "theirs": "#d1d9e0", "border": "#d1d9e0"},
    "dark": {"text": "#f0f6fc", "muted": "#9198a1", "ours": "#ECEAF3", "theirs": "#3d444d", "border": "#3d444d"},
}

WIDTH = 400
PAD = 20
LABEL = 132  # where the bars start, from the padding
BAR_END = 300  # the longest bar's end, leaving room for its number
ROW, BAR = 32, 20
FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"


def card(title, unit, rows, colours, height):
    """rows: (label, value, shown, ours)"""
    body = [
        f'<rect x="0.5" y="0.5" width="{WIDTH - 1}" height="{height - 1}" rx="6" fill="none" '
        f'stroke="{colours["border"]}"/>',
        f'<text x="{PAD}" y="{PAD + 16}" font-size="16" font-weight="600" fill="{colours["text"]}">'
        f"{escape(title)}</text>",
        f'<text x="{PAD}" y="{PAD + 36}" font-size="12" fill="{colours["muted"]}">{escape(unit)}</text>',
    ]
    y = PAD + 54
    most = max(value for _, value, _, _ in rows)
    left = PAD + LABEL
    for name, value, shown, ours in rows:
        width = max(3.0, (BAR_END - left) * value / most)
        fill, weight = (colours["ours"], "600") if ours else (colours["theirs"], "400")
        middle = y + BAR / 2 + 5
        body += [
            f'<text x="{PAD}" y="{middle}" font-size="14" font-weight="{weight}" fill="{colours["text"]}">'
            f"{escape(name)}</text>",
            f'<rect x="{left}" y="{y}" width="{width:.1f}" height="{BAR}" rx="3" fill="{fill}"/>',
            f'<text x="{left + width + 8:.1f}" y="{middle}" font-size="14" font-weight="{weight}" '
            f'fill="{colours["text"]}">{escape(shown)}</text>',
        ]
        y += ROW
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" '
            f'viewBox="0 0 {WIDTH} {height}" font-family="{FONT}">\n' + "\n".join(body) + "\n</svg>\n")


def rate(v):
    if v >= 1_000_000:
        return f"{v / 1e6:.2f} M"
    if v >= 10_000:
        return f"{v / 1e3:.0f} k"
    if v >= 1_000:
        return f"{v / 1e3:.1f} k"
    return f"{v:.0f}"


def size(v):
    return f"{v:.1f}" if v < 100 else f"{v:.0f}"


def seconds(v):
    return f"{v * 1000:.0f} ms" if v < 1 else f"{v:.1f} s"


def cards(summary):
    """name -> (title, unit, rows): the cards the landing page shows, in order."""
    idle = summary["idle"]["contenders"]
    app = summary["app"]["contenders"]
    kv = summary["kv"]["contenders"]
    metrics = summary["metrics"]["cases"]
    logs = summary["logs"]["cases"]
    out = {}

    served = idle["tinystore-serve"]
    services = idle["services"]
    out["memory"] = ("Memory at rest", "MiB, every engine open · lower is better", [
        ("TinyStore", served["groups"]["tinystore"] / MIB, size(served["groups"]["tinystore"] / MIB), True),
        ("The five services", services["pss_services"] / MIB, size(services["pss_services"] / MIB), False),
    ])

    tiny_ready = served["ready"]["tinystore"] + served["open_ms"] / 1000
    stack_ready = max(services["ready"].values())
    out["ready"] = ("Ready to serve", "from start to every engine open · lower is better", [
        ("TinyStore", tiny_ready, seconds(tiny_ready), True),
        ("The five services", stack_ready, seconds(stack_ready), False),
    ])

    def requests(contender, field="perSecond"):
        return app[contender]["stages"]["request@64"][field]
    out["requests"] = ("An application, 64 clients", "requests a second · higher is better", [
        ("TinyStore", requests("tinystore-sidecar"), rate(requests("tinystore-sidecar")), True),
        ("In process", requests("tinystore-embedded"), rate(requests("tinystore-embedded")), True),
        ("The five services", requests("services"), rate(requests("services")), False),
    ])
    def alone(contender):
        return app[contender]["stages"]["request@1"]["perSecond"]
    out["requests-one"] = ("An application, one client", "requests a second · higher is better", [
        ("TinyStore", alone("tinystore-sidecar"), rate(alone("tinystore-sidecar")), True),
        ("In process", alone("tinystore-embedded"), rate(alone("tinystore-embedded")), True),
        ("The five services", alone("services"), rate(alone("services")), False),
    ])

    def memory(contender):
        return app[contender]["memory"]["request@64"]["total_max"] / MIB
    out["load-memory"] = ("An application, memory", "MiB at 64 clients, program included · lower is better", [
        ("TinyStore", memory("tinystore-sidecar"), size(memory("tinystore-sidecar")), True),
        ("In process", memory("tinystore-embedded"), size(memory("tinystore-embedded")), True),
        ("The five services", memory("services"), size(memory("services")), False),
    ])

    read = {name: kv[name]["stages"]["get@1"]["p50us"] for name in
            ("tinystore-embedded-synced", "tinystore-sidecar-synced", "redis-synced", "valkey-synced")}
    out["kv-read"] = ("A key read, one at a time", "µs, median · lower is better", [
        ("In process", read["tinystore-embedded-synced"], f"{read['tinystore-embedded-synced']:.0f}", True),
        ("TinyStore", read["tinystore-sidecar-synced"], f"{read['tinystore-sidecar-synced']:.0f}", True),
        ("Redis", read["redis-synced"], f"{read['redis-synced']:.0f}", False),
        ("Valkey", read["valkey-synced"], f"{read['valkey-synced']:.0f}", False),
    ])

    corpus = "long" if "tinystore-long" in metrics else "wide"
    per_sample = {}
    for contender in ("tinystore", "victoria", "prometheus"):
        case = metrics[f"{contender}-{corpus}"]
        kept = {"tinystore": case["disk_closed"], "victoria": case["disk_closed"],
                "prometheus": case["disk_closed"]}[contender]
        per_sample[contender] = kept / case["samples"]
    out["metrics-disk"] = ("Metrics on disk", "bytes a TSBS sample · lower is better", [
        ("TinyStore", per_sample["tinystore"], f"{per_sample['tinystore']:.2f}", True),
        ("VictoriaMetrics", per_sample["victoria"], f"{per_sample['victoria']:.2f}", False),
        ("Prometheus", per_sample["prometheus"], f"{per_sample['prometheus']:.2f}", False),
    ])

    raw = sum(case["bytes"] for case in logs.values() if case["contender"] == "tinystore")
    tiny = sum(case["disk_closed_apparent"] for case in logs.values() if case["contender"] == "tinystore")
    loki = sum(case["chunks"] + case["index"] for case in logs.values() if case["contender"] == "loki")
    # the whole text under zstd -19: no store, nothing found in it without reading it all
    packed = sum(case["zstd19"] for case in logs.values() if case["contender"] == "tinystore")
    out["logs-disk"] = ("Logs on disk", "% of LogHub's text · lower is better", [
        ("TinyStore", 100 * tiny / raw, f"{100 * tiny / raw:.1f} %", True),
        ("Loki", 100 * loki / raw, f"{100 * loki / raw:.1f} %", False),
        ("zstd -19, a file", 100 * packed / raw, f"{100 * packed / raw:.1f} %", False),
    ])
    return out


def main():
    summary = json.loads((Path(sys.argv[1]) / "summary.json").read_text())
    out = Path(sys.argv[2])
    drawn = cards(summary)
    height = PAD + 54 + ROW * max(len(rows) for _, _, rows in drawn.values()) + PAD - (ROW - BAR)
    for name, (title, unit, rows) in drawn.items():
        print(name, ", ".join(f"{label} {shown}" for label, _, shown, _ in rows), file=sys.stderr)
        for theme, colours in THEMES.items():
            path = out / f"bench-{name}-{theme}.svg"
            path.write_text(card(title, unit, rows, colours, height), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
