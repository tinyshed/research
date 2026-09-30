"""Puts the runs pools.sh made in place of the ones they replace.

    python merge.py results/<date>

For each <engine>-pools.json, the contenders it ran again replace theirs in
<engine>.json, which is first kept as <engine>-before-pools.json. TinyStore,
run again only as a control, keeps its night's runs; the control's go under
"controls", so that the report can show the machine did not drift between.
"""

import json
import pathlib
import sys

directory = pathlib.Path(sys.argv[1])
for pools in sorted(directory.glob("*-pools.json")):
    engine = pools.name.removesuffix("-pools.json")
    base_path = directory / f"{engine}.json"
    before = directory / f"{engine}-before-pools.json"
    if not base_path.exists():
        print(f"{engine}: no {base_path.name}", file=sys.stderr)
        continue
    if not before.exists():
        before.write_text(base_path.read_text())
    base = json.loads(before.read_text())
    again = json.loads(pools.read_text())
    replaced = {run["contender"] for run in again["runs"]} - {"tinystore"}
    base["runs"] = [run for run in base["runs"] if run["contender"] not in replaced] + [
        run for run in again["runs"] if run["contender"] in replaced
    ]
    base["controls"] = [run for run in again["runs"] if run["contender"] == "tinystore"]
    base["pools_rerun"] = {"started": again.get("started"), "contenders": sorted(replaced)}
    if again.get("failed"):
        base.setdefault("failed", []).extend(again["failed"])
    base_path.write_text(json.dumps(base, indent=2))
    print(f"{engine}: replaced {', '.join(sorted(replaced))}; {len(base['controls'])} control runs")
