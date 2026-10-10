"""Reads a .cpuprofile and prints where the samples fall: by function alone,
and by function with all it calls."""
import collections
import json
import sys

profile = json.load(open(sys.argv[1]))
most = int(sys.argv[2]) if len(sys.argv) > 2 else 40
nodes = {node["id"]: node for node in profile["nodes"]}
parent = {}
for node in profile["nodes"]:
    for child in node.get("children", []):
        parent[child] = node["id"]

samples, deltas = profile.get("samples", []), profile.get("timeDeltas", [])
last = float(sys.argv[3]) if len(sys.argv) > 3 else None
if last is not None and deltas:
    at, times = 0, []
    for delta in deltas:
        at += delta
        times.append(at)
    since = times[-1] - last * 1e6
    samples = [sample for sample, time in zip(samples, times) if time >= since]
    print(f"the last {last}s of {times[-1] / 1e6:.1f}s")
hits = collections.Counter(samples)
if not hits:
    hits = collections.Counter({node["id"]: node.get("hitCount", 0) for node in profile["nodes"]})
total = sum(hits.values())


def name(node):
    frame = node["callFrame"]
    where = frame.get("url", "").rsplit("/", 1)[-1]
    return f"{frame.get('functionName') or '(anonymous)'} {where}:{frame.get('lineNumber', 0) + 1}"


own = collections.Counter()
under = collections.Counter()
for at, count in hits.items():
    own[name(nodes[at])] += count
    seen = set()
    while at is not None:
        label = name(nodes[at])
        if label not in seen:
            under[label] += count
            seen.add(label)
        at = parent.get(at)

print(f"samples {total}")
print("--- own")
for label, count in own.most_common(most):
    print(f"{count / total * 100:6.2f}%  {label}")
print("--- with what it calls")
for label, count in under.most_common(most):
    print(f"{count / total * 100:6.2f}%  {label}")
