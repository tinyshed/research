#!/usr/bin/env python3
"""Render the dated report from retained raw results; never run a benchmark."""
import json,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parent
RAW=ROOT.parent/'reports/data/metrics-layout-2026-10-09'
REPORT=ROOT.parent/'reports/metrics-layout-2026-10-09.md'
def read(name):return json.loads((RAW/name).read_text())
def result(dataset,variant):
 x=read(f'{dataset}-{variant}.json');return x['census']['result'] if variant=='baseline' else x['result']
def size(dataset,variant):return result(dataset,variant)['file_bytes']
def obj(c,name,key='bytes'):return sum(x[key] for x in c['physical']['objects'] if x['name']==name)
def table(headers,rows):return '\n'.join(['| '+' | '.join(headers)+' |','| '+' | '.join(['---']*len(headers))+' |']+['| '+' | '.join(str(v) for v in r)+' |' for r in rows])
def number(n):return f'{n:,}'
def med(rows,field):return statistics.median(field(x) for x in rows)
def main():
 p=read('provenance.json');timing=read('timings.json');retention=read('retention.json');publication=read('publication.json');rss=read('rss.json');pilots=read('pilots.json');cross=read('go-crossread.json')
 datasets=['tsbs','alibaba','regular','irregular','nonsparse','edge'];variants=['exact','rowid','page1024','page2048','native_control','cap8','cap16','inline0','inline32','inline64','inline128','rowid_native_control','rowid_inline0','rowid_inline32','rowid_inline64','rowid_inline128','rowid_page1024','rowid_page2048','rowid_cap16','rowid_cap16_inline64','rowid_cap16_inline128']
 baseline=[]
 for d in datasets:
  x=read(f'{d}-baseline.json');c=x['census']['result'];n=x['manifest']['sample_count'];baseline.append([d,number(n),number(c['blobs']['head_samples']),number(c['file_bytes']),f"{c['file_bytes']/n:.6f}",number(c['value_bytes_total']),number(c['blobs']['directory_stored']),number(c['blobs']['clock_stored']),number(c['blobs']['head_stored'])])
 decomposition=[]
 for d in ['tsbs','alibaba']:
  for v in ['baseline','exact','rowid']:
   c=result(d,v);g=obj(c,'groups');gi=obj(c,'sqlite_autoindex_groups_1');payload=obj(c,'payloads');clocks=obj(c,'clocks');state=obj(c,'series_state');other=c['file_bytes']-g-gi-payload-clocks-state-c['physical']['freelist_bytes'];decomposition.append([d,v,*map(number,[g,gi,payload,clocks,state,other,c['physical']['freelist_bytes']])])
 matrix=[]
 for v in variants:
  matrix.append([v,*[number(size(d,v)) for d in ['tsbs','alibaba','irregular','nonsparse']]])
 summaries=[]
 for d in ['tsbs','alibaba']:
  c=result(d,'baseline');q=c['expanded_directory_parts'];summaries.append([d,*map(number,[q['first'],q['summary_scalars_flags_resets'],q['exact_summary'],q['value_length'],q['address_or_inline']-c['inline_value_bytes'],c['inline_value_bytes'],c['block_clock_residual_bytes']])])
 performance=[];passes=[]
 for d in ['tsbs','alibaba','irregular']:
  for case in ['point','range','whole_summary','cut_summary']:
   selected=[r for r in timing if r['dataset']==d and r['result']['case']==case];base=med([r for r in selected if r['variant']=='exact'],lambda r:r['result']['elapsed_ns']/r['result']['iterations'])
   ratios=[]
   for v in ['rowid','cap16','rowid_inline32','rowid_inline64']:
    rows=[r for r in selected if r['variant']==v];ratios.append(f"{med(rows,lambda r:r['result']['elapsed_ns']/r['result']['iterations'])/base:.3f}×")
   performance.append([d,case,selected[0]['result']['iterations'],f'{base/1000:.2f}',*ratios])
   if d=='alibaba':
    for v in ['exact','rowid','cap16','rowid_inline32']:
     rows=sorted([r for r in selected if r['variant']==v],key=lambda r:r['pass_no']);passes.append([case,v,'; '.join(f"{r['result']['elapsed_ns']/r['result']['iterations']/1000:.2f}" for r in rows)])
 lifecycle=[];retained=[]
 for d in ['tsbs','alibaba','irregular']:
  for v in ['exact','rowid','native_control','rowid_native_control','cap16','rowid_inline32','rowid_inline64']:
   pub=[r for r in publication if r['dataset']==d and r['variant']==v];ret=[r for r in retention if r['dataset']==d and r['variant']==v];lifecycle.append([d,v,'; '.join(f"{r['result']['publication_ns']/1e6:.1f}" for r in sorted(pub,key=lambda r:r['pass_no'])),'; '.join(f"{r['result']['retention']['elapsed_ns']/1e6:.1f}" for r in sorted(ret,key=lambda r:r['pass_no']))])
   first=next(r for r in ret if r['pass_no']==0);c=first['result']['physical'];vac=first['vacuum']['result'];retained.append([d,v,number(size(d,v)),number(c['file_bytes']),number(c['physical']['freelist_bytes']),number(vac['file_bytes']),number(first['result']['wal_before_checkpoint']),number(first['result']['retention']['expired_samples'])])
 peak=[]
 for d in ['tsbs','alibaba','irregular']:
  for v in ['exact','rowid','cap16','rowid_inline32','rowid_inline64']:
   rows=[r for r in rss if r['dataset']==d and r['variant']==v];peak.append([d,v,*[next(r['peak_rss_kib'] for r in rows if r['result']['case']==case) for case in ['point','range','whole_summary','cut_summary']]])
 times=sorted(r['utc'] for r in timing)
 text=f'''# Current metrics group, directory, inline and page layout — 9 October 2026

This exploratory round measures the current published engine, TinyStore
`e307c48a40126aad0e2873b6bf3aaedef8115483`. Its existing format already has
WITHOUT ROWID groups, 32 slots of up to 240 samples, 16-byte inline values,
shared clocks and v4 directories with exact summaries and predicted fields.
Those features are the baseline, not new proposals. No production code or
migration changed, and no historical prototype density is used as a comparator.

| Question | Answer | What follows |
| --- | --- | --- |
| What does the current engine actually store? | TSBS: 4,665,344 bytes / 5,090,400 samples = 0.916499 B/sample. Alibaba: 7,843,840 / 12,431,885 = 0.630945. Both include durable heads, registry and every index. | Start from the complete contemporary file, not an old value-codec figure. |
| Does replacing WITHOUT ROWID groups help? | With original directory/clock/value bytes, Alibaba's freshly rebuilt control falls from 6,774,784 to 5,324,800 bytes, 21.40%; TSBS grows 0.45%. | A real corpus-dependent layout opportunity, with the UNIQUE index charged. |
| Is a larger inline threshold automatically better? | No. Rowid + inline32 reaches 5,230,592 bytes on Alibaba; 64/128 are larger there. Rowid + inline64 wins the deterministic irregular fixture. | Treat thresholds and row layout together. |
| Do smaller pages or groups win everywhere? | No. 4-KiB pages remain best on both public corpora for the rowid layouts; cap16 is a competitive Alibaba alternative but costs more clocks/groups. | Keep measured counterexamples and query/maintenance costs beside density. |
| Is the original-file-to-fresh-control saving a format gain? | No. Alibaba's unchanged head/state rows occupy 1,253,376 bytes originally and 188,416 after fresh insertion; both files have zero freelist pages. | Separate insertion/occupancy history from layout savings. |

## Environment and reproduction

Linux amd64 in Docker Desktop / WSL2 on an AMD Ryzen 7 7700, 16 logical CPUs
visible, cgroup `cpu.max=max 100000`; no CPU affinity or quota reduction.
Go 1.27.1; Rust 1.99.0 / LLVM 23.1.1; Python {p['python']}.
The engine uses stock rusqlite 0.40.1 against external static SQLite 3.53.4
(the matched original archive), FULL WAL, 1-MiB SQLite cache, foreign keys,
fullfsync and checkpoint_fullfsync enabled, auto-checkpoint 1,000 pages,
trusted_schema off and mmap off. Metadata uses zstd 0.13.3 / zstd-sys
2.1.0 + zstd 1.5.7. Native value compression is never invoked for a variant.

Raw provenance contains complete source hashes and compiler/archive settings:
[phase 1](data/metrics-layout-2026-10-09/phase1-provenance.json),
[phase 2](data/metrics-layout-2026-10-09/phase2-provenance.json), and
[final performance binary](data/metrics-layout-2026-10-09/provenance.json).
Go fixture binary SHA-256: `{p['go_binary_sha256']}`.
Final native binary SHA-256: `{p['rust_binary_sha256']}`.
The user waived the premeasurement harness-commit rule; this round is explicitly
exploratory and retains exact source/binary hashes rather than citing a
premeasurement harness commit.

Measurements ran serially with the companion payload experiment paused.
Read passes ran from {times[0]} to {times[-1]}; each cell has six passes,
forward/reverse variant order alternating. Handles are new each pass; the OS
file cache is warm, not a cold-device test. Synchronous local ratios are not
bare-Linux latency or concurrent production-throughput promises.

Reproduce using [the harness commands](../metrics-layout-bench/README.md),
placing stores and corpora on a Linux volume. Prepare the normalised inputs as
`<work>/metrics-storage-input/corpus/{{tsbs,alibaba}}-series.jsonl`, then run
`baseline`, the 21-variant `matrix`, `pilot`, `timing`, `publication`,
`retention`, `rss`, `crossread` and `provenance`. Use `--datasets tsbs alibaba
irregular` and `--variants exact rowid native_control rowid_native_control
cap16 rowid_inline32 rowid_inline64` for the performance stages. The baseline
and density matrix also include regular, nonsparse and IEEE-edge fixtures.

TSBS DevOps uses generator revision
`8323e59c74027b108f4ad5ec5d3e498b0101a02e`, seed 123, scale 20, ten-second
intervals from 2026-01-01T00:00:00Z through 07:00:00Z: 2,020 series,
5,090,400 samples, normalised SHA-256
`e4f502af7b0b2ff2c4dba92057a8f2b95e636882f3cb9900986e013d189572cf`.
Alibaba uses the official public 2018 machine trace, every sixteenth machine,
seconds strictly before 172,800: 248 machines, 1,240 series, 12,431,885 samples,
no duplicate timestamps. Normalised SHA-256
`7ace58aaba7b3c34efc34c41585014521121c76aa991646c30735127a619afdd`;
[archive, selector and converter provenance](data/metrics-layout-2026-10-09/alibaba-corpus-provenance.json).
Neither corpus is committed. All corpus metrics are gauges; deterministic
fixtures also include counters and IEEE edge patterns.

## Actual production baseline first

The public Go API ingests one series at a time in batches of at most 7,680,
maintains after each batch and to quiescence, then closes. The clock is fixed
after the fixture's newest time. No wall-clock operation forces a quiet head
to seal. Go verifies every timestamp/value bit through `metrics.Read`; native
decoding then independently matches those per-series SHA-256 hashes.

{table(['Fixture','All samples','Head samples','Closed file B','File B/sample','Sealed value body B','Stored directory B','Unique clock B','Head B'],baseline)}

Sealed values include their original per-block CRCs and inline bytes; they
exclude first values kept in directory fields. Stored clock bytes are unique
shared envelopes. Per-block residual clocks below count their logical copies,
so they must not be added to unique clock bytes. Expanded directory fields
describe the precompression representation and must not be added to stored
directory bytes:

{table(['Fixture','First-value field B','Summary scalar/flags/resets B','Exact sum/increase B','Value-length B','Payload-address B','Inline value B','Logical clock residual B'],summaries)}

Physical `dbstat` cell payload includes record headers and scalar columns as
well as BLOBs. Its btree/index/overflow pages are a separate accounting view.
For every file, object page bytes plus freelist bytes equal the whole file,
with no overlap. Python SQLite {p['sqlite_scan_version']} reads `dbstat` through
an immutable, read-only connection because the matched engine archive omits
the virtual table. Production `internal/dbstat` independently agrees with
every object's page total. Python SQLite never writes or times the engine.

{table(['Fixture','Layout','Groups B','Groups UNIQUE index B','Payloads B','Clocks B','Head/state B','Other objects B','Freelist B'],decomposition)}

The original-to-fresh-control delta is insertion history: TSBS head/state
falls 126,976 bytes, Alibaba falls 1,064,960; minor registry/index differences
account for the rest. Head BLOB bytes and every logical state row remain
identical. This is not a retention deletion or a freelist reclamation, and it
is excluded from the layout percentage comparisons.

## The overflow cost is in the groups object

Alibaba's production groups object uses 3,063,808 bytes, including 1,904,640
bytes of overflow pages. Those overflow pages contain 312,228 payload bytes
and 1,590,552 unused bytes. The byte-preserving rowid version reduces the
groups object to 1,560,576 bytes and adds a 53,248-byte UNIQUE index; all other
charged objects remain the same as the fresh control. Its net saving is
1,449,984 bytes. TSBS has no groups overflow: its groups object shrinks
24,576 bytes while its new UNIQUE index costs 45,056, a 20,480-byte loss.
The measured object division establishes the cause; BLOB length alone would
miss the page-local payload/overflow boundary.

## Staged density matrix

`exact`, `rowid` and page-only variants copy original directories, clocks and
value bodies byte for byte. `native_control` re-encodes only group envelopes
using pinned native metadata compression. It saves 8,192 bytes on each public
corpus; that compressor effect is not attributed to a layout. The matching
rowid native control is also retained. Caps split existing groups without
joining unrelated original boundaries; no value block is rebuilt. Thresholds
other than 16 use a self-identifying experimental v5 directory with a stored
u16 threshold and a distinct decoder. Caps stay at 8/16/32; 64 would require
a separate mask/format design and was not implemented.

Every variant verifies immutable block head/time/value/summary bytes by a
canonical SHA-256, confirms surviving external payload IDs never move, and
matches all samples including durable heads. Registry, head/state and migration
rows are copied verbatim, with two-way SQL EXCEPT verification in the final
publication runs. Extra rowid indexes are always charged.

{table(['Variant','TSBS file B','Alibaba file B','Irregular file B','Nonsparse file B'],matrix)}

The strongest Alibaba combination is rowid/32 slots/inline32/4-KiB pages:
5,230,592 bytes, 0.420740 B/sample. Against the matching native WITHOUT ROWID
control (6,766,592 bytes), the layout gain is 22.70%; against the matching
native rowid/inline16 control (5,312,512), inline32 alone saves 81,920 bytes,
1.54%. It moves 5,105 blocks / 103,946 body bytes inline: payload pages fall
151,552 bytes and group pages grow 69,632. TSBS has no bodies in the 17–32-byte
interval, so the same change only charges the v5 header.

Larger thresholds are counterexamples: WITHOUT ROWID inline128 reaches
7,200,768 bytes on TSBS and 8,515,584 on Alibaba. Rowid avoids much of that
overflow but 64/128 still lose to 32 on Alibaba. Irregular clocks favour
rowid/inline64 instead (430,080 versus 520,192 fresh-control bytes). Small-page
wins in tiny constant/edge fixtures largely remove minimum root-page cost;
they do not justify changing the public-corpus default.

## Six balanced read passes

The point trace reads an existing first sample, rotating series without loading
the corpus. The range covers 1/16 of one series; the cut sum covers its middle
third. Whole sums use exact summaries where available and raw heads; cuts use
raw samples at block boundaries. Selected external bodies are fetched before
the snapshot ends; decoding and answer construction occur afterwards. Every
variant/pass yields the same trace hash. Raw-versus-summary checks cover the
first 16 series of each finite fixture; full sample checks cover every series.

A 32-operation pilot runs every variant; the slowest sets one shared count
per fixture/case, target 120 ms and maximum 4,096 operations. Projected read
time was {pilots['projected_read_seconds']:.1f} seconds. Counts and all pilot
values are in [pilots.json](data/metrics-layout-2026-10-09/pilots.json).
Ratios below are median time / median exact-control time, so below 1 is faster:

{table(['Fixture','Trace','Ops/pass','Exact median µs/op','Rowid ratio','Cap16 ratio','Rowid inline32 ratio','Rowid inline64 ratio'],performance)}

Alibaba's actual six pass observations (µs/op, in pass order) make short-run
variation visible; the complete seven-variant observations are retained in
[timings.json](data/metrics-layout-2026-10-09/timings.json):

{table(['Trace','Variant','Six passes µs/op'],passes)}

## Publication and partial retention

Publication is a native transaction creating the complete schema, copying the
same registry/state rows and publishing the immutable blocks, followed by
`wal_checkpoint(TRUNCATE)`. Its timer excludes input preparation and postwrite
proofs. It is a prototype fresh-publication cost, not production Go ingest:
byte-copy `exact`/`rowid` use SQLite `INSERT SELECT`; re-encoded variants also
pay metadata/envelope publication work. Compare those with native controls.
Retention includes metadata fetch, liveness updates, payload deletion, clock
reference release and the FULL commit. It excludes independent visible-hash
validation and the subsequent checkpoint. Six balanced milliseconds follow:

{table(['Fixture','Variant','Six publication ms','Six retention ms'],lifecycle)}

The cutoff is 60% through the fixture's overall time interval. Only blocks
whose last sample is strictly below it are deleted; straddling blocks retain
their original bodies and summaries. Equal-to-cutoff blocks stay live.
Every retention pass verifies clipped visible samples before/after and rejects
any remaining live expired block. Clock refs and surviving payload IDs remain
owned by their original groups; no payload body is moved during retention.

The initial and retained files below are closed, checkpointed files; WAL peak
is the WAL observed after the retention FULL commit and before the explicit
TRUNCATE checkpoint. All post-checkpoint WAL byte counts are zero. Transient
SHM is not charged as persistent database storage. `VACUUM` is a separate
postmeasurement operation, never part of the retention timing:

{table(['Fixture','Variant','Before file B','After retention B','Freelist B','After VACUUM B','WAL before checkpoint B','Expired samples'],retained)}

Retained-file growth/free space and vacuum-compacted density are separate
states. This round does not simulate indefinite append/expire churn or claim
that deleting rows shrinks a file; a production candidate still needs that
lifecycle gate under the real indexed maintenance scheduler.

## Correctness, memory and limits

The final Linux Rust suite passes 30 tests, including independent production
Go manifest readback, IEEE payloads, -0, all residual/clock/envelope modes,
bounded decompression, truncations and checksum guards, and v5 decoder
identification. Additional public Go cross-reads pass {len(cross)} closed v4
files across TSBS, Alibaba and irregular fixtures (exact, rowid, cap16,
page1024 and native metadata control). Experimental inline v5 intentionally
requires its own decoder; it is not silently passed to the production engine.

Peak process RSS (KiB), GNU time, separate 512-operation processes, includes
native SQLite/zstd and allocator memory; these observations are not used for
latency comparisons:

{table(['Fixture','Variant','Point RSS','Range RSS','Whole sum RSS','Cut sum RSS'],peak)}

The synchronous research harness omits production admission, cancellation,
connection pools, directory locks, background work and maintenance scheduling.
It does not change value codecs, block sample counts, exact summary precision,
retention visibility, or payload address rules. No TinyStore source/format
migration, commit or release was made in this round.

## What follows

Carry the rowid groups alternative to a production-shaped candidate: preserve
the UNIQUE(series_id,start_ts) lookup, byte-exact v4 directories, CRCs and
stable payload IDs, then rerun the real public API query/admission and indexed
retention/merge tests. Keep the TSBS regression visible. Inline32 is a small
additional Alibaba win after rowid and needs an explicit new directory format;
it is not a universal threshold recommendation. Cap16 remains an alternate
tradeoff worth judging against its read/maintenance rows. Keep 4-KiB pages for
the public-corpus control. Do not combine these percentages with the companion
payload study without measuring the combined file and lifecycle.

Full raw data, exact source hashes, six-pass values, per-object page types,
overflow, unused bytes, input manifests and file hashes are in
[data/metrics-layout-2026-10-09](data/metrics-layout-2026-10-09/).
'''
 REPORT.write_text(text)
if __name__=='__main__':main()
