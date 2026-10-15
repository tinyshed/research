#!/usr/bin/env python3
from pathlib import Path
import sys
from run import run_case

output=Path(sys.argv[1])
assert not output.exists(), 'never overwrite evidence'
for repeat in range(2):
    for clients in (10,100,1000):
        for kind in (['fifo','notify'] if repeat==0 else ['notify','fifo']):
            run_case(['lease-memory',kind,clients],output,f'waiters-{repeat}-{clients}-{kind}')
