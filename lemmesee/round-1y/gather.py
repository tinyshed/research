"""Round 1y: the report's raw output gathered beside it.

    python -I gather.py <reports/data/keys-labels-...>

Writes, from the ignored corpus, what the report's figures were read from:

    weights.json              the weights' table by size, both seeds, and the calibration of the frozen score
    fits.json                 every first and second stage fitted here: seed, sample, thresholds, validation figures
    variants.json             every variant's thresholds by size
    development.json          every variant on every development set of round 1v, declared and weighted, the GitHub four pooled
    development-per-database.json   the counts a database behind it
    choice.json, frozen.json  the rule as it chose, and what was frozen before plain A or style 3 was scored
    heldout-<set>.json        every frozen variant and control on plain A and style 3, with the counts a database
    judge-<set>-count.json, judge-<set>-pairs.json   the judge's sample: strata, answers, estimates
    logs/                     the runs' output, machine paths replaced

No schema's text is written.
"""

import json
import re
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import pu  # noqa: E402

OUT = pu.OUT
REPO = HERE.parent.parent


def clean(text: str) -> str:
    text = text.replace(str(REPO), "<repo>").replace(str(REPO).replace("\\", "/"), "<repo>")
    text = re.sub(r"[A-Za-z]:[\\/]Users[\\/][^\\/\s\"']+", "<home>", text)
    return text


def main(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    for name in ("weights.json", "variants.json", "choice.json", "frozen.json"):
        if (OUT / name).exists():
            shutil.copy(OUT / name, out / name)
    fits = {"first": {}, "second": {}}
    for p in sorted((OUT / "first").glob("*/first.json")):
        fits["first"][p.parent.name] = json.load(open(p, encoding="utf-8"))
    for p in sorted((OUT / "second").glob("*/second.json")):
        r = json.load(open(p, encoding="utf-8"))
        total = max(sum(r["importance"].values()), 1)
        r["importance, the ten largest shares"] = {k: round(v / total, 4) for k, v in list(r["importance"].items())[:10]}
        r.pop("importance")
        r.pop("features")
        fits["second"][p.parent.name] = r
    (out / "fits.json").write_text(json.dumps(fits, indent=1) + "\n", encoding="utf-8")
    if (OUT / "development.json").exists():
        dev = json.load(open(OUT / "development.json", encoding="utf-8"))
        per = dev.pop("per database")
        (out / "development.json").write_text(json.dumps(dev, indent=1) + "\n", encoding="utf-8")
        (out / "development-per-database.json").write_text(json.dumps(per) + "\n", encoding="utf-8")
    for which in ("plain-a", "style3"):
        if (OUT / f"heldout-{which}.json").exists():
            shutil.copy(OUT / f"heldout-{which}.json", out / f"heldout-{which}.json")
        d = OUT / f"judge-{which}"
        if (d / "count.json").exists():
            shutil.copy(d / "count.json", out / f"judge-{which}-count.json")
            key = json.load(open(d / "key.json", encoding="utf-8"))
            answers = {}
            for f in sorted((d / "answers").glob("packet-*.json")):
                answers.update(json.load(open(f, encoding="utf-8")))
            for pid, v in key.items():
                v["answer"] = answers.get(pid)
            (out / f"judge-{which}-pairs.json").write_text(json.dumps(key, indent=1) + "\n", encoding="utf-8")
            shutil.copy(d / "totals.json", out / f"judge-{which}-totals.json")
    (out / "logs").mkdir(exist_ok=True)
    for p in sorted((OUT / "logs").glob("*.log")):
        (out / "logs" / p.name).write_text(clean(p.read_text(encoding="utf-8", errors="replace")), encoding="utf-8")
    print(sorted(p.name for p in out.iterdir()))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
