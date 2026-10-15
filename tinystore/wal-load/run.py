"""Sequential prebuilt-process runner. No build/test/lint during measurement."""
from pathlib import Path
import subprocess,sys,json,datetime
root=Path(__file__).resolve().parent
stage=sys.argv[1];out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=True)
if stage=='main': modes=['off','legacy','optimized'];seconds=60;passes=3
elif stage=='ablation':modes=['bounded','hash','copy','combined','optimized','nocache','full'];seconds=20;passes=3
elif stage=='soak':modes=['optimized'];seconds=180;passes=1
elif stage=='preflight':modes=['optimized','legacy','off','full'];seconds=3;passes=1
else:raise SystemExit('stage: preflight|main|ablation|soak')
for passed in range(1,passes+1):
    shift=(passed-1)*(2 if stage=='ablation' else 1)%len(modes)
    for mode in modes[shift:]+modes[:shift]:
        label=f'{stage}-p{passed}-{mode}'
        start=datetime.datetime.now(datetime.timezone.utc).isoformat()
        print(f'start {label} {start}',flush=True)
        with (out/f'{label}.jsonl').open('w') as stdout,(out/f'{label}.stderr').open('w') as stderr:
            result=subprocess.run([str(root/'target/release/tinystore-wal-load'),mode,str(seconds),'100'],cwd=root,stdout=stdout,stderr=stderr)
        end=datetime.datetime.now(datetime.timezone.utc).isoformat()
        rows=[json.loads(line) for line in (out/f'{label}.jsonl').read_text().splitlines()]
        final=[row for row in rows if row['case']=='result']
        meta={'label':label,'stage':stage,'pass':passed,'mode':mode,'start_utc':start,'end_utc':end,'exit':result.returncode}
        (out/f'{label}.meta.json').write_text(json.dumps(meta,indent=2)+'\n')
        if result.returncode or len(final)!=1 or not final[0]['verified']:raise SystemExit(f'failed {label}: inspect its stderr')
        r=final[0]
        print(f'done {label} writes/s={r["write_rate"]:.0f} reads/s={r["read_rate"]:.0f} rss={r["max_rss_bytes"]} denied={r["denied"]}',flush=True)
