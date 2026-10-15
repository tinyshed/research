#!/usr/bin/env python3
"""Separate observer-instrumented allocation cohort; never merge its timings."""
import run
for passno in (1,2):
    for kind,shape in [('fts-set','russian'),('tantivy','russian'),('geo','uniform'),('rstar','uniform'),('roaring','uniform')]:
        n=100000
        if kind=='tantivy':args=['tantivy',n,shape]
        elif kind in ('rstar','roaring'):args=['alternative',kind,n,shape]
        else:args=['native',kind,n,shape,'ordered',100]
        run.save('allocations',kind,passno,run.prepared(args,'source',n,shape))
for shape in ('russian','uniform'):run.tidy(run.SCRATCH/f'template-source-100000-{shape}-ordered.db')
