#!/usr/bin/env python3
"""Offline report tables from retained records optimization data; no benchmark."""
import json,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parent;DATA=ROOT.parent/'reports/data/records-optimization-2026-10-09/capped'
summary=json.loads((DATA/'summary.json').read_text());rows=[]
for name in ['timings.jsonl','ablations.jsonl','fast-cache-supplement.jsonl']:
 rows.extend(json.loads(line) for line in (DATA/name).read_text().splitlines())
def table(headers,values):return '\n'.join(['| '+' | '.join(headers)+' |','|'+'|'.join(['---']*len(headers))+'|',*['| '+' | '.join(map(str,row))+' |' for row in values]])
def number(v,scale=1e6):return f'{v/scale:.3f}'
tables={}
tables['MAIN']=table(['Fixture / artifact','Operation','Go ms','Previous Rust ms','Optimized Rust ms','Previous/optimized'],[[s['fixture'],s['case'],number(s['median_ns']['go']),number(s['median_ns']['previous']),number(s['median_ns']['opt']),f"{s['median_ns']['previous']/s['median_ns']['opt']:.2f}"]for s in summary if s['track']=='main' and s['case'] not in ['follow','follow_walk']])
tables['FOLLOW']=table(['Temperature / fixture','Operation','Go ms','Previous Rust ms','Optimized decoded-cache ms'],[[s['cache_state']+' / '+s['fixture'],s['case'],number(s['median_ns']['go']),number(s['median_ns']['previous']),number(s['median_ns']['opt'])]for s in summary if s['track'] in ['main','cold'] and s['case'] in ['follow','follow_walk']])
tables['ABLATIONS']=table(['Fixture / operation','Variant','Median ms','Relative to opt'],[[s['fixture']+' / '+s['case'],label,number(v),f"{v/s['median_ns']['opt']:.2f}"]for s in summary if s['track']=='ablation' for label,v in s['median_ns'].items()])
tables['FAST_CACHE']=table(['Fixture / operation','Opt decoded µs','Raw cache µs','Cache off µs'],[[s['fixture']+' / '+s['case'],number(s['median_ns']['opt'],1000),number(s['median_ns']['cache_raw'],1000),number(s['median_ns']['cache_off'],1000)]for s in summary if s['track']=='fast_cache_only'])
tables['ARCHIVE']=table(['Operation','Archived Go ms','Extended Go ms','Archived Rust ms','Copied previous Rust ms'],[[s['case'],number(s['median_ns']['go_archive']),number(next(x for x in summary if x['track']=='main' and x['fixture']==s['fixture'] and x['case']==s['case'])['median_ns']['go']),number(s['median_ns']['archive']),number(next(x for x in summary if x['track']=='main' and x['fixture']==s['fixture'] and x['case']==s['case'])['median_ns']['previous'])]for s in summary if s['track']=='archive'])
selected=[s for s in summary if s['track']=='main' and s['fixture']=='typed-native-sealed' and s['case'] in ['scan_full','scan_filtered','scan_page']]
tables['PASSES']=table(['Typed native-sealed case','Go ms, six passes','Previous Rust ms, six passes','Optimized Rust ms, six passes'],[[s['case'],*['; '.join(number(v) for v in s['passes_ns'][label])for label in ['go','previous','opt']]]for s in selected])
memory=[json.loads(line) for line in (DATA/'memory.jsonl').read_text().splitlines()]
tables['MEMORY']=table(['Representation','Current RSS MiB, three passes','Peak RSS MiB, three passes'],[[label,'; '.join(f"{x['rss_kib']/1024:.2f}" for x in memory if x['label']==label),'; '.join(f"{x['peak_rss_kib']/1024:.2f}" for x in memory if x['label']==label)]for label in ['go','previous','opt','sharing_off']])
allocation=[json.loads(line) for line in (DATA/'allocations.jsonl').read_text().splitlines()]
tables['ALLOCATIONS']=table(['Fixture / operation','Go allocs / bytes','Previous Rust allocs / bytes','Optimized Rust allocs / bytes'],[[fixture+' / '+case,*[f"{next(x for x in allocation if x['label']==label and x['fixture']==fixture and x['case']==case)['allocations_per_op']:,.1f} / {next(x for x in allocation if x['label']==label and x['fixture']==fixture and x['case']==case)['allocated_bytes_per_op']:,.0f}" for label in ['go','previous','opt']]]for fixture,case in dict.fromkeys((x['fixture'],x['case'])for x in allocation)])
duration=[]
for track,label in sorted({(x['track'],x['label'])for x in rows}):
 group=[x['ns_per_op']*x['iterations']/1e6 for x in rows if x['track']==track and x['label']==label];duration.append([track,label,f'{min(group):.2f}',f'{max(group):.2f}'])
tables['DURATIONS']=table(['Track','Candidate','Shortest sample ms','Longest sample ms'],duration)
template=ROOT/'report-template.md';text=template.read_text()
for key,value in tables.items():text=text.replace('{{'+key+'}}',value)
assert '{{' not in text
(ROOT.parent/'reports/records-optimization-2026-10-09.md').write_text(text)
print('Offline tables/report assembled from retained JSON')
