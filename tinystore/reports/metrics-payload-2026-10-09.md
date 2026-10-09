# Immutable metrics payload packs and incremental BLOB reads — 2026-10-09

This exploratory round preserves the current production value bodies, shared clocks and version-4 exact-summary contents while changing external-body placement. It tests bounded immutable packs, charged compact addresses, whole-BLOB materialization and safe incremental BLOB access. Production TinyStore is unchanged. These are sealed payload query paths, not an end-to-end public API comparison.

| Question | Answer | What follows |
| --- | --- | --- |
| Best tested packed layout for tsbs? | `compact-count8`: 4,476,928 bytes; -0.55% versus fresh original 4,501,504. | Versus fresh original layout (4,501,504 bytes): -0.55%; row-layout gains are separate. |
| Best tested packed layout for alibaba? | `rowid-compact-count32`: 5,419,008 bytes; -19.96% versus fresh original 6,770,688. | Versus rowid groups without packs (5,320,704 bytes): +1.85%; row-layout gains are separate. |
| Does constant range-copy size prove selective physical I/O? | No. SQLite-cache misses and separate syscall diagnostics measure different work. | Cold here means the SQLite cache, with Linux page cache left alone. |
| Does retention recover every dead body immediately? | No. A pack remains until its last reference dies. | Retained dead bodies, refcounts, freelist and WAL are recorded separately. |

At 50% sealed-prefix expiry, TSBS compact-count8 retains 1,294,988 dead body bytes beside 1,299,371 live body bytes; Alibaba rowid-compact-count32 retains 1,303,151 dead beside 1,289,142 live bytes. These owned dead bytes are not reusable freelist pages, so fresh-file density alone does not establish retention-space savings.

## Format, baseline and invariant checks

TinyStore source is `e307c48a40126aad0e2873b6bf3aaedef8115483`. The original engine has 32-slot groups, blocks of at most 240 samples, compressed binary v4 directories with exact summaries, a WITHOUT ROWID groups table, shared clocks, inline bodies at most 16 bytes, and separately checked ROWID payload bodies. The fresh native control recreates the original DDL and inserts identical values. Registry, postings, mutable heads and state stay byte-identical in every candidate and remain charged in whole-file totals.

The `0x50/1` experimental directory envelope keeps the original v4 directory, then unique pack rowids and per-external-slot pack-index/offset/expected-CRC references. Its checksum binds the series and group bounds. There is no mapping row per block. Keeping the now-unused original payload-id bytes is conservative and charged. Each pack row also charges reference/live-byte counters and a checked 24-byte identity/length header. Packs never exceed 16 KiB or 32 bodies; each selected block keeps its original body/clock checksum and original decode bound. Byte targets permit one oversized individual body while preserving the 16 KiB hard cap.

All 120 density variants pass exhaustive whole/range encoded-body, clock and exact-summary replay. 8 additional full decoded replays match independently generated production-Go public read hashes and exact-summary byte hashes. Sixteen Rust tests pass, including selected-body corruption, metadata/address bounds, swapped address checks, immutable-address directory replacement, exact float edge cases, and a WAL reader surviving a concurrent writer's pack deletion. The reader closes its Blob and snapshot before decoding owned copied bytes. The inherited test requiring five historical fixture directories is excluded; this round's actual Go fixtures are checked explicitly.

The query paths fetch group/clock metadata, retrieve selected bodies in one snapshot, close all handles and end the snapshot, then decode or fold exact sum/count. Whole-body and whole-pack SQL use batched `json_each` joins. Incremental mode includes safe rusqlite Blob open/read/reopen/close costs. Explicit limits cover directory bytes, requested and copied external bytes, a five-second snapshot, and 100,000 decoded samples. Full-query registry matching, head retrieval, admission, cancellation and concurrent production reader pooling are outside the timings.

## Fresh whole-file density

Every file below is checkpointed, closed and has zero freelist pages after a fresh complete rebuild. System SQLite's read-only `dbstat` scan is outside all timings; the measured archive is not rebuilt with extra instrumentation. Original Go files are reported separately so insertion order/compaction cannot be credited to packs.

### tsbs: 5,090,400 samples

Original Go file: 4,665,344 bytes (0.916499 bytes/sample). Its physical object census is retained in `tsbs-original.json`.

| Layout | File bytes | Bytes/sample | Groups b-tree | Payload b-tree | Stored directories | Added directory bytes | Overflow pages |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 4,501,504 | 0.884312 | 520,192 | 3,076,096 | 417,158 | +0 | 0 |
| count1 | 4,972,544 | 0.976847 | 651,264 | 3,416,064 | 533,511 | +116,353 | 0 |
| count2 | 5,308,416 | 1.042829 | 634,880 | 3,768,320 | 523,211 | +106,053 | 0 |
| count4 | 4,558,848 | 0.895578 | 647,168 | 3,006,464 | 524,071 | +106,913 | 238 |
| count8 | 4,530,176 | 0.889945 | 643,072 | 2,981,888 | 524,537 | +107,379 | 314 |
| count16 | 4,558,848 | 0.895578 | 643,072 | 3,010,560 | 524,844 | +107,686 | 343 |
| count32 | 4,739,072 | 0.930982 | 643,072 | 3,190,784 | 524,743 | +107,585 | 362 |
| bytes1024 | 4,702,208 | 0.923740 | 647,168 | 3,149,824 | 526,095 | +108,937 | 0 |
| bytes2048 | 4,681,728 | 0.919717 | 647,168 | 3,129,344 | 525,794 | +108,636 | 0 |
| bytes4096 | 4,956,160 | 0.973629 | 643,072 | 3,407,872 | 525,035 | +107,877 | 30 |
| bytes8192 | 4,829,184 | 0.948685 | 643,072 | 3,280,896 | 524,840 | +107,682 | 267 |
| bytes16384 | 4,739,072 | 0.930982 | 643,072 | 3,190,784 | 524,743 | +107,585 | 362 |
| baseline-repeat | 4,501,504 | 0.884312 | 520,192 | 3,076,096 | 417,158 | +0 | 0 |
| compact-unpacked | 4,575,232 | 0.898796 | 593,920 | 3,076,096 | 484,305 | +67,147 | 0 |
| compact-count1 | 4,923,392 | 0.967192 | 602,112 | 3,416,064 | 484,604 | +67,446 | 0 |
| compact-count8 | 4,476,928 | 0.879485 | 589,824 | 2,981,888 | 475,483 | +58,325 | 314 |
| compact-count32 | 4,681,728 | 0.919717 | 585,728 | 3,190,784 | 475,414 | +58,256 | 362 |
| rowid-baseline | 4,521,984 | 0.888336 | 495,616 | 3,076,096 | 417,158 | +0 | 0 |
| rowid-compact-unpacked | 4,595,712 | 0.902819 | 569,344 | 3,076,096 | 484,305 | +67,147 | 0 |
| rowid-compact-count8 | 4,493,312 | 0.882703 | 561,152 | 2,981,888 | 475,483 | +58,325 | 314 |
| rowid-compact-count32 | 4,702,208 | 0.923740 | 561,152 | 3,190,784 | 475,414 | +58,256 | 362 |

### alibaba: 12,431,885 samples

Original Go file: 7,843,840 bytes (0.630945 bytes/sample). Its physical object census is retained in `alibaba-original.json`.

| Layout | File bytes | Bytes/sample | Groups b-tree | Payload b-tree | Stored directories | Added directory bytes | Overflow pages |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 6,770,688 | 0.544623 | 3,063,808 | 2,973,696 | 1,303,051 | +0 | 465 |
| count1 | 8,073,216 | 0.649396 | 3,391,488 | 3,948,544 | 1,620,766 | +317,715 | 470 |
| count2 | 7,450,624 | 0.599316 | 3,354,624 | 3,362,816 | 1,576,311 | +273,260 | 470 |
| count4 | 7,184,384 | 0.577900 | 3,346,432 | 3,104,768 | 1,570,599 | +267,548 | 470 |
| count8 | 7,184,384 | 0.577900 | 3,346,432 | 3,104,768 | 1,570,593 | +267,542 | 470 |
| count16 | 7,942,144 | 0.638853 | 3,354,624 | 3,854,336 | 1,571,563 | +268,512 | 470 |
| count32 | 6,946,816 | 0.558790 | 3,346,432 | 2,867,200 | 1,571,992 | +268,941 | 762 |
| bytes1024 | 6,946,816 | 0.558790 | 3,354,624 | 2,859,008 | 1,573,077 | +270,026 | 470 |
| bytes2048 | 6,959,104 | 0.559779 | 3,354,624 | 2,871,296 | 1,572,455 | +269,404 | 470 |
| bytes4096 | 7,163,904 | 0.576252 | 3,346,432 | 3,084,288 | 1,572,046 | +268,995 | 576 |
| bytes8192 | 6,946,816 | 0.558790 | 3,346,432 | 2,867,200 | 1,571,992 | +268,941 | 762 |
| bytes16384 | 6,946,816 | 0.558790 | 3,346,432 | 2,867,200 | 1,571,992 | +268,941 | 762 |
| baseline-repeat | 6,770,688 | 0.544623 | 3,063,808 | 2,973,696 | 1,303,051 | +0 | 465 |
| compact-unpacked | 6,995,968 | 0.562744 | 3,289,088 | 2,973,696 | 1,515,711 | +212,660 | 469 |
| compact-count1 | 7,979,008 | 0.641818 | 3,297,280 | 3,948,544 | 1,522,636 | +219,585 | 469 |
| compact-count8 | 7,094,272 | 0.570651 | 3,256,320 | 3,104,768 | 1,477,632 | +174,581 | 469 |
| compact-count32 | 6,860,800 | 0.551871 | 3,260,416 | 2,867,200 | 1,481,176 | +178,125 | 761 |
| rowid-baseline | 5,320,704 | 0.427989 | 1,560,576 | 2,973,696 | 1,303,051 | +0 | 0 |
| rowid-compact-unpacked | 5,578,752 | 0.448745 | 1,818,624 | 2,973,696 | 1,515,711 | +212,660 | 0 |
| rowid-compact-count8 | 5,656,576 | 0.455005 | 1,765,376 | 3,104,768 | 1,477,632 | +174,581 | 0 |
| rowid-compact-count32 | 5,419,008 | 0.435896 | 1,765,376 | 2,867,200 | 1,481,176 | +178,125 | 292 |

### regular: 491,552 samples

Original Go file: 86,016 bytes (0.174989 bytes/sample). Its physical object census is retained in `regular-original.json`.

| Layout | File bytes | Bytes/sample | Groups b-tree | Payload b-tree | Stored directories | Added directory bytes | Overflow pages |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 86,016 | 0.174989 | 20,480 | 4,096 | 10,294 | +0 | 0 |
| count1 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| count2 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| count4 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| count8 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| count16 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| count32 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| bytes1024 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| bytes2048 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| bytes4096 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| bytes8192 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| bytes16384 | 86,016 | 0.174989 | 20,480 | 4,096 | 11,222 | +928 | 0 |
| baseline-repeat | 86,016 | 0.174989 | 20,480 | 4,096 | 10,294 | +0 | 0 |
| compact-unpacked | 86,016 | 0.174989 | 20,480 | 4,096 | 10,380 | +86 | 0 |
| compact-count1 | 86,016 | 0.174989 | 20,480 | 4,096 | 10,380 | +86 | 0 |
| compact-count8 | 86,016 | 0.174989 | 20,480 | 4,096 | 10,380 | +86 | 0 |
| compact-count32 | 86,016 | 0.174989 | 20,480 | 4,096 | 10,380 | +86 | 0 |
| rowid-baseline | 90,112 | 0.183321 | 20,480 | 4,096 | 10,294 | +0 | 0 |
| rowid-compact-unpacked | 90,112 | 0.183321 | 20,480 | 4,096 | 10,380 | +86 | 0 |
| rowid-compact-count8 | 90,112 | 0.183321 | 20,480 | 4,096 | 10,380 | +86 | 0 |
| rowid-compact-count32 | 90,112 | 0.183321 | 20,480 | 4,096 | 10,380 | +86 | 0 |

### irregular: 491,552 samples

Original Go file: 544,768 bytes (1.108261 bytes/sample). Its physical object census is retained in `irregular-original.json`.

| Layout | File bytes | Bytes/sample | Groups b-tree | Payload b-tree | Stored directories | Added directory bytes | Overflow pages |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 520,192 | 1.058264 | 73,728 | 118,784 | 55,717 | +0 | 40 |
| count1 | 806,912 | 1.641560 | 303,104 | 176,128 | 72,902 | +17,185 | 103 |
| count2 | 774,144 | 1.574897 | 303,104 | 143,360 | 70,975 | +15,258 | 103 |
| count4 | 753,664 | 1.533234 | 303,104 | 122,880 | 70,455 | +14,738 | 103 |
| count8 | 753,664 | 1.533234 | 303,104 | 122,880 | 70,695 | +14,978 | 103 |
| count16 | 741,376 | 1.508235 | 303,104 | 110,592 | 70,791 | +15,074 | 103 |
| count32 | 765,952 | 1.558232 | 303,104 | 135,168 | 70,917 | +15,200 | 103 |
| bytes1024 | 770,048 | 1.566565 | 303,104 | 139,264 | 70,876 | +15,159 | 103 |
| bytes2048 | 765,952 | 1.558232 | 303,104 | 135,168 | 70,917 | +15,200 | 103 |
| bytes4096 | 765,952 | 1.558232 | 303,104 | 135,168 | 70,917 | +15,200 | 103 |
| bytes8192 | 765,952 | 1.558232 | 303,104 | 135,168 | 70,917 | +15,200 | 103 |
| bytes16384 | 765,952 | 1.558232 | 303,104 | 135,168 | 70,917 | +15,200 | 103 |
| baseline-repeat | 520,192 | 1.058264 | 73,728 | 118,784 | 55,717 | +0 | 40 |
| compact-unpacked | 708,608 | 1.441573 | 262,144 | 118,784 | 67,373 | +11,656 | 92 |
| compact-count1 | 794,624 | 1.616561 | 290,816 | 176,128 | 67,787 | +12,070 | 98 |
| compact-count8 | 626,688 | 1.274917 | 176,128 | 122,880 | 65,089 | +9,372 | 65 |
| compact-count32 | 655,360 | 1.333247 | 192,512 | 135,168 | 65,574 | +9,857 | 69 |
| rowid-baseline | 520,192 | 1.058264 | 69,632 | 118,784 | 55,717 | +0 | 39 |
| rowid-compact-unpacked | 540,672 | 1.099928 | 90,112 | 118,784 | 67,373 | +11,656 | 39 |
| rowid-compact-count8 | 540,672 | 1.099928 | 86,016 | 122,880 | 65,089 | +9,372 | 39 |
| rowid-compact-count32 | 552,960 | 1.124927 | 86,016 | 135,168 | 65,574 | +9,857 | 39 |

### nonsparse: 491,552 samples

Original Go file: 4,648,960 bytes (9.457718 bytes/sample). Its physical object census is retained in `nonsparse-original.json`.

| Layout | File bytes | Bytes/sample | Groups b-tree | Payload b-tree | Stored directories | Added directory bytes | Overflow pages |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 127,199 | +0 | 64 |
| count1 | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 144,384 | +17,185 | 64 |
| count2 | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 143,481 | +16,282 | 64 |
| count4 | 4,562,944 | 9.282729 | 303,104 | 4,198,400 | 142,961 | +15,762 | 576 |
| count8 | 3,690,496 | 7.507845 | 303,104 | 3,325,952 | 142,689 | +15,490 | 832 |
| count16 | 3,727,360 | 7.582840 | 303,104 | 3,362,816 | 142,664 | +15,465 | 679 |
| count32 | 3,727,360 | 7.582840 | 303,104 | 3,362,816 | 142,664 | +15,465 | 679 |
| bytes1024 | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 144,384 | +17,185 | 64 |
| bytes2048 | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 144,384 | +17,185 | 64 |
| bytes4096 | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 143,481 | +16,282 | 64 |
| bytes8192 | 3,727,360 | 7.582840 | 303,104 | 3,362,816 | 142,875 | +15,676 | 474 |
| bytes16384 | 3,727,360 | 7.582840 | 303,104 | 3,362,816 | 142,664 | +15,465 | 679 |
| baseline-repeat | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 127,199 | +0 | 64 |
| compact-unpacked | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 133,525 | +6,326 | 64 |
| compact-count1 | 4,571,136 | 9.299395 | 303,104 | 4,206,592 | 133,810 | +6,611 | 64 |
| compact-count8 | 3,690,496 | 7.507845 | 303,104 | 3,325,952 | 131,650 | +4,451 | 832 |
| compact-count32 | 3,727,360 | 7.582840 | 303,104 | 3,362,816 | 131,477 | +4,278 | 679 |
| rowid-baseline | 4,440,064 | 9.032745 | 167,936 | 4,206,592 | 127,199 | +0 | 0 |
| rowid-compact-unpacked | 4,452,352 | 9.057744 | 180,224 | 4,206,592 | 133,525 | +6,326 | 0 |
| rowid-compact-count8 | 3,559,424 | 7.241195 | 167,936 | 3,325,952 | 131,650 | +4,451 | 768 |
| rowid-compact-count32 | 3,596,288 | 7.316190 | 167,936 | 3,362,816 | 131,477 | +4,278 | 615 |

### edge: 5,768 samples

Original Go file: 69,632 bytes (12.072122 bytes/sample). Its physical object census is retained in `edge-original.json`.

| Layout | File bytes | Bytes/sample | Groups b-tree | Payload b-tree | Stored directories | Added directory bytes | Overflow pages |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 69,632 | 12.072122 | 4,096 | 4,096 | 765 | +0 | 0 |
| count1 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,005 | +240 | 0 |
| count2 | 69,632 | 12.072122 | 4,096 | 4,096 | 997 | +232 | 0 |
| count4 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,003 | +238 | 0 |
| count8 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,008 | +243 | 0 |
| count16 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,010 | +245 | 0 |
| count32 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,011 | +246 | 0 |
| bytes1024 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,010 | +245 | 0 |
| bytes2048 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,011 | +246 | 0 |
| bytes4096 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,011 | +246 | 0 |
| bytes8192 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,011 | +246 | 0 |
| bytes16384 | 69,632 | 12.072122 | 4,096 | 4,096 | 1,011 | +246 | 0 |
| baseline-repeat | 69,632 | 12.072122 | 4,096 | 4,096 | 765 | +0 | 0 |
| compact-unpacked | 69,632 | 12.072122 | 4,096 | 4,096 | 877 | +112 | 0 |
| compact-count1 | 69,632 | 12.072122 | 4,096 | 4,096 | 877 | +112 | 0 |
| compact-count8 | 69,632 | 12.072122 | 4,096 | 4,096 | 886 | +121 | 0 |
| compact-count32 | 69,632 | 12.072122 | 4,096 | 4,096 | 892 | +127 | 0 |
| rowid-baseline | 73,728 | 12.782247 | 4,096 | 4,096 | 765 | +0 | 0 |
| rowid-compact-unpacked | 73,728 | 12.782247 | 4,096 | 4,096 | 877 | +112 | 0 |
| rowid-compact-count8 | 73,728 | 12.782247 | 4,096 | 4,096 | 886 | +121 | 0 |
| rowid-compact-count32 | 73,728 | 12.782247 | 4,096 | 4,096 | 892 | +127 | 0 |

Each raw density record also contains every object's leaf/internal/overflow pages, cell payload, unused bytes and maximum cell payload. Pack references and row headers are included in groups/payload b-trees, not hidden in an uncounted map. Any existing groups WITHOUT ROWID overflow waste remains charged and is not a packing saving.

## Six balanced performance passes

Numbers below are median microseconds per synchronous backend operation. Point returns one value after decoding its complete block. Sparse reads select 16 blocks across the fixture; range selects up to eight adjacent blocks in one series; full reads the selected series' complete sealed range within the query ceiling; summary folds whole-block exact sums and counts without fetching any value pack. Identical selections and result digests are checked across every implementation and pass.

Each cell uses one common fixed iteration count across candidates. A separate 16-operation pilot targets at least 150 ms for its fastest implementation and caps the slowest projected interval at one second. Counts and projections are in `plan.json`. Six passes reverse candidate order, keeping comparisons for one workload/cache adjacent. Eight warm operations precede each process. Raw timing does not include db-status/VFS/strace instrumentation.

Warm means a warmed 1 MiB SQLite cache. Cold uses `PRAGMA shrink_memory` outside each timed interval; prepared statements stay cached and Linux page cache is not evicted. This shared WSL2 VM does not establish bare-Linux request latency or cold physical disk throughput.

## Compact format and independent row-layout controls

Format 0x51 replaces obsolete payload ids inside one compressed directory stream, copying the original scalar/exact-summary and inline tokens verbatim. Compact-unpacked is a same-format control with offset zero and original payload rows; compact-count1 separately charges a pack header/row per body. ROWID-group variants retain and charge their composite UNIQUE index. Group size32 and inline16 stay fixed in this study.

The first format0x50 was preserved as executed source under format50-source, with its original environment-density.json. Format0x51 has separate environment-density-format51.json and the later performance environment. All corpora/databases/binaries stay outside Git. This refinement adds no production format reader or migration.

Alibaba's best packed combined layout must also be compared with rowid-baseline: the simpler unpacked ROWID groups are smaller. Its apparent saving against original WITHOUT ROWID groups is chiefly a row-layout effect, not a packing gain. TSBS compact-count8 saves only about0.55% against the fresh original control; the nonsparse fixture has a much larger genuine pack gain. The companion group study has a different fresh insertion control and its percentages are not multiplied into these results.

Four expected-CRC bytes per external reference are charged explicitly. A dedicated test constructs intact different bodies with equal head and clock; each passes its own original checksum, while the expected trailer rejects an address swap. Removing that field would weaken the tested address-integrity contract, not eliminate an inherent SQLite tax.

The compact control changes directory representation, so original v4 directory byte identity is not claimed for0x51. Immutable encoded value bodies, clock residuals, exact-summary encodings and every sample bit remain checked.

### tsbs

| Query / cache | baseline/whole | compact-count8/range | compact-count8/whole | rowid-compact-count32/range | rowid-compact-count32/whole | rowid-compact-unpacked/whole |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| point / warm | 10.82 | 14.09 | 14.96 | 13.37 | 16.46 | 14.41 |
| point / cold | 16.46 | 20.23 | 20.34 | 19.81 | 21.75 | 20.42 |
| sparse / warm | 127.34 | 163.61 | 165.78 | 157.29 | 169.91 | 166.47 |
| sparse / cold | 168.27 | 197.81 | 205.89 | 197.34 | 198.28 | 204.55 |
| range / warm | 27.94 | 29.59 | 30.67 | 30.20 | 30.74 | 30.25 |
| range / cold | 35.63 | 37.81 | 38.40 | 37.88 | 37.29 | 37.10 |
| full / warm | 32.40 | 34.17 | 35.18 | 34.40 | 35.30 | 34.83 |
| full / cold | 38.89 | 41.49 | 42.20 | 41.01 | 40.04 | 41.63 |
| summary / warm | 9.31 | 12.15 | 11.97 | 12.23 | 12.35 | 12.45 |
| summary / cold | 12.23 | 15.82 | 15.78 | 16.13 | 15.75 | 15.91 |

Representative raw passes, microseconds; columns retain the six values instead of hiding spread:

| Query/cache | Implementation | Six passes | Iterations/pass |
| --- | --- | --- | ---: |
| point/warm | baseline/whole | 10.65; 10.49; 11.62; 10.99; 10.55; 11.19 | 13226 |
| point/warm | compact-count8/range | 14.59; 12.85; 14.91; 15.05; 13.59; 13.44 | 13226 |
| point/warm | compact-count8/whole | 14.69; 14.73; 16.60; 16.03; 14.85; 15.07 | 13226 |
| point/warm | rowid-compact-count32/range | 13.05; 13.19; 12.94; 13.65; 13.67; 13.55 | 13226 |
| point/warm | rowid-compact-count32/whole | 16.52; 16.16; 16.44; 16.48; 16.20; 17.35 | 13226 |
| point/warm | rowid-compact-unpacked/whole | 15.14; 13.29; 14.13; 14.69; 13.63; 14.96 | 13226 |
| point/cold | baseline/whole | 16.43; 16.49; 17.30; 17.83; 15.87; 15.16 | 8543 |
| point/cold | compact-count8/range | 19.68; 19.53; 21.35; 21.78; 20.79; 19.20 | 8543 |
| point/cold | compact-count8/whole | 20.99; 19.86; 20.51; 21.06; 20.17; 19.46 | 8543 |
| point/cold | rowid-compact-count32/range | 19.18; 19.27; 20.35; 21.88; 21.89; 18.75 | 8543 |
| point/cold | rowid-compact-count32/whole | 22.18; 21.32; 22.71; 22.42; 20.89; 21.08 | 8543 |
| point/cold | rowid-compact-unpacked/whole | 20.11; 20.93; 21.52; 20.73; 20.04; 18.97 | 8543 |
| full/warm | baseline/whole | 32.10; 31.35; 32.55; 32.25; 38.64; 33.80 | 4870 |
| full/warm | compact-count8/range | 33.42; 34.72; 34.52; 37.85; 33.82; 32.05 | 4870 |
| full/warm | compact-count8/whole | 35.60; 36.53; 34.42; 36.45; 34.76; 34.64 | 4870 |
| full/warm | rowid-compact-count32/range | 34.80; 35.51; 33.15; 37.37; 33.63; 34.00 | 4870 |
| full/warm | rowid-compact-count32/whole | 37.01; 34.65; 34.76; 41.67; 33.78; 35.83 | 4870 |
| full/warm | rowid-compact-unpacked/whole | 37.14; 33.61; 35.01; 37.56; 32.21; 34.64 | 4870 |
| full/cold | baseline/whole | 38.70; 34.94; 38.54; 44.45; 40.03; 39.07 | 3933 |
| full/cold | compact-count8/range | 44.21; 37.36; 40.97; 42.02; 45.49; 37.89 | 3933 |
| full/cold | compact-count8/whole | 42.50; 37.51; 40.22; 44.68; 41.89; 42.64 | 3933 |
| full/cold | rowid-compact-count32/range | 39.60; 39.30; 41.66; 42.73; 44.13; 40.35 | 3933 |
| full/cold | rowid-compact-count32/whole | 39.26; 36.59; 41.74; 42.25; 40.82; 38.37 | 3933 |
| full/cold | rowid-compact-unpacked/whole | 41.40; 37.21; 41.86; 43.06; 43.76; 41.22 | 3933 |

### alibaba

| Query / cache | baseline/whole | compact-count8/range | compact-count8/whole | rowid-compact-count32/range | rowid-compact-count32/whole | rowid-compact-unpacked/whole |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| point / warm | 22.79 | 23.62 | 24.02 | 22.46 | 23.85 | 24.10 |
| point / cold | 30.25 | 30.95 | 32.10 | 30.25 | 31.82 | 31.89 |
| sparse / warm | 421.17 | 464.68 | 468.45 | 453.32 | 473.36 | 470.67 |
| sparse / cold | 495.27 | 547.05 | 537.56 | 510.26 | 541.11 | 530.90 |
| range / warm | 56.35 | 59.94 | 57.86 | 58.74 | 59.19 | 60.83 |
| range / cold | 65.26 | 67.25 | 65.59 | 63.28 | 64.05 | 68.81 |
| full / warm | 221.93 | 214.73 | 213.62 | 219.58 | 210.66 | 219.27 |
| full / cold | 230.33 | 224.72 | 227.60 | 222.61 | 224.68 | 227.74 |
| summary / warm | 49.22 | 53.34 | 55.73 | 52.19 | 54.34 | 54.96 |
| summary / cold | 54.28 | 57.19 | 56.56 | 56.38 | 55.20 | 59.60 |

Representative raw passes, microseconds; columns retain the six values instead of hiding spread:

| Query/cache | Implementation | Six passes | Iterations/pass |
| --- | --- | --- | ---: |
| point/warm | baseline/whole | 22.48; 21.12; 23.12; 23.80; 22.13; 23.10 | 6664 |
| point/warm | compact-count8/range | 23.25; 21.73; 23.85; 25.49; 23.61; 23.63 | 6664 |
| point/warm | compact-count8/whole | 24.03; 21.39; 24.01; 24.69; 23.37; 25.13 | 6664 |
| point/warm | rowid-compact-count32/range | 23.49; 22.32; 22.27; 22.60; 23.12; 22.01 | 6664 |
| point/warm | rowid-compact-count32/whole | 24.29; 26.40; 23.09; 23.40; 25.33; 22.52 | 6664 |
| point/warm | rowid-compact-unpacked/whole | 24.07; 22.19; 23.69; 24.30; 24.13; 27.80 | 6664 |
| point/cold | baseline/whole | 30.60; 30.15; 30.36; 29.79; 30.12; 31.43 | 5351 |
| point/cold | compact-count8/range | 30.26; 29.28; 31.48; 30.41; 31.91; 31.91 | 5351 |
| point/cold | compact-count8/whole | 33.02; 31.77; 32.03; 29.78; 32.25; 32.18 | 5351 |
| point/cold | rowid-compact-count32/range | 29.16; 31.99; 29.23; 29.73; 30.78; 31.66 | 5351 |
| point/cold | rowid-compact-count32/whole | 32.85; 32.21; 29.54; 31.43; 31.02; 32.22 | 5351 |
| point/cold | rowid-compact-unpacked/whole | 32.56; 31.47; 30.93; 30.91; 32.31; 32.81 | 5351 |
| full/warm | baseline/whole | 213.17; 226.79; 218.53; 218.77; 228.26; 225.09 | 773 |
| full/warm | compact-count8/range | 212.33; 217.13; 229.92; 224.24; 206.41; 208.22 | 773 |
| full/warm | compact-count8/whole | 208.58; 247.17; 213.71; 235.98; 213.52; 205.26 | 773 |
| full/warm | rowid-compact-count32/range | 206.24; 219.68; 222.08; 225.84; 219.48; 201.82 | 773 |
| full/warm | rowid-compact-count32/whole | 208.43; 221.52; 209.01; 250.11; 198.21; 212.32 | 773 |
| full/warm | rowid-compact-unpacked/whole | 216.41; 222.13; 238.20; 235.74; 209.40; 216.36 | 773 |
| full/cold | baseline/whole | 241.71; 251.54; 233.52; 223.17; 227.14; 218.70 | 713 |
| full/cold | compact-count8/range | 225.18; 228.76; 226.88; 222.69; 224.26; 215.41 | 713 |
| full/cold | compact-count8/whole | 222.44; 256.24; 237.80; 217.17; 225.55; 229.65 | 713 |
| full/cold | rowid-compact-count32/range | 223.16; 228.51; 222.06; 214.30; 229.70; 213.06 | 713 |
| full/cold | rowid-compact-count32/whole | 226.74; 245.06; 222.61; 241.43; 218.79; 212.22 | 713 |
| full/cold | rowid-compact-unpacked/whole | 223.37; 266.13; 257.13; 226.42; 229.06; 225.28 | 713 |

## Copies, page-cache work and overflow

These counters come from separate untimed diagnostics. `copied` is bytes returned by SQLite's BLOB APIs; whole-pack mode additionally copies selected body slices, reported separately. Requested bytes exclude inline values already present in metadata. Cache misses include metadata/b-tree/overflow work and are not a count of cold physical disk reads.

| Dataset/query (SQLite cold) | Layout/mode | Requested | Copied | Extra slice copy | Metadata | Cache misses | Opens/reopens |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| tsbs/point | baseline/whole | 975 | 975 | 0 | 609 | 8 | 0/0 |
| tsbs/point | compact-count8/range | 975 | 999 | 0 | 647 | 9 | 1/0 |
| tsbs/point | compact-count8/whole | 975 | 7,816 | 975 | 647 | 8 | 0/0 |
| tsbs/point | rowid-compact-count32/range | 975 | 999 | 0 | 641 | 9 | 1/0 |
| tsbs/point | rowid-compact-count32/whole | 975 | 15,584 | 975 | 641 | 8 | 0/0 |
| tsbs/point | rowid-compact-unpacked/whole | 975 | 975 | 0 | 663 | 9 | 0/0 |
| tsbs/sparse | baseline/whole | 2,879 | 2,879 | 0 | 5,320 | 40 | 0/0 |
| tsbs/sparse | compact-count8/range | 2,879 | 3,095 | 0 | 5,737 | 42 | 1/8 |
| tsbs/sparse | compact-count8/whole | 2,879 | 22,567 | 2,879 | 5,737 | 40 | 0/0 |
| tsbs/sparse | rowid-compact-count32/range | 2,879 | 3,095 | 0 | 5,741 | 46 | 1/8 |
| tsbs/sparse | rowid-compact-count32/whole | 2,879 | 50,045 | 2,879 | 5,741 | 40 | 0/0 |
| tsbs/sparse | rowid-compact-unpacked/whole | 2,879 | 2,879 | 0 | 5,785 | 42 | 0/0 |
| tsbs/full | baseline/whole | 9,710 | 9,710 | 0 | 609 | 10 | 0/0 |
| tsbs/full | compact-count8/range | 9,710 | 9,758 | 0 | 647 | 11 | 1/1 |
| tsbs/full | compact-count8/whole | 9,710 | 15,608 | 9,710 | 647 | 9 | 0/0 |
| tsbs/full | rowid-compact-count32/range | 9,710 | 9,734 | 0 | 641 | 11 | 1/0 |
| tsbs/full | rowid-compact-count32/whole | 9,710 | 15,584 | 9,710 | 641 | 8 | 0/0 |
| tsbs/full | rowid-compact-unpacked/whole | 9,710 | 9,710 | 0 | 663 | 11 | 0/0 |
| tsbs/summary | baseline/whole | 0 | 0 | 0 | 609 | 5 | 0/0 |
| tsbs/summary | compact-count8/range | 0 | 0 | 0 | 647 | 6 | 0/0 |
| tsbs/summary | compact-count8/whole | 0 | 0 | 0 | 647 | 6 | 0/0 |
| tsbs/summary | rowid-compact-count32/range | 0 | 0 | 0 | 641 | 6 | 0/0 |
| tsbs/summary | rowid-compact-count32/whole | 0 | 0 | 0 | 641 | 6 | 0/0 |
| tsbs/summary | rowid-compact-unpacked/whole | 0 | 0 | 0 | 663 | 6 | 0/0 |
| alibaba/point | baseline/whole | 52 | 52 | 0 | 621 | 10 | 0/0 |
| alibaba/point | compact-count8/range | 52 | 76 | 0 | 705 | 10 | 1/0 |
| alibaba/point | compact-count8/whole | 52 | 437 | 52 | 705 | 10 | 0/0 |
| alibaba/point | rowid-compact-count32/range | 52 | 76 | 0 | 706 | 9 | 1/0 |
| alibaba/point | rowid-compact-count32/whole | 52 | 1,567 | 52 | 706 | 9 | 0/0 |
| alibaba/point | rowid-compact-unpacked/whole | 52 | 52 | 0 | 738 | 10 | 0/0 |
| alibaba/sparse | baseline/whole | 928 | 928 | 0 | 17,642 | 71 | 0/0 |
| alibaba/sparse | compact-count8/range | 928 | 1,168 | 0 | 19,038 | 73 | 1/9 |
| alibaba/sparse | compact-count8/whole | 928 | 7,575 | 928 | 19,038 | 73 | 0/0 |
| alibaba/sparse | rowid-compact-count32/range | 928 | 1,168 | 0 | 19,080 | 61 | 1/9 |
| alibaba/sparse | rowid-compact-count32/whole | 928 | 28,962 | 928 | 19,080 | 58 | 0/0 |
| alibaba/sparse | rowid-compact-unpacked/whole | 928 | 928 | 0 | 19,262 | 60 | 0/0 |
| alibaba/full | baseline/whole | 2,336 | 2,336 | 0 | 1,726 | 11 | 0/0 |
| alibaba/full | compact-count8/range | 2,336 | 2,504 | 0 | 1,978 | 11 | 1/6 |
| alibaba/full | compact-count8/whole | 2,336 | 2,940 | 2,336 | 1,978 | 11 | 0/0 |
| alibaba/full | rowid-compact-count32/range | 2,336 | 2,408 | 0 | 1,996 | 10 | 1/2 |
| alibaba/full | rowid-compact-count32/whole | 2,336 | 4,945 | 2,336 | 1,996 | 10 | 0/0 |
| alibaba/full | rowid-compact-unpacked/whole | 2,336 | 2,336 | 0 | 2,054 | 12 | 0/0 |
| alibaba/summary | baseline/whole | 0 | 0 | 0 | 1,726 | 7 | 0/0 |
| alibaba/summary | compact-count8/range | 0 | 0 | 0 | 1,978 | 7 | 0/0 |
| alibaba/summary | compact-count8/whole | 0 | 0 | 0 | 1,978 | 7 | 0/0 |
| alibaba/summary | rowid-compact-count32/range | 0 | 0 | 0 | 1,996 | 7 | 0/0 |
| alibaba/summary | rowid-compact-count32/whole | 0 | 0 | 0 | 1,996 | 7 | 0/0 |
| alibaba/summary | rowid-compact-unpacked/whole | 0 | 0 | 0 | 2,054 | 8 | 0/0 |

Separate `strace-*.txt` summaries count pread64 calls over process startup, fixture selection, eight warmups and one SQLite-cold query. They deliberately provide no speed numbers and do not isolate one query. They complement per-query SQLite misses; neither proves physical storage I/O. Incremental range access may still traverse overflow-chain pages even when it copies only one body.

## Retention and lifecycle economics

Each trace starts from a fresh copy, expires whole prefix blocks at 25/50/75/100% of the sealed time span, and clips overlapping blocks during comparison with original samples. Packs are never relocated/repacked. Directory live-mask publication, reference decrements and last-reference deletion share a FULL WAL transaction. A post-transaction census verifies every refcount/live-byte total and the absence of unreferenced packs; census time is excluded from maintenance diagnostics. Alternate-series retirement separately drops all sealed blocks in every second series, illustrating differing per-series lifetimes. Mutable heads stay present, so 100% here means all sealed blocks, not emptying the complete engine.

| Dataset/layout/scenario | Live packs | Live bodies, B | Retained dead bodies, B | Payload b-tree, B | Freelist pages | WAL before checkpoint, B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| tsbs/baseline/25% | — | — | — | 2,977,792 | 24 | 2,809,872 |
| tsbs/baseline/50% | — | — | — | 2,203,648 | 213 | 2,904,632 |
| tsbs/baseline/75% | — | — | — | 1,056,768 | 493 | 2,043,552 |
| tsbs/baseline/100% | — | — | — | 4,096 | 876 | 2,278,392 |
| tsbs/baseline/alternate-series | — | — | — | 2,162,688 | 227 | 2,678,032 |
| tsbs/compact-count8/25% | 1430 | 2076892 | 521158 | 2,928,640 | 13 | 2,171,272 |
| tsbs/compact-count8/50% | 1425 | 1299371 | 1294988 | 2,928,640 | 13 | 2,274,272 |
| tsbs/compact-count8/75% | 1425 | 779938 | 1814421 | 2,928,640 | 16 | 2,533,832 |
| tsbs/compact-count8/100% | 0 | 0 | 0 | 4,096 | 870 | 1,054,752 |
| tsbs/compact-count8/alternate-series | 1232 | 1321563 | 814458 | 2,617,344 | 94 | 2,179,512 |
| tsbs/rowid-compact-unpacked/25% | — | — | — | 2,977,792 | 24 | 2,855,192 |
| tsbs/rowid-compact-unpacked/50% | — | — | — | 2,203,648 | 213 | 2,949,952 |
| tsbs/rowid-compact-unpacked/75% | — | — | — | 1,056,768 | 493 | 2,088,872 |
| tsbs/rowid-compact-unpacked/100% | — | — | — | 4,096 | 898 | 2,294,872 |
| tsbs/rowid-compact-unpacked/alternate-series | — | — | — | 2,162,688 | 224 | 2,781,032 |
| tsbs/rowid-compact-count32/25% | 425 | 2076892 | 536111 | 3,129,344 | 15 | 2,224,832 |
| tsbs/rowid-compact-count32/50% | 424 | 1299371 | 1310295 | 3,125,248 | 16 | 2,220,712 |
| tsbs/rowid-compact-count32/75% | 424 | 779938 | 1829728 | 3,125,248 | 16 | 2,220,712 |
| tsbs/rowid-compact-count32/100% | 0 | 0 | 0 | 4,096 | 924 | 482,072 |
| tsbs/rowid-compact-count32/alternate-series | 441 | 1321563 | 1336617 | 3,190,784 | 3 | 2,352,552 |
| alibaba/baseline/25% | — | — | — | 2,973,696 | 0 | 5,838,072 |
| alibaba/baseline/50% | — | — | — | 2,560,000 | 101 | 5,685,632 |
| alibaba/baseline/75% | — | — | — | 737,280 | 921 | 3,345,472 |
| alibaba/baseline/100% | — | — | — | 4,096 | 1540 | 4,268,352 |
| alibaba/baseline/alternate-series | — | — | — | 2,338,816 | 411 | 3,893,432 |
| alibaba/compact-count8/25% | 4040 | 1998380 | 412905 | 3,104,768 | 0 | 6,039,952 |
| alibaba/compact-count8/50% | 3002 | 1289142 | 456490 | 2,772,992 | 81 | 6,093,512 |
| alibaba/compact-count8/75% | 1735 | 511332 | 478099 | 1,630,208 | 755 | 3,926,392 |
| alibaba/compact-count8/100% | 0 | 0 | 0 | 4,096 | 1620 | 3,048,832 |
| alibaba/compact-count8/alternate-series | 2671 | 1332757 | 229414 | 2,560,000 | 407 | 4,202,432 |
| alibaba/rowid-compact-unpacked/25% | — | — | — | 2,973,696 | 0 | 4,581,472 |
| alibaba/rowid-compact-unpacked/50% | — | — | — | 2,560,000 | 101 | 4,429,032 |
| alibaba/rowid-compact-unpacked/75% | — | — | — | 737,280 | 704 | 3,032,352 |
| alibaba/rowid-compact-unpacked/100% | — | — | — | 4,096 | 1248 | 4,021,152 |
| alibaba/rowid-compact-unpacked/alternate-series | — | — | — | 2,338,816 | 223 | 4,441,392 |
| alibaba/rowid-compact-count32/25% | 1076 | 1998380 | 593913 | 2,867,200 | 0 | 3,448,472 |
| alibaba/rowid-compact-count32/50% | 1076 | 1289142 | 1303151 | 2,867,200 | 0 | 3,448,472 |
| alibaba/rowid-compact-count32/75% | 997 | 511332 | 1882277 | 2,768,896 | 171 | 3,333,112 |
| alibaba/rowid-compact-count32/100% | 0 | 0 | 0 | 4,096 | 1209 | 1,211,312 |
| alibaba/rowid-compact-count32/alternate-series | 963 | 1332757 | 944411 | 2,686,976 | 104 | 3,572,072 |

File bytes and per-object pages after each checkpoint are in `lifecycle.jsonl`. Deleted/released pages join the freelist and do not shrink the file. Retained dead body bytes inside a live pack are different: they remain owned payload bytes and cannot be reused independently. No unmeasured repacking/compaction saving is assumed. Maintenance times are single-trace diagnostics, not stable speed claims; a production repacking design would need separate extra-I/O/WAL and atomic crash-publication measurements.

## Environment and reproduction

Measured research base: `2bc44a6c61fdc3c692a90dc5d3e00c2c30bf07ad`. Harness source, imported codec/exact modules, Cargo.lock, binary and official SQLite archive SHA256 hashes are preserved in `environment-density.json` and `environment-performance.json`. The user explicitly authorized exploratory uncommitted harness runs; these hashes pin the exact measured content. No production/submodule change is part of this round.

Environment: model name	: AMD Ryzen 7 7700 8-Core Processor; Linux x86_64; CPU affinity 0; cpu.max `max 100000`; memory.max `max`; rustc 1.99.0 (b940084d7 2026-09-28); go version go1.27.1 linux/amd64; Python 3.13.5. Stock rusqlite 0.40.1 links the matched static SQLite 3.53.4 archive and native zstd. `ldd` is preserved. WAL, synchronous=FULL, fullfsync/checkpoint_fullfsync, 4096-byte pages, 1 MiB cache, mmap=0 and query-only readers remain fixed. No other agent builds/tests/measurements ran during retained windows; host/storage activity remains outside the harness's control.

Corpus inputs are public normalized TSBS and Alibaba from the common Go exporter, plus bounded synthetic regular/irregular/nonsparse/IEEE edge cases. Each copied manifest names exact corpus SHA256, sample and series counts and independently verified Go query hashes. Corpus/database files are fetched/generated and not committed.

Reproduce from `<repo>/tinystore/metrics-payload-bench` using [the harness README](../metrics-payload-bench/README.md). The actual selected performance variants and fixed counts are in the raw plan. Every phase must run in an exclusive quiet window, and fixture files live on the Linux volume.

Evidence: [density and per-object pages](data/metrics-payload-2026-10-09/density.jsonl), [all encoded byte replays](data/metrics-payload-2026-10-09/verification-bytes.jsonl), [full bitwise Go-manifest replays](data/metrics-payload-2026-10-09/verification.jsonl), [pilot and counts](data/metrics-payload-2026-10-09/plan.json), [six-pass summary](data/metrics-payload-2026-10-09/summary.json), [raw timings](data/metrics-payload-2026-10-09/timings.jsonl), [untimed cache diagnostics](data/metrics-payload-2026-10-09/diagnostics.jsonl), [retention traces](data/metrics-payload-2026-10-09/lifecycle.jsonl), [environment](data/metrics-payload-2026-10-09/environment-performance.json).

## What follows

Use the measured whole-file gain and retained-dead-byte cost together when judging a pack size. Incremental BLOB is a useful copy-control mechanism only after its open/reopen, metadata, cache-page and overflow-chain work is measured; copy bytes alone do not justify a performance or I/O claim. Production adoption would require hardening the experimental compact directory, exact query-budget accounting for both fetch modes, complete engine retention/head/clock integration, corruption/refcount recovery, crash/concurrency publication tests, reader pooling/cancellation, and a representative ongoing-ingest/maintenance WAL experiment. This prototype establishes none of those production guarantees.
