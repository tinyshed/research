#!/usr/bin/env python3
"""Repeat key resume/switch crash stages at the two main corpus sizes."""
import json
import os
import shutil
import subprocess as sp
import run

for n in (100000,1000000):
    for kind in ('fts','geo'):
        template=run.template(kind,n,'uniform')
        for passno in (1,2):
            for stage in ('chunk-committed','switched'):
                db=run.SCRATCH/'large-crash.db';run.tidy(db);shutil.copyfile(template,db)
                proc=sp.Popen([run.BIN,'crash',kind,'frontier',stage,str(n),'prepared'],env=dict(run.BASE_ENV,BENCH_DB=str(db)),stdout=sp.PIPE,stderr=sp.PIPE,text=True)
                ready=proc.stdout.readline();assert ready and json.loads(ready)['ready'],proc.stderr.read()
                before={'db_bytes':db.stat().st_size,'wal_bytes':os.path.getsize(str(db)+'-wal')}
                proc.kill();proc.wait(timeout=5)
                r=run.command(['recover'],db);r['fault']={'kind':kind,'algorithm':'frontier','stage':stage,'rows':n,'signal':'SIGKILL','before_kill':before}
                run.save('large-crashes',f'{n}-{kind}-{stage}',passno,r);run.tidy(db)
        run.tidy(template)
