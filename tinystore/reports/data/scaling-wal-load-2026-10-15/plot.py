"""Publication plot: median and observed range, never a confidence interval."""
from pathlib import Path
import json,statistics
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
root=Path(__file__).resolve().parent
summary=json.loads((root/'derived.json').read_text())
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'svg.fonttype':'none','svg.hashsalt':'wal-load-20261015','axes.spines.top':False,'axes.spines.right':False})
fig,axes=plt.subplots(2,2,figsize=(13,7.7),layout='constrained')
colors={'off':'#6c757d','legacy':'#c04c36','optimized':'#187c98'}
labels={'off':'No capture','legacy':'Legacy','optimized':'Optimized'}
a=axes[0,0]
for i,mode in enumerate(colors):
    rs=[r for r in summary['results']['main'].values() if r['mode']==mode]
    ys=[r['write_rate']/1000 for r in rs];y=statistics.median(ys)
    a.bar(i,y,color=colors[mode],width=.6)
    a.errorbar(i,y,yerr=[[y-min(ys)],[max(ys)-y]],fmt='none',ecolor='#222222',capsize=6)
    a.text(i,max(ys)+.35,f'{min(ys):.2f}–{max(ys):.2f}',ha='center',fontsize=9)
a.set(xticks=range(3),xticklabels=list(labels.values()),ylabel='ACKed writes / second (thousands)',title='60 s × 3 passes: writes under mixed load',ylim=(0,16.5))
a.grid(axis='y',alpha=.15)
a=axes[0,1]
for mode,color in colors.items():
    for i,p in enumerate(sorted((root/'main').glob(f'*{mode}.jsonl'))):
        rs=[json.loads(l) for l in p.read_text().splitlines() if json.loads(l)['case']=='sample']
        a.plot([r['elapsed_seconds'] for r in rs],[r['rss_bytes']/2**20 for r in rs],color=color,alpha=.65,label=labels[mode] if i==0 else None)
p=next((root/'soak').glob('*.jsonl'));rs=[json.loads(l) for l in p.read_text().splitlines() if json.loads(l)['case']=='sample']
a.plot([r['elapsed_seconds'] for r in rs],[r['rss_bytes']/2**20 for r in rs],color='#187c98',linestyle='--',label='Optimized 180 s soak')
a.set(xlabel='Elapsed seconds',ylabel='Process RSS (MiB)',title='Resident history stops growing with the bounded cache')
a.grid(alpha=.15);a.legend(fontsize=8)
a=axes[1,0]
modes=['bounded','hash','copy','combined','optimized','nocache','full']
for i,mode in enumerate(modes):
    ys=[r['write_rate']/1000 for r in summary['results']['ablation'].values() if r['mode']==mode];y=statistics.median(ys)
    a.bar(i,y,color='#187c98' if mode!='full' else '#c04c36',width=.6)
    a.errorbar(i,y,yerr=[[y-min(ys)],[max(ys)-y]],fmt='none',ecolor='#222222',capsize=4)
a.set(xticks=range(len(modes)),xticklabels=modes,ylabel='ACKed writes / second (thousands)',title='20 s × 3 component controls: substantial variability',ylim=(0,14))
a.tick_params(axis='x',labelrotation=25);a.grid(axis='y',alpha=.15)
a=axes[1,1]
xs=[r['elapsed_seconds'] for r in rs]
a.plot(xs,[r['wal_bytes']/2**20 for r in rs],color='#b36f13',label='Native WAL (sampled)')
a.plot(xs,[r['cache_reserved']/2**20 for r in rs],color='#187c98',label='Cache reservation')
a.axhline(8,color='#666666',linestyle=':',label='8 MiB watermark / cache limit')
a.set(xlabel='Elapsed seconds',ylabel='MiB',ylim=(0,10),title='180 s soak: bounded RAM/WAL; feed keeps growing')
a.grid(alpha=.15)
b=a.twinx();b.plot(xs,[r['feed_bytes']/2**20 for r in rs],color='#864994',alpha=.7,label='Feed on disk');b.set_ylabel('Feed on disk (MiB)',color='#864994');b.spines['top'].set_visible(False)
h,l=a.get_legend_handles_labels();h2,l2=b.get_legend_handles_labels();a.legend(h+h2,l+l2,fontsize=8,loc='center left')
fig.suptitle('TinyStore WAL capture — Linux, 2 CPU quota, SQLite FULL\nSynthetic 64 writers + 8 readers + periodic 100 ms pinned snapshot',fontsize=14)
fig.savefig(root/'comparison.svg',metadata={'Date':None})
fig.savefig(root/'comparison.png',dpi=150,metadata={'Software':'matplotlib'})
p=root/'comparison.svg';p.write_text('\n'.join(l.rstrip() for l in p.read_text().splitlines())+'\n')
