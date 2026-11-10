"""Round 1v: a schema's own valley as its threshold, tried on a frozen model's kept scores.

    python -I valley.py [<model>]

A schema named in a style the model has not seen scores low as a whole, its
keys still above its other columns. Here a column is drawn when it passes
the model's threshold, or when it lies above the cut that parts the schema's
columns' best scores into two (wide.py `valley`), the cut being clear enough
and the score above a floor. Development sets only (dump.py's); prints F1
over every declared key a set for each setting. It lost on most sets and is
not carried; the same cut is one of the second stage's features in wide.py.
"""
import sys; sys.path.insert(0,'round-1v')
import numpy as np, whole, math
from wide import valley, logit
sets=['schemapile validation','schemapile validation, renamed','schemapile test','spider','bird','ctu','livesqlbench (development)','github 2025','github 2025, rest','github style','github fresh']
model=sys.argv[1] if len(sys.argv)>1 else 'nine-mended'
def run(sc, rule):
    out=[dict() for _ in sc.schemas]
    for si,(a,b) in sc.spans.items():
        best={}
        for i in range(a,b):
            k=(int(sc.table[i]),int(sc.column[i]))
            if k not in best or sc.second[i]>sc.second[best[k]]: best[k]=i
        vals=[float(sc.second[i]) for i in best.values()]
        cut,apart,above,width=valley(vals)
        s=sc.schemas[si]
        for (ti,ci),i in best.items():
            if rule(float(sc.second[i]),cut,apart,above,width,sc.threshold):
                t=s['tables'][ti]; out[si][(t['name'],t['columns'][ci]['name'])]=s['tables'][sc.target[i]]['name']
    return out
rules={
 'as read': lambda p,cut,ap,ab,w,T: p>=T,
 'valley, apart>=.6, floor .05': lambda p,cut,ap,ab,w,T: p>=T or (ap>=0.6 and logit(p)>cut and p>=0.05),
 'valley, apart>=.7, floor .1': lambda p,cut,ap,ab,w,T: p>=T or (ap>=0.7 and logit(p)>cut and p>=0.1),
 'valley, apart>=.8, floor .1': lambda p,cut,ap,ab,w,T: p>=T or (ap>=0.8 and logit(p)>cut and p>=0.1),
 'valley, apart>=.8, floor .3': lambda p,cut,ap,ab,w,T: p>=T or (ap>=0.8 and logit(p)>cut and p>=0.3),
 'valley, apart>=.8, above<=.4, floor .1': lambda p,cut,ap,ab,w,T: p>=T or (ap>=0.8 and ab<=0.4 and logit(p)>cut and p>=0.1),
 'T=0.5': lambda p,cut,ap,ab,w,T: p>=0.5,
}
print(model)
res={r:[] for r in rules}
for name in sets:
    sc=whole.Scored(model,name)
    for r,fn in rules.items():
        per=sc.counts(run(sc,fn)); m=whole.measure(per,sc.tables)
        res[r].append((name,m))
for r in rules:
    print(f'{r:40}'+' '.join(f"{m['f1']:.3f}" for _,m in res[r]))
print(' '*40+' '.join(n[:5] for n in ['spval','spvRn','sptst','spidr','bird ','ctu  ','lsb  ','gh25 ','ghRst','ghSty','ghFre']))
