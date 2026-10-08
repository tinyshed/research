#!/usr/bin/env python3
"""Append the independent Linux sync-policy experiment from retained raw rows."""
import json,re,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parent;DATA=ROOT.parent/'reports/data/kv-optimization-2026-10-09/fdatasync';REPORT=ROOT.parent/'reports/kv-optimization-2026-10-09.md'
def main():
 env=json.loads((DATA/'environment.json').read_text());rows=[json.loads(x)for x in(DATA/'runs.jsonl').read_text().splitlines()];syscalls=[json.loads(x)for x in(DATA/'syscalls.jsonl').read_text().splitlines()]
 def get(c,v):return sorted([r for r in rows if r['case']==c and r['variant']==v],key=lambda r:r['pass'])
 def median(c,v):return statistics.median(r['ns_op']for r in get(c,v))/1000
 def ratios(c,a,b):return [x['ns_op']/y['ns_op']for x,y in zip(get(c,a),get(c,b))]
 cases=list(env['fixed_counts']);summary={c:{v:{'median_us_op':median(c,v),'passes_us_op':[r['ns_op']/1000 for r in get(c,v)]}for v in ['go','fsync','fdatasync']}for c in cases};(DATA/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
 p=['## Linux sync-policy supplement','','This is a separate same-session causal ablation, with its own Go/current-native baseline. Its ratios are not combined with the earlier six-way collection. The candidate links the exact same final optimized Rust source against an isolated SQLite archive compiled with all original flags plus only `-DHAVE_FDATASYNC=1`. The shared archive, previous binaries and measured Rust/Go source remain unchanged.','',
 'The pinned amalgamation (`sqlite3.c` SHA-256 `b1dd5d74ec7f29055a6684fa06fb3c2f6821c87dd38f9a458dfd2e8a1db28189`, lines 43928–44041) aliases fdatasync to fsync unless configured; its Unix full_fsync chooses fdatasync on supported Linux systems when enabled. The option is explicitly documented in [SQLite platform configuration](https://sqlite.org/compile.html#have_fdatasync). This changes the Unix sync implementation; it keeps the WAL/FULL synchronization policy, savepoints, commit ordering and SQL. [SQLite synchronous](https://sqlite.org/pragma.html#pragma_synchronous) describes the commit sync required by FULL in WAL mode. This experiment does not reduce synchronous to NORMAL/OFF, skip sync calls or prove power-loss behavior on hardware. It is Linux-specific and does not propose this flag as a Windows/macOS default.','',
 f"Archive SHA-256 `{env['build']['archive_sha256']}` versus original `{env['build']['baseline_archive_sha256']}`; candidate executable `{env['build']['binary_sha256']}`. Exact flags, C/header/Rust source hashes, crate and binary comparison, source IDs and reader/writer PRAGMA readback are in [build.json](data/kv-optimization-2026-10-09/fdatasync/build.json). Both native variants and production Go read back `synchronous=2`, `journal_mode=wal`, `wal_autocheckpoint=1000`; all native compile-option inventories match because this platform macro is not enumerated by compile_options. The source and observed syscall change, rather than that inventory alone, identify the ablation.",'',
 f"Collection UTC `{env['started_utc']}` to `{env['finished_utc']}`. Six balanced triples alternate Go→fsync→fdatasync and the reverse. Every fresh closed fixture begins without a WAL, uses the exact `(iteration×997)` key permutation and 64 warmups; initial revision and final committed rows/versions/expiry/spills are retained and must match. Counts are fixed equally across candidates and cover repeated WAL/checkpoint work. Phase evidence from the earlier experiment explains the hypothesis; only this supplement's same-session baselines support its speed comparison.",'',
 '| Case | Count/pass | Go median μs/op | Native fsync | Native fdatasync | fsync/fdatasync | Go/fdatasync |','|---|---:|---:|---:|---:|---:|---:|']
 for c in cases:p.append(f"| `{c}` | {env['fixed_counts'][c]:,} | {median(c,'go'):.3f} | {median(c,'fsync'):.3f} | {median(c,'fdatasync'):.3f} | {statistics.median(ratios(c,'fsync','fdatasync')):.3f}× | {statistics.median(ratios(c,'go','fdatasync')):.3f}× |")
 p+=['','Per-pass synchronous loop elapsed μs/op (0–5):','', '| Case | Variant | Passes |','|---|---|---|']
 for c in cases:
  for v in ['go','fsync','fdatasync']:p.append(f"| `{c}` | {v} | "+'; '.join(f'{r["ns_op"]/1000:.3f}'for r in get(c,v))+' |')
 p+=['','Separate whole-process strace counts include 512 timed writes, 64 warmups, setup and close; ptrace elapsed values are excluded from speed comparisons:','', '| Case | Variant | fsync calls | fdatasync calls | pwrite64 |','|---|---|---:|---:|---:|']
 for r in syscalls:
  counts={}
  for l in r['diagnostic_stderr'].splitlines():
   a=l.split()
   if len(a)>=5 and a[-1]in['fsync','fdatasync','pwrite64']:counts[a[-1]]=int(a[3])
  p.append(f"| `{r['case']}` | {r['variant']} | {counts.get('fsync',0)} | {counts.get('fdatasync',0)} | {counts.get('pwrite64',0)} |")
 p+=['','The same complete 56-command and 56-command no-result oracle, 32 typed cross-read/rewrites, ownership/bounds/Take/TTL/reopen tests and native unit tests pass for the relinked candidate. Committed state/checksum parity is also checked after every timed triple. Matching sync counts and FULL readback support unchanged synchronization policy; they are not a standalone crash/power-loss proof. All previous runtime omissions and the WSL2 scope still apply.','',
 'Reproduce after building the ordinary optimized binaries:','', '```sh','cd <repo>/tinystore/kv-opt-bench','python3 fdatasync_build.py','# Obtain a separate quiet research window before calibration/timing/strace:','GOMAXPROCS=1 GOGC=100 taskset -c 2 python3 fdatasync_run.py','python3 fdatasync_report.py','```','',
 'Supplement artifacts: [runs](data/kv-optimization-2026-10-09/fdatasync/runs.jsonl), [calibration](data/kv-optimization-2026-10-09/fdatasync/calibration.jsonl), [summary](data/kv-optimization-2026-10-09/fdatasync/summary.json), [syscalls](data/kv-optimization-2026-10-09/fdatasync/syscalls.jsonl), [environment and consistency](data/kv-optimization-2026-10-09/fdatasync/environment.json), [correctness](data/kv-optimization-2026-10-09/fdatasync/correctness.json).']
 original=REPORT.read_text();anchor='\n## Linux sync-policy supplement\n';original=original.split(anchor)[0];REPORT.write_text(original.rstrip()+'\n\n'+'\n'.join(p)+'\n');print(REPORT)
if __name__=='__main__':main()
