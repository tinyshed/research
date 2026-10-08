#!/usr/bin/env python3
"""Complete diagnostics after an allocation-binary execute-mode failure.

No successful timing is rerun. The original runner and binary contents remain.
"""
import datetime as dt,hashlib,json,statistics
from pathlib import Path
import run_capped as r

info=json.loads((r.DATA/'environment.json').read_text())
for name,sha in info['source_sha256'].items():assert hashlib.sha256((r.ROOT/name).read_bytes()).hexdigest()==sha,name
assert hashlib.sha256(r.binary('opt',True).read_bytes()).hexdigest()==info['rust_alloc_binary_sha256']
partial=r.DATA/'allocations.jsonl';backup=r.DATA/'allocations-before-execute-mode-fix.jsonl'
assert not backup.exists();partial.rename(backup)
with partial.open('w') as out:
 for fixture,case in [('typed-go-head','scan_full'),('typed-go-head','scan_filtered'),('typed-native-sealed','scan_full'),('typed-native-sealed','scan_filtered'),('typed-native-sealed','scan_page'),('typed-native-sealed','follow'),('typed-native-sealed','follow_walk')]:
  for label in ['go','previous','opt','sharing_off','cache_raw']:
   value=r.row(label,fixture,case,8,alloc=True);out.write(json.dumps(value)+'\n');out.flush()
with (r.DATA/'memory.jsonl').open('w') as out:
 for pass_id,order in enumerate([['go','previous','opt','sharing_off'],['sharing_off','opt','previous','go'],['previous','go','sharing_off','opt']],1):
  for label in order:
   r.fresh('typed-native-sealed');p=r.call(['/usr/bin/time','-f','%M',*r.args(label,'typed-native-sealed','scan_full',64,mode='memory')]);value=json.loads(p.stdout);value.update(label=label,pass_id=pass_id,peak_rss_kib=int(p.stderr.strip().splitlines()[-1]));out.write(json.dumps(value)+'\n');out.flush()
raw=[]
for name in ['timings.jsonl','ablations.jsonl','fast-cache-supplement.jsonl']:
 raw.extend(json.loads(line) for line in (r.DATA/name).read_text().splitlines())
summary=[]
for key in sorted({(x['track'],x['fixture'],x['case'],x['cache_state']) for x in raw}):
 group=[x for x in raw if (x['track'],x['fixture'],x['case'],x['cache_state'])==key]
 values={label:[x['ns_per_op'] for x in group if x['label']==label] for label in sorted({x['label'] for x in group})}
 assert all(len(v)==6 for v in values.values()),key
 summary.append(dict(zip(['track','fixture','case','cache_state'],key),median_ns={label:statistics.median(v) for label,v in values.items()},passes_ns=values))
r.write('summary.json',summary)
for name,sha in info['source_sha256'].items():assert hashlib.sha256((r.ROOT/name).read_bytes()).hexdigest()==sha,name
for fixture,sha in info['fixture_sha256'].items():assert hashlib.sha256((r.FIXTURES/fixture/'records.db').read_bytes()).hexdigest()==sha,fixture
status={'finished_utc':dt.datetime.now(dt.timezone.utc).isoformat(),'status':'completed all original timings and remaining diagnostics','original_failure':'allocation binary lacked execute permission (0644); exit126; chmod0755 restored execution without changing content','helper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'allocation_binary_sha256':info['rust_alloc_binary_sha256'],'original_runtime_and_runner_source_hashes_preserved':True,'fixture_hashes_preserved':True,'completed_timing_rows':len(raw),'summary_groups':len(summary),'timings_not_repeated':True}
r.write('completion.json',status);print(json.dumps(status))
