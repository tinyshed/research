"""Separate strace diagnostic. Run only after timing; no traced rates are findings."""
from pathlib import Path
import subprocess, datetime, json, re, sys
root=Path(__file__).resolve().parent
module=root.parents[2]/'wal-load'
out=root/'trace';out.mkdir(exist_ok=True)
if '--parse-only' not in sys.argv:
    for mode in ['legacy','optimized','full']:
        prefix=out/mode
        start=datetime.datetime.now(datetime.timezone.utc).isoformat()
        with prefix.with_suffix('.jsonl').open('w') as stdout,prefix.with_suffix('.stderr').open('w') as stderr:
            p=subprocess.run(['strace','-f','-T','-ttt','-yy','-e','trace=fdatasync,fsync,openat','-o',str(prefix.with_suffix('.trace')),str(module/'target/release/tinystore-wal-load'),mode,'5','100'],cwd=module,stdout=stdout,stderr=stderr)
        end=datetime.datetime.now(datetime.timezone.utc).isoformat()
        text=prefix.with_suffix('.trace').read_text()
        text=re.sub(r'/workspace/scratch/replication-runs/[^/"<> ]+', '<tmp>', text)
        text=text.replace(str(module), '<spike>').replace('/workspace/scratch/replication-runs', '<scratch>')
        prefix.with_suffix('.trace').write_text(text)
        prefix.with_suffix('.meta.json').write_text(json.dumps({'mode':mode,'start_utc':start,'end_utc':end,'exit':p.returncode,'warning':'traced syscall diagnostic only; rates are not normal throughput'},indent=2)+'\n')
        if p.returncode:raise RuntimeError(mode)
        print('traced',mode,flush=True)
# Join unfinished syscalls by tid, then keep only the marked timed region.
summary={}
for mode in ['legacy','optimized','full']:
    active=False;pending={};rows=[];groups={}
    for line in (out/f'{mode}.trace').read_text().splitlines():
        if 'timed-begin' in line and 'openat(' in line:active=True
        if 'timed-end' in line and 'openat(' in line:active=False
        if not active:continue
        parts=line.split(None,2)
        if len(parts)<3:continue
        tid,timestamp,call=parts
        if '<unfinished ...>' in call:
            pending[tid]=call.replace('<unfinished ...>','');continue
        if call.startswith('<... ') and ' resumed>' in call:
            old=pending.pop(tid,None)
            if old is None:continue
            call=old+call.split(' resumed>',1)[1]
        m=re.match(r'(fdatasync|fsync)\(\d+<(.*)>\s*\).* = 0 <([\d.]+)>$',call)
        if not m:continue
        syscall,path,seconds=m.groups()
        group='feed' if '/feed/' in path and path.endswith('.log') else 'native_wal' if path.endswith('app.db-wal') else 'native_db' if path.endswith('app.db') else 'other'
        duration=float(seconds)
        rows.append({'syscall':syscall,'file_group':group,'seconds':duration})
        g=groups.setdefault(group,{'count':0,'seconds':0});g['count']+=1;g['seconds']+=duration
    result=json.loads((out/f'{mode}.jsonl').read_text().splitlines()[-1])
    assert groups.get('feed',{}).get('count',0)==result['stats']['syncs'], mode
    summary[mode]={'groups':groups,'calls':rows}
(out/'sync-derived.json').write_text(json.dumps(summary,indent=2)+'\n')
for mode,r in summary.items():print(mode,r['groups'])
