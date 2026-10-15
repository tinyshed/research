"""Render frozen JSONL with matplotlib 3.10.8; run beside these inputs."""
from pathlib import Path
import json
import statistics
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent
plt.rcParams.update({'font.size': 10, 'svg.hashsalt': 'tinystore-wal-2026-10-15'})
capture = [json.loads(line) for line in (ROOT/'capture.jsonl').read_text().splitlines()][1:]
pipeline = [json.loads(line) for line in (ROOT/'pipeline.jsonl').read_text().splitlines()]
fig, axes = plt.subplots(1, 2, figsize=(11.5, 4.2))
colors = ['#768391', '#c77b32', '#7c68b5', '#158f86']
for offset, (mode, color) in enumerate(zip(['off', 'snapshot', 'full', 'guarded'], colors)):
    for index, rows in enumerate([4096, 32768]):
        values = [r['p50_us']/1000 for r in capture if r['workers']==1 and r['rows']==rows and r['mode']==mode]
        mid = statistics.median(values)
        axes[0].errorbar(index+(offset-1.5)*0.16, mid, yerr=[[mid-min(values)],[max(values)-mid]],
                         fmt='o', capsize=4, color=color, label=mode if index==0 else None)
axes[0].set(yscale='log', xticks=[0,1], xticklabels=['1.13 MB','8.98 MB'],
            ylabel='Write + capture p50 (ms, log scale)', xlabel='Source database size',
            title='One caller; one dirty page / commit')
axes[0].legend(frameon=False, ncols=2)
axes[0].grid(axis='y', alpha=.2)
for index, (forward, background, color) in enumerate([('after_sync',False,'#768391'),('after_sync',True,'#7c68b5'),('after_append',True,'#158f86')]):
    values = [r['ship_apply_batch_p50_us']/1000 for r in pipeline if r['batch']==1 and r['injected_one_way_delay_ms']==0 and r['forward']==forward and r['background_applier']==background]
    mid = statistics.median(values)
    axes[1].bar(index, mid, color=color, width=.57)
    axes[1].errorbar(index, mid, yerr=[[mid-min(values)],[max(values)-mid]],fmt='none',color='#243140',capsize=5)
    axes[1].text(index, max(values)+.07, f'{mid:.2f}', ha='center', va='bottom')
echo = statistics.median([r['echo_p50_us']/1000 for r in pipeline if r['batch']==1 and r['injected_one_way_delay_ms']==0 and r['forward']=='after_append'])
axes[1].axhline(echo, color='#bd4c4c', linestyle='--', label=f'Echo: {echo:.3f} ms')
axes[1].set(xticks=[0,1,2], xticklabels=['Serial','Separate apply','Early forward'],
            ylabel='Durable receipt p50 (ms)', ylim=(0,3.4), title='3 TCP hops; loopback; one commit / batch')
axes[1].legend(frameon=False)
axes[1].grid(axis='y',alpha=.2)
fig.suptitle('TinyStore WAL research — three rotated passes, SQLite FULL', fontsize=13)
fig.text(.5,.015,'Dots/bars: median of pass p50s. Whiskers: min–max, not confidence intervals. Guarded: 256 KiB policy.',ha='center',fontsize=8)
fig.tight_layout(rect=(0,.045,1,.95))
fig.savefig(ROOT/'comparison.svg',metadata={'Date':None})
fig.savefig(ROOT/'comparison.png',dpi=170)

# Normalize generated SVG whitespace for clean repository diffs.
svg = ROOT/"comparison.svg"
svg.write_text("\n".join(line.rstrip() for line in svg.read_text().splitlines())+"\n")
