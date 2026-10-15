import json,os,platform,subprocess,datetime
from pathlib import Path

def cmd(*args):
 return subprocess.check_output(args,text=True).strip()
def read(path):
 return Path(path).read_text().strip()
env={
 'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'kernel':platform.release(),'machine':platform.machine(),'cpu_model':next(l.split(':',1)[1].strip() for l in read('/proc/cpuinfo').splitlines() if l.startswith('model name')),
 'visible_cpus':os.cpu_count(),'affinity_cpus':len(os.sched_getaffinity(0)),
 'cpu_max':read('/sys/fs/cgroup/cpu.max'),'memory_max':read('/sys/fs/cgroup/memory.max'),
 'rustc':cmd('rustc','--version'),'cargo':cmd('cargo','--version'),'bun':cmd('bun','--version'),
 'research':cmd('git','-C',str(Path(__file__).resolve().parents[2]),'rev-parse','HEAD'),
 'product':cmd('git','-C',str(Path(__file__).resolve().parent/'source'),'rev-parse','HEAD'),
 'tmp_filesystem':cmd('findmnt','-n','-o','FSTYPE','-T',os.environ.get('TMPDIR','/tmp')),
 'tmpdir':'<disk-backed-scratch>',
 'sqlite_flags':read(Path(__file__).resolve().parent/'.cargo/config.toml'),
 'method':'release build; no concurrent build, test or lint; synthetic corpus generated outside timings; process-local loopback TCP for network; source SQLite FULL',
}
print(json.dumps(env,indent=2))
