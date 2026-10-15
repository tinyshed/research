#!/usr/bin/env python3
"""Longer paired controls using the exact same committed release binary."""
from pathlib import Path
import sys
from run import run_case

output = Path(sys.argv[1])
assert not output.exists(), "never overwrite evidence"
for repeat, order in enumerate((['broadcast', 'ring', 'queues'], ['queues', 'ring', 'broadcast'])):
    for kind in order:
        run_case(['channels', kind, 1000, 30000, 64, 'paced', 1024], output, f'channel-soak-{repeat}-{kind}')
for repeat, order in enumerate((['sql-poll', 'admitted'], ['admitted', 'sql-poll'])):
    for kind in order:
        run_case(['leases', kind, 1000, 30, 'hot', 1, 'FULL', 64], output, f'lease-soak-{repeat}-{kind}')
