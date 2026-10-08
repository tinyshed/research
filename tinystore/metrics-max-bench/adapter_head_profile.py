#!/usr/bin/env python3
"""Diagnose the isolated head/lookaside interaction outside latency trials."""
import datetime as dt
import json
from pathlib import Path
import adapter_run as engine

def main():
    rows=[]
    started=dt.datetime.now(dt.timezone.utc).isoformat()
    for pass_id in range(3):
        for variant in ['control','metadata','lookaside_512','both_lookaside_512']:
            row=engine.run_one(variant,'read_head_full8','profile',256,64)
            row.update(variant=variant,pass_id=pass_id+1);rows.append(row)
    target=Path(__file__).resolve().parent.parent/'reports/data/sqlite-adapter-2026-10-08/head-profiles.json'
    target.write_text(json.dumps({'started_utc':started,'finished_utc':dt.datetime.now(dt.timezone.utc).isoformat(),
        'note':'Instrumented inclusive wall-time diagnostics; not uninstrumented latency comparisons','rows':rows},indent=2)+'\n')

if __name__=='__main__':main()
