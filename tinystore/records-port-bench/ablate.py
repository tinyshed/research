"""Turns off one of the Go engine's codings at a time and measures the corpus of records with fields.

    python ablate.py <go-repo> <corpus>

<go-repo> is TinyStore at e81a050 with go/zz_fields_test.go copied into its
records/; each coding is turned off by an edit of records/ints.go or
records/values.go, which `git checkout` gives back after its run.
"""
import os
import pathlib
import re
import subprocess
import sys

go = pathlib.Path(sys.argv[1])
env = dict(os.environ, TINYSTORE_SPIKE="1", TINYSTORE_CORPUS=sys.argv[2])
ablations = {
    "all on": [],
    "no int dictionary, dominant": [("ints.go", "\tif len(values) < 8 {\n\t\treturn out", "\tif true || len(values) < 8 {\n\t\treturn out")],
    "no radix": [("ints.go", "\tif top < math.MaxUint64 {", "\tif false && top < math.MaxUint64 {")],
    "no fse": [("ints.go", "\tif top < 256 && m >= 32 {", "\tif false && top < 256 && m >= 32 {")],
    "no timed integers": [("values.go", "e.timedIntegers(times); ok {", "e.timedIntegers(times); ok && false {")],
    "no uuid": [("values.go", "\tcase n > 0 && uuids == n:", "\tcase false && n > 0 && uuids == n:")],
    "no hex": [("values.go", "\tcase n > 0 && hexes == n:", "\tcase false && n > 0 && hexes == n:")],
    "no exceptions": [("values.go", "\tcase flags == 0 && n >= 8", "\tcase false && flags == 0 && n >= 8")],
    "no quotes taken off": [("values.go", "\tif !quoted {\n\t\te.inner = append(e.inner[:0], values...)",
                             "\tif true || !quoted {\n\t\te.inner = append(e.inner[:0], values...)")],
}
for name, edits in ablations.items():
    for file, old, new in edits:
        path = go / "records" / file
        text = path.read_text(encoding="utf-8")
        assert text.count(old) == 1, (name, old)
        path.write_text(text.replace(old, new), encoding="utf-8", newline="\n")
    try:
        run = subprocess.run(["go", "test", "./records", "-run", "TestCompareFields$", "-count=1", "-v"],
                             cwd=go, env=env, capture_output=True, text=True, timeout=900)
    finally:
        subprocess.run(["git", "checkout", "--", "records/ints.go", "records/values.go"], cwd=go, check=True)
    out = run.stdout + run.stderr
    whole = re.search(r"whole=([\d.]+)", out)
    slots = dict(re.findall(r"slot (\w+)\s+([\d.]+)", out))
    print(f"{name:<30} whole={whole.group(1) if whole else out[-400:]} attr={slots.get('attr')} "
          f"level={slots.get('level')} shape={slots.get('shape')} body={slots.get('body')} time={slots.get('time')}",
          flush=True)
