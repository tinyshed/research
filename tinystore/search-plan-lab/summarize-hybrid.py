"""The hybrid round's tables: python summarize-hybrid.py <data dir>.

Every mode's nDCG@10 over each cohort's 600 questions, then by the kind of
question: one naming a thing by a code (a model number, a version, an
identifier in code) against one in words alone; ESCI's also by its graded
labels, Exact 3, Substitute 2, Complement 1, Irrelevant 0.
"""

import json
import math
import re
import sys
from collections import defaultdict
from pathlib import Path

data = Path(sys.argv[1])

# A code: letters and digits in one token (s10e, rtx3080, 4k), an identifier
# written as code (snake_case, camelCase, dotted.name, call(), --flag), or a
# constant with an underscore (ERR_CONNECTION_RESET).
CODE = re.compile(
    r"(?:^|\s)(?=\S*\d)(?=\S*[^\W\d_])\S+"
    r"|\b[a-z]+_[a-z0-9_]+\b"
    r"|\b[a-z]+[A-Z][A-Za-z0-9]*\b"
    r"|\b\w+\.\w+\b"
    r"|\w+\(\)"
    r"|(?:^|\s)--?[a-z][\w-]*"
    r"|\b[A-Z][A-Z0-9]*_[A-Z0-9_]+\b"
)


def kind(text):
    return "code" if CODE.search(text) else "words"


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join("---" for _ in header) + "|")
    for row in rows:
        print("| " + " | ".join(str(cell) for cell in row) + " |")
    print()


def ndcg(found, gains):
    dcg = sum(gains.get(doc, 0) / math.log2(rank + 2) for rank, doc in enumerate(found[:10]))
    ideal = sorted(gains.values(), reverse=True)[:10]
    best = sum(gain / math.log2(rank + 2) for rank, gain in enumerate(ideal) if gain > 0)
    return dcg / best if best else 0.0


def graded(cohort):
    gains = defaultdict(dict)
    for path in sorted((data / f"raw-{cohort}").glob("graded-*.tsv")):
        locale = path.stem.split("-")[1]
        for line in path.read_text(encoding="utf-8").splitlines():
            question, _, doc, grade = line.split("\t")
            gains[f"esci-{locale}:{question}"][f"esci-{locale}:{doc}"] = int(grade)
    return gains


print("## Every mode, all questions\n")
rows = []
for line in (data / "hybrid.jsonl").read_text(encoding="utf-8").splitlines():
    h = json.loads(line)
    rows.append([h["context"]["arm"], h["mode"], f"{h['ndcg10']:.4f}", f"{h['recall10']:.4f}", f"{h['p50_ms']:.2f}",
                 f"{h['p99_ms']:.2f}"])
table(["vectors by", "mode", "nDCG@10", "Recall@10", "p50 ms", "p99 ms"], rows)

for cohort in ("esci", "cqa"):
    folder = data / f"raw-{cohort}"
    if not folder.exists():
        continue
    asked = json.loads((folder / "questions.json").read_text(encoding="utf-8"))
    binary = {q["id"]: {doc: grade for doc, grade in q["judged"].items()} for q in asked}
    kinds = {q["id"]: kind(q["text"]) for q in asked}
    sets = {q["id"]: q["id"].split(":")[0] for q in asked}
    grades = graded(cohort) if cohort == "esci" else {}
    counted = defaultdict(int)
    for k in kinds.values():
        counted[k] += 1
    print(f"## {cohort}: by kind of question ({dict(counted)})\n")
    rows = []
    for path in sorted(folder.glob("hybrid-*.jsonl")):
        arm = path.stem.removeprefix("hybrid-")
        sums = defaultdict(lambda: defaultdict(list))
        for line in path.read_text(encoding="utf-8").splitlines():
            r = json.loads(line)
            q = r["question"]
            score = ndcg(r["found"], binary[q])
            sums[r["mode"]]["all"].append(score)
            sums[r["mode"]][kinds[q]].append(score)
            sums[r["mode"]][sets[q]].append(score)
            if grades:
                sums[r["mode"]]["graded"].append(ndcg(r["found"], grades[q]))
        for mode, groups in sums.items():
            mean = lambda key: f"{sum(groups[key]) / len(groups[key]):.4f}" if groups.get(key) else ""
            row = [arm, mode, mean("all"), mean("code"), mean("words")]
            if grades:
                row += [mean("graded"), mean("esci-us"), mean("esci-es"), mean("esci-jp")]
            rows.append(row)
    header = ["vectors by", "mode", "nDCG@10", "code", "words"]
    if cohort == "esci":
        header += ["graded", "us", "es", "jp"]
    table(header, rows)
