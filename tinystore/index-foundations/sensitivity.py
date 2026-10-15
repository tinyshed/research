#!/usr/bin/env python3
"""Additional questions: exact WAL pages, cache/order, full-duration writer load."""
import os
from pathlib import Path
import shutil
import sys
import run

def case(args,template,env,label,passno):
    db=run.SCRATCH/'sensitivity.db';run.tidy(db);shutil.copyfile(template,db)
    original=run.BASE_ENV.copy()
    try:
        run.BASE_ENV.update(env)
        r=run.command([*args,'prepared'],db);r['experimental_env']={k:('<scratch>/sqlite-temp' if k=='SQLITE_TMPDIR' else v) for k,v in env.items()}
        run.save('sensitivity',label,passno,r)
    finally:
        run.BASE_ENV.clear();run.BASE_ENV.update(original);run.tidy(db)

if sys.argv[1]=='wal':
    temp=run.SCRATCH/'sqlite-temp';temp.mkdir(exist_ok=True)
    for n in (100000,1000000):
        for shape in ('uniform','clustered'):
            template=run.template('source',n,shape)
            for passno in (1,2):
                for cache in (1024,4096,65536):
                    for order in ('rowid','spatial'):
                        env={'BENCH_CACHE_KIB':str(cache),'BENCH_PAGE_DIAGNOSTICS':'1','BENCH_BUILD_ONLY':'1','BENCH_SQL_BULK':'1','BENCH_BUILD_ORDER':order,'SQLITE_TMPDIR':str(temp)}
                        case(['native','geo',n,shape,'ordered',100],template,env,f'wal-{n}-{shape}-cache{cache}-{order}',passno)
            run.tidy(template)
        template=run.template('source',n,'russian')
        for passno in (1,2):
            for cache in (1024,4096):
                env={'BENCH_CACHE_KIB':str(cache),'BENCH_PAGE_DIAGNOSTICS':'1','BENCH_BUILD_ONLY':'1'}
                case(['native','fts-set',n,'russian','ordered',100],template,env,f'wal-{n}-russian-fts-set-cache{cache}',passno)
        run.tidy(template)
    # Distinguish physical source insertion order from the index's scan order.
    for n in (100000,1000000):
        for passno in (1,2):
            for source_order in ('ordered','shuffled'):
                template=run.template('source',n,'clustered',source_order)
                env={'BENCH_CACHE_KIB':'1024','BENCH_PAGE_DIAGNOSTICS':'1','BENCH_BUILD_ONLY':'1','BENCH_SQL_BULK':'1','BENCH_BUILD_ORDER':'rowid'}
                case(['native','geo',n,'clustered',source_order,100],template,env,f'source-order-{n}-{source_order}',passno)
        for order in ('ordered','shuffled'):run.tidy(run.SCRATCH/f'template-source-{n}-clustered-{order}.db')
elif sys.argv[1]=='continuous':
    for n in (100000,1000000):
        for kind,shape in [('fts','russian'),('geo','clustered')]:
            template=run.template(kind,n,shape)
            for passno in (1,2):
                for cache in (1024,4096):
                    for algorithm in ('none','frontier','delta'):
                        env={'BENCH_CONTINUOUS':'1','BENCH_CACHE_KIB':str(cache)}
                        case(['rebuild',kind,algorithm,n,shape,2000,64,'selective',1,10,2000],template,env,f'continuous-{n}-{kind}-{algorithm}-cache{cache}',passno)
            run.tidy(template)
else:raise SystemExit('wal | continuous')
