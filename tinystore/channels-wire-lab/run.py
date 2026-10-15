#!/usr/bin/env python3
"""Run serially, after every build and other benchmark has stopped."""
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import subprocess as sp

ROOT=Path(__file__).resolve().parent
OUT=ROOT.parent/'reports/data/index-foundations-2026-10-15/channels.jsonl'
REV=sp.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
TARGET=Path(os.environ.get('LAB_TARGET','/target/release'))
SCRATCH=Path(os.environ.get('LAB_SCRATCH','/data/channels-wire-lab'))
ENV=dict(os.environ,LAB_LIB=str(TARGET/'libchannels_wire_lab.so'),SDK_ROOT=os.environ.get('SDK_ROOT','/data/sdk-js'))
SCRATCH.mkdir(parents=True,exist_ok=True)

def cpu(pid):
    # Fields after comm are fixed; our daemon's comm contains no spaces.
    s=Path(f'/proc/{pid}/stat').read_text().split()
    return (int(s[13])+int(s[14]))/os.sysconf('SC_CLK_TCK')

for mode in ('embedded','unix','tcp'):
    for n,connections,profile in [(100,1,'fast'),(100,100,'fast'),(1000,1,'fast'),(1000,1000,'fast'),(1000,1,'slow')]:
        for passno in (1,2):
            for algo in ('broadcast','ring') if passno==1 else ('ring','broadcast'):
                label=f'{mode}-{n}-{connections}-{profile}-{algo}'
                directory=SCRATCH/'store';shutil.rmtree(directory,ignore_errors=True)
                socket=SCRATCH/'lab.sock';socket.unlink(missing_ok=True)
                env=dict(ENV,BENCH_DB=str(directory),LAB_SOCKET=str(socket))
                server=None
                try:
                    if mode!='embedded':
                        server=sp.Popen([str(TARGET/'channels-wire-lab'),str(directory),algo,mode,str(socket)],stdout=sp.PIPE,stderr=sp.PIPE,text=True)
                        assert server.stdout.readline().strip()=='ready'
                    before=cpu(server.pid) if server else None
                    stamp=dt.datetime.now(dt.timezone.utc).isoformat()
                    p=sp.run(['bun',str(ROOT/'client.ts'),mode,algo,str(n),str(connections),profile],env=env,capture_output=True,text=True,timeout=180)
                    record={'revision':REV,'utc':stamp,'label':label,'pass_number':passno,'exit':p.returncode,'stderr':p.stderr.replace(str(ROOT),'<harness>').replace(str(SCRATCH),'<scratch>'),'command':f'SDK_ROOT=<sdk> LAB_LIB=<target>/libchannels_wire_lab.so BENCH_DB=<scratch>/store LAB_SOCKET=<scratch>/lab.sock bun client.ts {mode} {algo} {n} {connections} {profile}'}
                    if p.returncode==0:
                        record['result']=json.loads(p.stdout.strip().splitlines()[-1])
                        if server:record['daemon_cpu_s']=cpu(server.pid)-before
                    else:record['stdout']=p.stdout
                    with OUT.open('a') as f:f.write(json.dumps(record)+'\n')
                    print(label,passno,p.returncode,flush=True)
                    if p.returncode:raise RuntimeError(record)
                finally:
                    if server:server.terminate();server.wait(timeout=5)
                    shutil.rmtree(directory,ignore_errors=True);socket.unlink(missing_ok=True)
