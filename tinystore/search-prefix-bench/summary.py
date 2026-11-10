"""The tables of search-prefix-memory from round.sh's raw lines.

    python -I summary.py <data> [measure.jsonl]

Reads <data>/builds.jsonl and <data>/measure.jsonl, or the file named. A
heap is the median of a search's heap passes, with every pass's value when
they differ; a time is the median of its time passes, with their least and
most.
"""

import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

MIB = 1 << 20


def lines(path):
    with open(path, encoding="utf-8") as file:
        return [json.loads(line) for line in file if line.strip()]


def size(count):
    if abs(count) >= MIB:
        return f"{count / MIB:,.1f} MiB"
    if abs(count) >= 1024:
        return f"{count / 1024:,.0f} KiB"
    return f"{count:,} B"


def table(head, rows):
    cells = [head] + [[str(cell) for cell in row] for row in rows]
    widths = [max(len(row[at]) for row in cells) for at in range(len(head))]
    out = ["| " + " | ".join(cell.ljust(widths[at]) for at, cell in enumerate(row)) + " |" for row in cells]
    out.insert(1, "|" + "|".join("-" * (width + 2) for width in widths) + "|")
    return "\n".join(out)


def heap(values):
    middle = statistics.median(values)
    if len(set(values)) == 1:
        return middle, size(int(middle))
    return middle, size(int(middle)) + " (" + "; ".join(size(value) for value in values) + ")"


def times(values):
    values = sorted(values)
    return f"{statistics.median(values) / 1e6:,.2f} ({values[0] / 1e6:,.2f}–{values[-1] / 1e6:,.2f})"


def order(index):
    documents, words = (int(part) for part in index.split("x"))
    return (words != 50, documents if words == 50 else words)


def main(data, name):
    builds = {line["index"]: line for line in lines(data / "builds.jsonl")}
    measured = lines(data / name)
    heaps, timed, answers, arms, rests = defaultdict(list), defaultdict(list), defaultdict(set), {}, {}
    for line in measured:
        if line["step"] == "arms":
            arms[(line["index"], line["memory"])] = line
        elif line["step"] == "rest":
            rests[(line["index"], line["memory"])] = line
        elif line["step"] == "search":
            at = (line["index"], line["memory"], line["arm"])
            answers[at].add("ok" if line["ok"] else line["error_kind"])
            if line["kind"] == "heap":
                heaps[at].append(line)
            elif line["kind"] == "time":
                timed[at].append(line["nanos"])
    indexes = sorted({index for index, _ in arms}, key=order)
    budgets = sorted({memory for _, memory in arms if memory is not None})

    reallocs = sorted({line.get("realloc", "copy") for line in arms.values()})
    print(f"# search-prefix-memory: {name}, a realloc counted as {', '.join(reallocs)}\n")
    print("## Indexes\n")
    rows = []
    for index in indexes:
        build = builds[index]
        rows.append([index, f"{build['documents']:,}", build["words"], f"{build['postings']:,}",
                     f"{build['keys']['written']:,}", build["keys"]["counted_otherwise"],
                     size(build["search_db_bytes"]), f"{build['seconds']:.0f}"])
    print(table(["index", "documents", "words a document", "postings", "keys", "rows counting otherwise",
                 "search.db", "built, s"], rows))

    print("\n## Prefixes: the postings the 64 words each begins hold, over every prefix of the index\n")
    rows = []
    for index in indexes:
        for spread in arms[(index, None)]["spreads"]:
            postings = spread["postings"]
            rows.append([index, spread["letters"], "yes" if spread["read_as_prefix"] else "no", postings["prefixes"],
                         f"{postings['least']:,}", f"{postings['median']:,}", f"{postings['p90']:,}",
                         f"{postings['most']:,}"])
    print(table(["index", "letters", "read as a prefix", "prefixes", "least", "median", "p90", "most"], rows))

    for pair in ["last 1", "last 2", "last 4", "every 1", "every 2", "every 4"]:
        print(f"\n## {pair}, no budget: the heap above the same search without\n")
        rows = []
        for index in indexes:
            off, on = (index, None, pair + ", off"), (index, None, pair)
            described = next(arm for arm in arms[(index, None)]["arms"] if arm["arm"] == pair)
            postings = described["postings"]
            off_heap, off_text = heap([line["heap_peak"] for line in heaps[off]])
            on_heap, on_text = heap([line["heap_peak"] for line in heaps[on]])
            above = on_heap - off_heap
            per = f"{above / postings:.1f}" if postings and above > 0 else "—"
            sqlite = max(line["sqlite_peak"] for line in heaps[on])
            reserved = max(line["reserved"] for line in heaps[on])
            rows.append([index, f"{postings:,}", off_text, on_text, size(int(above)), per, size(sqlite),
                         reserved, "; ".join(sorted(answers[on])), times(timed[off]), times(timed[on])])
        print(table(["index", "postings of its words", "heap, off", "heap, on", "above", "bytes a posting",
                     "SQLite's most, on", "reserved, on", "answered", "ms, off", "ms, on"], rows))
    print("\n## 8 common words, whole, no budget\n")
    rows = []
    for index in indexes:
        at = (index, None, "8 common")
        rows.append([index, heap([line["heap_peak"] for line in heaps[at]])[1],
                     size(max(line["sqlite_peak"] for line in heaps[at])), times(timed[at])])
    print(table(["index", "heap", "SQLite's most", "ms"], rows))

    for budget in budgets:
        print(f"\n## With a budget of {size(budget)}: the most heap a search held against it\n")
        rows = []
        for index in indexes:
            for pair in ["last 2", "last 4", "every 2", "every 4"]:
                at = (index, budget, pair)
                most = max(line["heap_peak"] for line in heaps[at])
                unbounded = heap([line["heap_peak"] for line in heaps[(index, None, pair)]])[0]
                reserved = max(line["reserved"] for line in heaps[at])
                rows.append([index, pair, size(most), f"{most / budget:.2f}", size(int(unbounded)), reserved,
                             "; ".join(sorted(answers[at])), times(timed[at])])
        print(table(["index", "search", "heap, most", "of the budget", "heap without a budget", "reserved",
                     "answered", "ms"], rows))

    print("\n## Held at rest, every search run once\n")
    rows = []
    for index in indexes:
        row = [index]
        for memory in [None] + budgets:
            rest = rests[(index, memory)]
            row += [size(rest["heap"]), size(rest["reserved"])]
        rows.append(row)
    head = ["index"]
    for memory in [None] + budgets:
        name = "no budget" if memory is None else size(memory)
        head += [f"heap, {name}", f"reserved, {name}"]
    print(table(head, rows))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), sys.argv[2] if len(sys.argv) > 2 else "measure.jsonl")
