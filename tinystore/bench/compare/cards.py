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
    if not path.exists():
        return None

    round_ = json.loads(path.read_text())
    if round_.get("failed"):
        raise ValueError(f"{engine}: failed runs cannot become README figures")

    for run in round_["runs"]:
        for stage in run["stages"]:
            if stage.get("errors"):
                raise ValueError(f'{engine}: {run["contender"]} {stage["name"]} has errors')

    if engine == "metrics":
        raw = directory / "metrics-raw.json"
        if raw.exists():
            round_["raw"] = json.loads(raw.read_text())

    return round_


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


def metric_disk(round_, contender):
    if contender == "raw":
        return round_["raw"]["TotalBytes"] / 2**20 if "raw" in round_ else None
    return disk(round_, contender)


def memory_pair(round_, contender):
    runs = [run for run in round_["runs"] if run["contender"] == contender]
    if not runs:
        return None
    for run in runs:
        if run.get("processes", 1) > 1 and "open_service_pss_bytes" not in run:
            raise ValueError("service idle was not measured in this round")
    idle = [run["open_rss_bytes"] + max(run.get("open_service_pss_bytes", 0),
                                      run.get("open_service_peak_rss_bytes", 0)) for run in runs]
    return statistics.median(idle) / 2**20, peak(round_, contender)


def settled_ingest(round_, contender):
    values = []
    for run in round_["runs"]:
        if run["contender"] != contender:
            continue
        stages = {stage["name"]: stage for stage in run["stages"]}
        values.append(stages["ingest"]["ops"] / (stages["ingest"]["seconds"] + stages["settle"]["seconds"]))
    return statistics.median(values) if values else None


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
     [("TinyStore Batch", "tinystore-batch"), ("TinyStore Split", "tinystore"), ("Services", "services")]),
    ("stack-memory", "An application, memory", "Idle / load peak, MiB (client + services)", "stack-memory",
     memory_pair, lambda pair: f"{size(pair[0])} / {size(pair[1])}",
     [("TinyStore", "tinystore-batch"), ("Services", "services")]),
    ("kv", "KV writes, 64 goroutines", "durable sets a second · higher is better", "kv",
     lambda r, c: per_second(r, c, "set", 64), rate,
     [("TinyStore", "tinystore"), ("Pebble", "pebble"), ("Redis", "redis"), ("bbolt", "bbolt"), ("SQLite", "sqlite")]),
    ("sqldb", "SQL inserts, 64 goroutines", "durable inserts a second · higher is better", "sqldb",
     lambda r, c: per_second(r, c, "insert", 64), rate,
     [("TinyStore", "tinystore"), ("Postgres", "postgres"), ("SQLite", "sqlite")]),
    ("records", "Log lines on disk", "MiB after close · lower is better", "records",
     disk, size,
     [("TinyStore", "tinystore"), ("JSONL+zstd", "jsonl-zstd"), ("JSONL", "jsonl")]),
    ("metrics-disk", "Metrics on disk", "MiB after settle and close · lower is better", "metrics",
     metric_disk, size,
     [("TinyStore", "tinystore"), ("Victoria", "victoria"), ("Prometheus", "prometheus"), ("Raw binary", "raw")]),
    ("metrics-memory", "Metrics, memory", "Client HWM + service max(HWM, PSS), MiB", "metrics",
     peak, size,
     [("TinyStore", "tinystore"), ("Victoria", "victoria"), ("Prometheus", "prometheus")]),
    ("metrics-ingest", "Metrics ingest + settle", "samples a second · higher is better", "metrics",
     settled_ingest, rate,
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


def memory_card(title, unit, rows, colours, height):
    body = [
        f'<rect x="0.5" y="0.5" width="{WIDTH - 1}" height="{height - 1}" rx="6" fill="none" '
        f'stroke="{colours["border"]}"/>',
        f'<text x="{PAD}" y="36" font-size="16" font-weight="600" fill="{colours["text"]}">{escape(title)}</text>',
        f'<text x="{PAD}" y="56" font-size="12" fill="{colours["muted"]}">{escape(unit)}</text>',
    ]
    left, end = PAD + LABEL, 300
    maximum = max(peak for _, (_, peak), _ in rows)
    for i, (label, (idle, peak), _) in enumerate(rows):
        y = 74 + i * 48
        colour = colours["ours"] if i == 0 else colours["theirs"]
        body.append(f'<text x="{PAD}" y="{y + 13}" font-size="14" fill="{colours["text"]}">{escape(label)}</text>')
        for value, offset, opacity in ((idle, 0, 1), (peak, 20, 0.4)):
            width = max(3, (end - left) * value / maximum)
            body += [
                f'<rect x="{left}" y="{y + offset}" width="{width:.1f}" height="12" rx="2" '
                f'fill="{colour}" opacity="{opacity}"/>',
                f'<text x="{left + width + 8:.1f}" y="{y + offset + 11}" font-size="12" '
                f'fill="{colours["text"]}">{size(value)}</text>',
            ]
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" '
            f'viewBox="0 0 {WIDTH} {height}" font-family="{FONT}">\n' + "\n".join(body) + "\n</svg>\n")


def client_modes_card(round_, colours, mobile):
    width, height = (400, 550) if mobile else (800, 310)
    body = [
        f'<rect x="0.5" y="0.5" width="{width - 1}" height="{height - 1}" rx="6" fill="none" '
        f'stroke="{colours["border"]}"/>',
        f'<text x="20" y="36" font-size="16" font-weight="600" fill="{colours["text"]}">'
        'KV: embedded, sidecar, server</text>',
        f'<text x="20" y="56" font-size="12" fill="{colours["muted"]}">'
        '64 calls in flight · calls/second · higher is better</text>',
    ]
    modes = [("Go embedded", "go-embedded"), ("Go sidecar", "go-sidecar"),
             ("Bun sidecar", "bun-tinystore"), ("Python sidecar", "python-tinystore"),
             ("Go server", "go-server"), ("Bun server", "bun-tinystore-server"),
             ("Python server", "python-tinystore-server")]
    for panel, (operation, title) in enumerate((("get", "Reads"), ("set", "Writes"))):
        x, y = (0, 80 + panel * 240) if mobile else (panel * 400, 80)
        body.append(f'<text x="{x + 20}" y="{y}" font-size="14" font-weight="600" '
                    f'fill="{colours["text"]}">{title}</text>')
        values = [per_second(round_, contender, operation, 64) for _, contender in modes]
        if any(value is None for value in values):
            raise ValueError("client modes card needs every measured mode")
        maximum = max(values)
        for row, ((label, _), value) in enumerate(zip(modes, values)):
            top = y + 16 + row * 27
            length = max(3, 162 * value / maximum)
            fill = colours["ours"] if row == 0 else colours["theirs"]
            body += [
                f'<text x="{x + 20}" y="{top + 13}" font-size="13" fill="{colours["text"]}">{label}</text>',
                f'<rect x="{x + 138}" y="{top}" width="{length:.1f}" height="17" rx="3" fill="{fill}"/>',
                f'<text x="{x + 146 + length:.1f}" y="{top + 13}" font-size="13" '
                f'fill="{colours["text"]}">{rate(value)}</text>',
            ]
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
            f'viewBox="0 0 {width} {height}" font-family="{FONT}">\n' + "\n".join(body) + "\n</svg>\n")


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
            draw = memory_card if name == "stack-memory" else card
            path.write_text(draw(title, unit, rows, colours, height), encoding="utf-8", newline="\n")
    modes = load(directory, "sdk-modes")
    if modes is not None:
        for theme, colours in THEMES.items():
            for mobile in (False, True):
                suffix = f"mobile-{theme}" if mobile else theme
                path = out / f"bench-sdk-modes-{suffix}.svg"
                path.write_text(client_modes_card(modes, colours, mobile), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
