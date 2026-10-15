#!/usr/bin/env python3
"""Small matched-query A/B, serially after other benchmarks stop."""
import run
for n in (100000,1000000):
    for shape in ('uniform','clustered'):
        for passno in (1,2):
            run.save('geo-certified',f'{n}-{shape}',passno,
                run.prepared(['geo-probe',n,shape],'geo',n,shape))
        run.tidy(run.SCRATCH/f'template-geo-{n}-{shape}-ordered.db')
