"""Draws the report's line charts from a results directory's rounds.

    python charts.py results/<date> <out-dir>

latency-<engine>.svg: p99 against the rate each contender achieved at a
quarter, a half, three quarters and nine tenths of its own maximum, from the
*-latency rounds; the knee is where a contender stops keeping up.
steady-<engine>.svg: requests a second, minute by minute, from the *-steady
rounds; a line that sinks is a store that slows as it grows.

One SVG a GitHub theme, as cards.py draws them. A README image cannot hover,
so every line is labelled at its end as well as in the legend, and the
figures are in the report's tables.
"""

import json
import math
import pathlib
import statistics
import sys
from xml.sax.saxutils import escape

# Categorical slots 1-3 of the dataviz reference palette, which pass its
# validator on GitHub's surfaces in both themes; TinyStore is always slot 1.
THEMES = {
    "light": {"text": "#1f2328", "muted": "#59636e", "grid": "#d1d9e0", "border": "#d1d9e0",
              "series": ["#2a78d6", "#1baf7a", "#eb6834"], "surface": "#ffffff"},
    "dark": {"text": "#f0f6fc", "muted": "#9198a1", "grid": "#3d444d", "border": "#3d444d",
             "series": ["#3987e5", "#199e70", "#d95926"], "surface": "#0d1117"},
}
FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"
WIDTH, HEIGHT = 816, 380
LEFT, RIGHT, TOP, BOTTOM = 64, 150, 100, 44

# label, contender; at most three, so that every pair stays apart
LINES = {
    "stack": [("TinyStore", "tinystore"), ("sidecar", "tinystore-sidecar"), ("services", "services")],
    "kv": [("TinyStore", "tinystore"), ("Redis", "redis"), ("Pebble", "pebble")],
}
TITLES = {
    "latency": ("{what}: p99 as the load rises", "p99 (log) against requests a second achieved · lower and further right is better"),
    "steady": ("{what}: fifteen minutes at 64 clients", "requests a second, a minute each · flat is better"),
}
WHAT = {"stack": "An application", "kv": "KV, 90 % reads"}


def rate(v):
    if v >= 1_000_000:
        return f"{v / 1e6:.1f} M"
    if v >= 1_000:
        return f"{v / 1e3:.0f} k"
    return f"{v:.0f}"


def micros(v):
    if v >= 1000:
        return f"{v / 1000:.0f} ms" if v >= 10_000 else f"{v / 1000:.1f} ms"
    return f"{v:.0f} µs"


def latency_points(round_, contender):
    """(achieved per second, p99 µs) at each offered share, medians over repeats"""
    by_share = {}
    for run in round_["runs"]:
        if run["contender"] != contender:
            continue
        for s in run["stages"]:
            if s.get("offered_per_second") and not s.get("errors"):
                by_share.setdefault(s["name"], []).append((s["per_second"], s["p99_us"]))
    points = [(statistics.median(p[0] for p in v), statistics.median(p[1] for p in v)) for v in by_share.values()]
    return sorted(points)


def steady_points(round_, contender):
    """(minute, per second) of the first run's timeline"""
    for run in round_["runs"]:
        if run["contender"] == contender:
            for s in run["stages"]:
                if s.get("timeline"):
                    return [(w["at_seconds"] / 60, w["per_second"]) for w in s["timeline"]]
    return []


def nice_ticks(low, high, count=5):
    step = 10 ** math.floor(math.log10((high - low) / count or 1))
    for m in (1, 2, 5, 10):
        if (high - low) / (step * m) <= count:
            step *= m
            break
    first = math.floor(low / step) * step
    return [first + i * step for i in range(int((high - first) / step) + 2) if first + i * step <= high * 1.0001]


def log_ticks(low, high):
    ticks, decade = [], 10 ** math.floor(math.log10(low))
    while decade <= high * 10:
        for m in (1, 2, 5):
            if low <= decade * m <= high:
                ticks.append(decade * m)
        decade *= 10
    return ticks


def chart(title, subtitle, series, colours, x_label, y_log, x_format, y_format):
    xs = [x for _, points in series for x, _ in points]
    ys = [y for _, points in series for _, y in points]
    plot_w, plot_h = WIDTH - LEFT - RIGHT, HEIGHT - TOP - BOTTOM
    x_high = max(xs) * 1.05
    if y_log:
        y_low, y_high = 10 ** math.floor(math.log10(min(ys))), 10 ** math.ceil(math.log10(max(ys)))
        y_ticks = log_ticks(y_low, y_high)

        def y_of(v):
            return TOP + plot_h * (1 - (math.log10(v) - math.log10(y_low)) / (math.log10(y_high) - math.log10(y_low)))
    else:
        y_low, y_high = 0, max(ys) * 1.1
        y_ticks = nice_ticks(0, y_high)

        def y_of(v):
            return TOP + plot_h * (1 - v / y_high)

    def x_of(v):
        return LEFT + plot_w * v / x_high

    out = [
        f'<rect x="0.5" y="0.5" width="{WIDTH - 1}" height="{HEIGHT - 1}" rx="6" fill="none" stroke="{colours["border"]}"/>',
        f'<text x="20" y="36" font-size="16" font-weight="600" fill="{colours["text"]}">{escape(title)}</text>',
        f'<text x="20" y="56" font-size="12" fill="{colours["muted"]}">{escape(subtitle)}</text>',
    ]
    for t in y_ticks:
        y = y_of(t)
        out.append(f'<line x1="{LEFT}" x2="{LEFT + plot_w}" y1="{y:.1f}" y2="{y:.1f}" stroke="{colours["grid"]}" '
                   f'stroke-width="1"/>')
        out.append(f'<text x="{LEFT - 8}" y="{y + 4:.1f}" font-size="11" text-anchor="end" '
                   f'fill="{colours["muted"]}">{y_format(t)}</text>')
    for t in nice_ticks(0, x_high):
        x = x_of(t)
        out.append(f'<text x="{x:.1f}" y="{TOP + plot_h + 18}" font-size="11" text-anchor="middle" '
                   f'fill="{colours["muted"]}">{x_format(t)}</text>')
    out.append(f'<text x="{LEFT + plot_w}" y="{TOP + plot_h + 36}" font-size="11" text-anchor="end" '
               f'fill="{colours["muted"]}">{escape(x_label)}</text>')
    # legend, one row above the plot
    legend_x = LEFT
    for i, (label, _) in enumerate(series):
        colour = colours["series"][i]
        out.append(f'<line x1="{legend_x}" x2="{legend_x + 16}" y1="{TOP - 24}" y2="{TOP - 24}" stroke="{colour}" '
                   f'stroke-width="2"/>')
        out.append(f'<text x="{legend_x + 22}" y="{TOP - 20}" font-size="12" fill="{colours["text"]}">'
                   f'{escape(label)}</text>')
        legend_x += 34 + 7 * len(label)
    ends = []
    for i, (label, points) in enumerate(series):
        colour = colours["series"][i]
        path = " ".join(f"{'M' if j == 0 else 'L'}{x_of(x):.1f},{y_of(y):.1f}" for j, (x, y) in enumerate(points))
        out.append(f'<path d="{path}" fill="none" stroke="{colour}" stroke-width="2" stroke-linejoin="round"/>')
        if len(points) <= 12:
            for x, y in points:
                out.append(f'<circle cx="{x_of(x):.1f}" cy="{y_of(y):.1f}" r="4" fill="{colour}" '
                           f'stroke="{colours["surface"]}" stroke-width="2"/>')
        last_x, last_y = points[-1]
        ends.append([y_of(last_y), x_of(last_x), label, y_format(last_y) if y_log else rate(last_y), colour])
    # direct labels at the lines' ends, pushed apart so that none overlaps
    ends.sort()
    for i in range(1, len(ends)):
        ends[i][0] = max(ends[i][0], ends[i - 1][0] + 15)
    for y, x, label, value, colour in ends:
        out.append(f'<circle cx="{LEFT + plot_w + 14}" cy="{y:.1f}" r="4" fill="{colour}"/>')
        out.append(f'<text x="{LEFT + plot_w + 24}" y="{y + 4:.1f}" font-size="12" fill="{colours["text"]}">'
                   f'{escape(label)} <tspan fill="{colours["muted"]}">{escape(value)}</tspan></text>')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}" '
            f'font-family="{FONT}">\n' + "\n".join(out) + "\n</svg>\n")


def main():
    directory, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    for kind, points_of, x_label, y_log, x_format, y_format in (
        ("latency", latency_points, "requests a second", True, rate, micros),
        ("steady", steady_points, "minutes", False, lambda v: f"{v:.0f}", rate),
    ):
        for engine, lines in LINES.items():
            path = directory / f"{engine}-{kind}.json"
            if not path.exists():
                print(f"{kind}-{engine}: no {path.name}, skipped", file=sys.stderr)
                continue
            round_ = json.loads(path.read_text())
            series = [(label, points_of(round_, c)) for label, c in lines]
            series = [(label, p) for label, p in series if p]
            if not series:
                continue
            title, subtitle = TITLES[kind]
            for theme, colours in THEMES.items():
                svg = chart(title.format(what=WHAT[engine]), subtitle, series, colours, x_label, y_log, x_format,
                            y_format)
                (out / f"{kind}-{engine}-{theme}.svg").write_text(svg, encoding="utf-8", newline="\n")
            print(f"{kind}-{engine}: " + "; ".join(f"{label} {len(p)} points" for label, p in series), file=sys.stderr)


if __name__ == "__main__":
    main()
