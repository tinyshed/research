#!/usr/bin/env python3
"""Standalone descriptive figure; speedup is the ratio of five-run medians."""
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT=Path(__file__).resolve().parent
DATA=ROOT.parent/'reports/data/metrics-native-2026-10-08'
rows=json.loads((DATA/'summary.json').read_text())
colors=['#b77c20' if r['case'].startswith('ingest_') else '#64748b' if r['case'] in ['maintain_ready','expire_all'] else '#246b8e' if not r['case'].startswith('aggregate_') else '#408657' for r in rows]
fig,ax=plt.subplots(figsize=(10,11))
ax.barh(range(len(rows)),[r['go_over_native']for r in rows],color=colors,height=.74)
ax.set_yticks(range(len(rows)),[r['case']for r in rows],fontsize=8)
ax.invert_yaxis();ax.axvline(1,color='#333333',linewidth=1)
ax.set_xlim(0,3.9);ax.set_xlabel('Go time / Rust-native time — higher is faster')
for i,r in enumerate(rows):ax.text(r['go_over_native']+.04,i,f"{r['go_over_native']:.2f}×",va='center',fontsize=8)
ax.set_title('TinyStore metrics: complete synchronous paths\nSame fixed traces, 5 paired passes, SQLite 3.53.4 WAL/FULL',fontsize=12)
ax.spines[['right','top']].set_visible(False)
ax.grid(axis='x',alpha=.2);ax.set_axisbelow(True)
fig.text(.02,.01,'Local prototype • warm synthetic fixtures • one reader/writer • native compression and wrapper also differ',fontsize=8)
fig.tight_layout(rect=(0,.025,1,1))
fig.savefig(DATA/'speedup.svg',metadata={'Date':None})
fig.savefig(DATA/'speedup.png',dpi=140)
plt.close(fig)
