"""Derive tables/ratios from immutable per-process JSONL; never modify inputs."""
from pathlib import Path
import json, statistics
root=Path(__file__).resolve().parent
results={}
samples={}
for stage in ['main','ablation','soak']:
    results[stage]={}
    samples[stage]={}
    for p in sorted((root/stage).glob('*.jsonl')):
        rows=[json.loads(line) for line in p.read_text().splitlines()]
        found=[r for r in rows if r['case']=='result']
        if len(found)!=1 or not found[0]['verified']:raise ValueError(p)
        r=found[0]
        key=p.stem
        results[stage][key]=r
        samples[stage][key]=[v for v in rows if v['case']=='sample']
summary={'results':results,'main_ratios':[], 'ablation_ratios':{}}
for i in [1,2,3]:
    r=results['main']
    a,b,c=(r[f'main-p{i}-{mode}'] for mode in ['off','legacy','optimized'])
    summary['main_ratios'].append({'pass':i,'legacy_vs_off':b['write_rate']/a['write_rate'],'optimized_vs_off':c['write_rate']/a['write_rate'],'optimized_vs_legacy':c['write_rate']/b['write_rate'],'rss_legacy_vs_optimized':b['max_rss_bytes']/c['max_rss_bytes']})
for numerator,denominator in [('hash','bounded'),('copy','bounded'),('combined','bounded'),('optimized','combined'),('nocache','optimized'),('full','optimized')]:
    ratios=[]
    for i in [1,2,3]:
        a,b=(results['ablation'][f'ablation-p{i}-{mode}'] for mode in [numerator,denominator])
        ratios.append(a['write_rate']/b['write_rate'])
    summary['ablation_ratios'][f'{numerator}/{denominator}']=ratios
summary['stage_microseconds_per_capture']={}
for stage in ['main','ablation']:
    for key,r in results[stage].items():
        if not r['stats']:continue
        s=r['stats']
        summary['stage_microseconds_per_capture'][key]={k: v/1e3/s['commits'] for k,v in s.items() if k.endswith('_ns')}
        summary['stage_microseconds_per_capture'][key]['branches_per_page']=r['branch_hashes']/s['pages']
summary['soak_windows']=[]
for key,rows in samples['soak'].items():
    for begin,end in [(0,30),(30,60),(60,90),(90,120),(120,150),(150,180)]:
        selected=rows[begin:end]
        seconds=selected[-1]['elapsed_seconds']-(rows[begin-1]['elapsed_seconds'] if begin else 0)
        summary['soak_windows'].append({'label':key,'seconds':[begin,end],'write_rate':sum(v['write_delta'] for v in selected)/seconds,'read_rate':sum(v['read_delta'] for v in selected)/seconds,'rss_max_bytes':max(v['rss_bytes'] for v in selected),'wal_max_bytes':max(v['wal_bytes'] for v in selected),'cache_max_bytes':max(v['cache_reserved'] for v in selected),'feed_bytes_at_end':selected[-1]['feed_bytes']})
(root/'derived.json').write_text(json.dumps(summary,indent=2)+'\n')
for stage,rs in results.items():
    print('\n'+stage)
    for mode in dict.fromkeys(r['mode'] for r in rs.values()):
        rows=[r for r in rs.values() if r['mode']==mode]
        print(mode,'write','; '.join(f'{r["write_rate"]:,.0f}' for r in rows),'read','; '.join(f'{r["read_rate"]:,.0f}' for r in rows),'rss MiB','; '.join(f'{r["max_rss_bytes"]/2**20:.2f}' for r in rows))
print('\nratios',json.dumps(summary['main_ratios'],indent=2),json.dumps(summary['ablation_ratios'],indent=2))
