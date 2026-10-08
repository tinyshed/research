#!/usr/bin/env python3
"""Render every paired pass and the wrapper comparisons from retained JSON."""
import json
from pathlib import Path
import statistics

ROOT=Path(__file__).resolve().parent.parent/'reports/data'

def main():
    for name in ['sqlite-adapter-2026-10-08','rusqlite-fork-2026-10-08']:
        directory=ROOT/name
        summary=json.loads((directory/'summary.json').read_text())
        variants=list(summary[0]['medians_ns'])
        # The lookaside followup appears on one case only; include all keys.
        variants=list(dict.fromkeys(v for r in summary for v in r['medians_ns']))
        lines=['# Complete-operation passes','', 'Microseconds per operation; six same-session passes in each cell.','',
            '| Case | '+' | '.join(variants)+' |', '| --- | '+' | '.join('---' for _ in variants)+' |']
        for row in summary:
            cells=['; '.join(f'{n/1000:.2f}' for n in row['passes_ns'].get(v,[])) or '—' for v in variants]
            lines.append('| '+row['case']+' | '+' | '.join(cells)+' |')
        (directory/'passes.md').write_text('\n'.join(lines)+'\n')
        wrappers=json.loads((directory/'wrappers.json').read_text())
        builds=list(dict.fromkeys(row.get('variant','stock') for row in wrappers))
        rows=[]
        for build in builds:
            for limit in [1,64,240]:
                grouped={method:[row['ns_per_op'] for row in wrappers if row['case']==f'{method}_{limit}' and row.get('variant','stock')==build]
                    for method in ['safe','raw']}
                safe,raw=(statistics.median(grouped[method]) for method in ['safe','raw'])
                rows.append({'variant':build,'rows':limit,'safe_median_ns':safe,'raw_median_ns':raw,
                    'safe_over_raw':safe/raw,'safe_passes_ns':grouped['safe'],'raw_passes_ns':grouped['raw']})
        (directory/'wrapper-summary.json').write_text(json.dumps(rows,indent=2)+'\n')

if __name__=='__main__':main()
