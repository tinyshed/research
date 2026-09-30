"""Draws the README's comparison cards from a results directory's rounds.

    python cards.py results/<date> <tinystore>/.github/assets

Each card is one bordered SVG a GitHub theme, 400 wide, so that two sit on a
line of a README and a phone wraps them one to a line. Every card has room for
as many bars as the fullest one, so that a row of two lines up. A figure is the
median of a contender's repeats, read from the round's JSON and never typed in.
"""

import json
import pathlib
import statistics
import sys
from xml.sax.saxutils import escape


def load(directory, engine):
    path = directory / f"{engine}.json"
    return json.loads(path.read_text()) if path.exists() else None


def per_second(round_, contender, stage, goroutines):
    values = [
        s["per_second"]
        for run in round_["runs"] if run["contender"] == contender
        for s in run["stages"] if s["name"] == stage and s["goroutines"] == goroutines and not s.get("errors")
    ]
    return statistics.median(values) if values else None


def peak(round_, contender):
    values = [
        run["peak_rss_bytes"] + max(run.get("service_pss_bytes", 0), run.get("service_peak_rss_bytes", 0))
        for run in round_["runs"] if run["contender"] == contender
    ]
    return statistics.median(values) / 2**20 if values else None


def disk(round_, contender):
    values = [run["disk_bytes"] for run in round_["runs"] if run["contender"] == contender]
    return statistics.median(values) / 2**20 if values else None


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


# name, title, unit, engine, measure, format, [(label, contender)]; TinyStore first.
# measure takes the round and a contender and returns the figure.
CARDS = [
    ("stack", "An application, 64 clients", "requests a second · higher is better", "stack",
     lambda r, c: per_second(r, c, "request", 64), rate,
     [("TinyStore", "tinystore"), ("sidecar", "tinystore-sidecar"), ("services", "services")]),
    ("stack-memory", "An application, memory", "MiB at peak, services included · lower is better", "stack",
     peak, size,
     [("TinyStore", "tinystore"), ("sidecar", "tinystore-sidecar"), ("services", "services")]),
    ("kv", "KV writes, 64 goroutines", "durable sets a second · higher is better", "kv",
     lambda r, c: per_second(r, c, "set", 64), rate,
     [("TinyStore", "tinystore"), ("Pebble", "pebble"), ("Redis", "redis"), ("bbolt", "bbolt"), ("SQLite", "sqlite")]),
    ("sqldb", "SQL inserts, 64 goroutines", "durable inserts a second · higher is better", "sqldb",
     lambda r, c: per_second(r, c, "insert", 64), rate,
     [("TinyStore", "tinystore"), ("Postgres", "postgres"), ("SQLite", "sqlite")]),
    ("records", "Log lines on disk", "MiB after close · lower is better", "records",
     disk, size,
     [("TinyStore", "tinystore"), ("JSONL+zstd", "jsonl-zstd"), ("JSONL", "jsonl")]),
    ("metrics-memory", "Metrics, memory", "MiB at peak · lower is better", "metrics",
     peak, size,
     [("TinyStore", "tinystore"), ("Victoria", "victoria"), ("Prometheus", "prometheus")]),
]

THEMES = {
    "light": {"text": "#1f2328", "muted": "#59636e", "ours": "#23212B", "theirs": "#d1d9e0", "border": "#d1d9e0"},
    "dark": {"text": "#f0f6fc", "muted": "#9198a1", "ours": "#ECEAF3", "theirs": "#3d444d", "border": "#3d444d"},
}

WIDTH = 400
PAD = 20
LABEL = 118  # where the bars start, from the padding
BAR_END = 300  # the longest bar's end, leaving room for its number
ROW, BAR = 32, 20
FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"


def card(title, unit, rows, colours, height):
    body = [
        f'<rect x="0.5" y="0.5" width="{WIDTH - 1}" height="{height - 1}" rx="6" fill="none" '
        f'stroke="{colours["border"]}"/>',
        f'<text x="{PAD}" y="{PAD + 16}" font-size="16" font-weight="600" fill="{colours["text"]}">'
        f"{escape(title)}</text>",
        f'<text x="{PAD}" y="{PAD + 36}" font-size="12" fill="{colours["muted"]}">{escape(unit)}</text>',
    ]
    y = PAD + 54
    most = max(value for _, value, _ in rows)
    left = PAD + LABEL
    for i, (name, value, shown) in enumerate(rows):
        width = max(3.0, (BAR_END - left) * value / most)
        fill, weight = (colours["ours"], "600") if i == 0 else (colours["theirs"], "400")
        middle = y + BAR / 2 + 5
        body += [
            f'<text x="{PAD}" y="{middle}" font-size="14" font-weight="{weight}" fill="{colours["text"]}">'
            f"{escape(name)}</text>",
            f'<rect x="{left}" y="{y}" width="{width:.1f}" height="{BAR}" rx="3" fill="{fill}"/>',
            f'<text x="{left + width + 8:.1f}" y="{middle}" font-size="14" font-weight="{weight}" '
            f'fill="{colours["text"]}">{shown}</text>',
        ]
        y += ROW
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" '
        f'viewBox="0 0 {WIDTH} {height}" font-family="{FONT}">\n' + "\n".join(body) + "\n</svg>\n"
    )


def main():
    directory, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    drawn = []
    for name, title, unit, engine, measure, fmt, contenders in CARDS:
        round_ = load(directory, engine)
        if round_ is None:
            print(f"{name}: no {engine}.json, skipped", file=sys.stderr)
            continue
        rows = []
        for label, contender in contenders:
            value = measure(round_, contender)
            if value is None:
                print(f"{name}: no {contender} in {engine}.json", file=sys.stderr)
                continue
            rows.append((label, value, fmt(value)))
        if rows:
            drawn.append((name, title, unit, rows))
    height = PAD + 54 + ROW * max(len(rows) for *_, rows in drawn) + PAD - (ROW - BAR)
    for name, title, unit, rows in drawn:
        print(name, ", ".join(f"{label} {shown}" for label, _, shown in rows), file=sys.stderr)
        for theme, colours in THEMES.items():
            path = out / f"bench-{name}-{theme}.svg"
            path.write_text(card(title, unit, rows, colours, height), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
