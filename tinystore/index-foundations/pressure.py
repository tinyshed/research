#!/usr/bin/env python3
"""ENOSPC correctness only, in this container's disposable 64 MiB /dev/shm."""
import json
import os
from pathlib import Path
import subprocess as sp
import run

root=Path('/dev/shm/index-foundations-enospc')
root.mkdir(exist_ok=True)
for passno in (1,2):
    for kind in ('fts','geo'):
        db=root/'case.db';run.tidy(db);padding=root/'padding'
        try:
            proc=sp.Popen([run.BIN,'crash',kind,'frontier','prepared'],env=dict(run.BASE_ENV,BENCH_DB=str(db)),stdout=sp.PIPE,stderr=sp.PIPE,text=True)
            assert json.loads(proc.stdout.readline())['ready'];proc.kill();proc.wait(timeout=5)
            available=os.statvfs(root).f_bavail*os.statvfs(root).f_frsize
            target=available-8192;fd=os.open(padding,os.O_WRONLY|os.O_CREAT,0o600)
            try:
                chunk=bytes(1<<20)
                while target:
                    written=os.write(fd,chunk[:min(target,len(chunk))]);target-=written
                os.fsync(fd)
            finally:os.close(fd)
            failed=run.command(['recover'],db)
            assert failed['exit']!=0,failed
            assert 'full' in failed['stderr'].lower(),failed
            padding.unlink()
            recovered=run.command(['recover'],db)
            recovered['expected_enospc_failure']=failed
            recovered['pressure_scope']='64 MiB tmpfs, correctness only; 8192 bytes free before failed resume'
            run.save('pressure',kind+'-ENOSPC-resume',passno,recovered)
        finally:
            padding.unlink(missing_ok=True);run.tidy(db)
root.rmdir()
