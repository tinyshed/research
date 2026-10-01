
## blobs-served


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | put-4k | 1 | 1,054; 1,134; 1,159 | 1,134 |
| tinystore | get-4k | 1 | 38,380; 38,185; 37,737 | 38,185 |
| tinystore | put-4k | 64 | 4,311; 4,053; 4,145 | 4,145 |
| tinystore | get-4k | 64 | 100,208; 102,542; 102,543 | 102,542 |
| tinystore | put-1m | 1 | 77; 71; 71 | 71 |
| tinystore | get-1m | 1 | 753; 757; 743 | 753 |
| tinystore | put-1m | 8 | 76; 76; 77 | 76 |
| tinystore | get-1m | 8 | 4,155; 4,197; 4,191 | 4,191 |
| tinystore-sidecar | put-4k | 1 | 949; 1,006; 972 | 972 |
| tinystore-sidecar | get-4k | 1 | 8,893; 8,901; 8,920 | 8,901 |
| tinystore-sidecar | put-4k | 64 | 4,228; 4,126; 4,167 | 4,167 |
| tinystore-sidecar | get-4k | 64 | 71,762; 71,141; 71,419 | 71,419 |
| tinystore-sidecar | put-1m | 1 | 78; 75; 76 | 76 |
| tinystore-sidecar | get-1m | 1 | 504; 500; 512 | 504 |
| tinystore-sidecar | put-1m | 8 | 76; 77; 77 | 77 |
| tinystore-sidecar | get-1m | 8 | 855; 774; 894 | 855 |
| tinystore-server | put-4k | 1 | 912; 969; 905 | 912 |
| tinystore-server | get-4k | 1 | 8,196; 7,959; 7,916 | 7,959 |
| tinystore-server | put-4k | 64 | 4,186; 4,325; 4,187 | 4,187 |
| tinystore-server | get-4k | 64 | 62,543; 62,377; 61,372 | 62,377 |
| tinystore-server | put-1m | 1 | 76; 78; 78 | 78 |
| tinystore-server | get-1m | 1 | 492; 489; 479 | 489 |
| tinystore-server | put-1m | 8 | 77; 77; 77 | 77 |
| tinystore-server | get-1m | 8 | 1,415; 1,375; 1,504 | 1,415 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 56.7 | 1271.84 |
| tinystore-sidecar | 179.0 | 1281.99 |
| tinystore-server | 158.2 | 1277.28 |

## blobs


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | put-4k | 1 | 814; 957; 1,066 | 957 |
| tinystore | get-4k | 1 | 37,738; 38,134; 36,903 | 37,738 |
| tinystore | put-4k | 64 | 4,391; 4,188; 4,187 | 4,188 |
| tinystore | get-4k | 64 | 100,932; 101,187; 101,638 | 101,187 |
| tinystore | put-1m | 1 | 60; 72; 75 | 72 |
| tinystore | get-1m | 1 | 746; 738; 747 | 746 |
| tinystore | put-1m | 8 | 77; 77; 77 | 77 |
| tinystore | get-1m | 8 | 4,198; 4,169; 4,147 | 4,169 |
| tinystore-baseline | put-4k | 1 | 244; 416; 468 | 416 |
| tinystore-baseline | get-4k | 1 | 31,205; 31,621; 31,723 | 31,621 |
| tinystore-baseline | put-4k | 64 | 4,469; 4,353; 4,221 | 4,353 |
| tinystore-baseline | get-4k | 64 | 61,352; 51,471; 57,075 | 57,075 |
| tinystore-baseline | put-1m | 1 | 67; 71; 71 | 71 |
| tinystore-baseline | get-1m | 1 | 741; 750; 739 | 741 |
| tinystore-baseline | put-1m | 8 | 76; 76; 76 | 76 |
| tinystore-baseline | get-1m | 8 | 4,172; 4,134; 4,167 | 4,167 |
| files | put-4k | 1 | 207; 226; 276 | 226 |
| files | get-4k | 1 | 86,022; 85,461; 87,180 | 86,022 |
| files | put-4k | 64 | 2,850; 2,844; 2,681 | 2,844 |
| files | get-4k | 64 | 284,548; 287,804; 284,028 | 284,548 |
| files | put-1m | 1 | 73; 73; 73 | 73 |
| files | get-1m | 1 | 2,571; 2,795; 2,777 | 2,777 |
| files | put-1m | 8 | 78; 78; 78 | 78 |
| files | get-1m | 8 | 6,393; 6,232; 6,394 | 6,393 |
| sqlite | put-4k | 1 | 462; 397; 322 | 397 |
| sqlite | get-4k | 1 | 30,477; 29,997; 30,424 | 30,424 |
| sqlite | put-4k | 64 | 515; 463; 520 | 515 |
| sqlite | get-4k | 64 | 90,636; 91,415; 89,042 | 90,636 |
| sqlite | put-1m | 1 | 38; 38; 38 | 38 |
| sqlite | get-1m | 1 | 1,013; 1,020; 970 | 1,013 |
| sqlite | put-1m | 8 | 38; 39; 39 | 39 |
| sqlite | get-1m | 8 | 2,595; 2,564; 2,663 | 2,595 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 55.9 | 1275.38 |
| tinystore-baseline | 60.8 | 1233.81 |
| files | 60.7 | 1203.62 |
| sqlite | 256.6 | 647.11 |

## crash

tinystore: 30 crashes, 0 lost, 0 wrong
tinystore-sidecar: 30 crashes, 0 lost, 0 wrong
sqlite: 30 crashes, 0 lost, 0 wrong
bbolt: 30 crashes, 0 lost, 0 wrong
badger: 30 crashes, 0 lost, 0 wrong
pebble: 30 crashes, 0 lost, 0 wrong
redis: 30 crashes, 0 lost, 0 wrong

## jobs-served


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | enqueue | 1 | 1,344; 1,311; 1,222 | 1,311 |
| tinystore | enqueue | 8 | 8,544; 7,888; 7,843 | 7,888 |
| tinystore | enqueue | 64 | 29,973; 30,069; 29,612 | 29,973 |
| tinystore | drain | 8 | 7,879; 7,624; 7,435 | 7,624 |
| tinystore-sidecar | enqueue | 1 | 1,082; 1,094; 1,051 | 1,082 |
| tinystore-sidecar | enqueue | 8 | 7,404; 6,849; 6,459 | 6,849 |
| tinystore-sidecar | enqueue | 64 | 27,924; 27,005; 25,250 | 27,005 |
| tinystore-sidecar | drain | 8 | 3,829; 3,676; 3,713 | 3,713 |
| tinystore-server | enqueue | 1 | 1,154; 1,078; 966 | 1,078 |
| tinystore-server | enqueue | 8 | 6,952; 6,267; 6,269 | 6,269 |
| tinystore-server | enqueue | 64 | 26,464; 24,416; 26,183 | 26,183 |
| tinystore-server | drain | 8 | 3,939; 3,747; 3,481 | 3,747 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 42.0 | 18.31 |
| tinystore-sidecar | 70.7 | 16.28 |
| tinystore-server | 70.5 | 15.65 |

## jobs


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | enqueue | 1 | 1,200; 612; 1,224 | 1,200 |
| tinystore | enqueue | 8 | 7,409; 4,277; 6,869 | 6,869 |
| tinystore | enqueue | 64 | 29,172; 18,765; 26,393 | 26,393 |
| tinystore | drain | 8 | 7,866; 4,736; 6,556 | 6,556 |
| tinystore-baseline | enqueue | 1 | 527; 283; 392 | 392 |
| tinystore-baseline | enqueue | 8 | 3,566; 2,125; 2,823 | 2,823 |
| tinystore-baseline | enqueue | 64 | 18,205; 11,262; 15,863 | 15,863 |
| tinystore-baseline | drain | 8 | 3,448; 1,889; 3,030 | 3,030 |
| goqite | enqueue | 1 | 437; 478; 454 | 454 |
| goqite | enqueue | 8 | 500; 466; 492 | 492 |
| goqite | enqueue | 64 | 493; 476; 489 | 489 |
| goqite | drain | 8 | 248; 140; 233 | 233 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 41.6 | 16.20 |
| tinystore-baseline | 40.8 | 8.84 |
| goqite | 43.2 | 2.21 |

## kv


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | get | 1 | 63,358; 62,537; 62,847 | 62,847 |
| tinystore | get | 8 | 193,655; 189,461; 190,279 | 190,279 |
| tinystore | get | 64 | 192,449; 192,122; 192,462 | 192,449 |
| tinystore | set | 1 | 1,166; 1,085; 1,263 | 1,166 |
| tinystore | set | 8 | 5,237; 4,899; 5,153 | 5,153 |
| tinystore | set | 64 | 5,403; 5,420; 5,500 | 5,420 |
| tinystore | mixed | 1 | 9,846; 9,590; 9,823 | 9,823 |
| tinystore | mixed | 8 | 36,717; 33,393; 34,178 | 34,178 |
| tinystore | mixed | 64 | 58,100; 59,053; 59,004 | 59,004 |
| tinystore-baseline | get | 1 | 56,730; 56,759; 56,032 | 56,730 |
| tinystore-baseline | get | 8 | 145,297; 141,113; 139,983 | 141,113 |
| tinystore-baseline | get | 64 | 139,049; 138,985; 137,169 | 138,985 |
| tinystore-baseline | set | 1 | 508; 401; 436 | 436 |
| tinystore-baseline | set | 8 | 2,859; 2,712; 2,419 | 2,712 |
| tinystore-baseline | set | 64 | 5,641; 5,554; 5,677 | 5,641 |
| tinystore-baseline | mixed | 1 | 4,543; 4,244; 4,403 | 4,403 |
| tinystore-baseline | mixed | 8 | 18,043; 17,544; 18,005 | 18,005 |
| tinystore-baseline | mixed | 64 | 48,655; 45,586; 46,124 | 46,124 |
| tinystore-sidecar | get | 1 | 11,346; 11,191; 11,225 | 11,225 |
| tinystore-sidecar | get | 8 | 62,076; 61,202; 61,090 | 61,202 |
| tinystore-sidecar | get | 64 | 114,983; 115,006; 113,684 | 114,983 |
| tinystore-sidecar | set | 1 | 1,116; 1,118; 1,059 | 1,116 |
| tinystore-sidecar | set | 8 | 4,711; 5,242; 4,980 | 4,980 |
| tinystore-sidecar | set | 64 | 5,393; 5,413; 5,383 | 5,393 |
| tinystore-sidecar | mixed | 1 | 5,363; 5,560; 5,562 | 5,560 |
| tinystore-sidecar | mixed | 8 | 26,060; 26,976; 26,899 | 26,899 |
| tinystore-sidecar | mixed | 64 | 60,149; 58,742; 57,762 | 58,742 |
| tinystore-server | get | 1 | 9,914; 10,421; 10,440 | 10,421 |
| tinystore-server | get | 8 | 44,049; 44,361; 43,608 | 44,049 |
| tinystore-server | get | 64 | 91,887; 93,269; 92,741 | 92,741 |
| tinystore-server | set | 1 | 1,087; 1,087; 821 | 1,087 |
| tinystore-server | set | 8 | 4,288; 4,172; 4,275 | 4,275 |
| tinystore-server | set | 64 | 5,721; 5,398; 5,422 | 5,422 |
| tinystore-server | mixed | 1 | 4,938; 5,181; 4,861 | 4,938 |
| tinystore-server | mixed | 8 | 21,897; 23,964; 21,538 | 21,897 |
| tinystore-server | mixed | 64 | 54,427; 57,805; 59,349 | 57,805 |
| sqlite | get | 1 | 32,301; 33,408; 32,490 | 32,490 |
| sqlite | get | 8 | 94,454; 101,044; 99,329 | 99,329 |
| sqlite | get | 64 | 64,529; 70,719; 69,840 | 69,840 |
| sqlite | set | 1 | 316; 504; 502 | 502 |
| sqlite | set | 8 | 329; 537; 488 | 488 |
| sqlite | set | 64 | 345; 485; 538 | 485 |
| sqlite | mixed | 1 | 2,816; 1,819; 4,173 | 2,816 |
| sqlite | mixed | 8 | 3,239; 4,595; 4,005 | 4,005 |
| sqlite | mixed | 64 | 2,379; 5,061; 4,414 | 4,414 |
| bbolt | get | 1 | 454,952; 437,630; 462,892 | 454,952 |
| bbolt | get | 8 | 492,583; 489,663; 521,679 | 492,583 |
| bbolt | get | 64 | 504,666; 494,925; 527,954 | 504,666 |
| bbolt | set | 1 | 81; 77; 84 | 81 |
| bbolt | set | 8 | 621; 610; 622 | 621 |
| bbolt | set | 64 | 3,269; 3,139; 3,448 | 3,269 |
| bbolt | mixed | 1 | 856; 740; 862 | 856 |
| bbolt | mixed | 8 | 6,218; 4,972; 6,285 | 6,218 |
| bbolt | mixed | 64 | 32,484; 32,521; 35,621 | 32,521 |
| bbolt-borrowed | get | 1 | 463,055; 474,250; 494,936 | 474,250 |
| bbolt-borrowed | get | 8 | 495,491; 498,161; 548,250 | 498,161 |
| bbolt-borrowed | get | 64 | 515,747; 507,580; 549,877 | 515,747 |
| bbolt-borrowed | set | 1 | 79; 68; 84 | 79 |
| bbolt-borrowed | set | 8 | 578; 489; 637 | 578 |
| bbolt-borrowed | set | 64 | 3,081; 2,287; 3,535 | 3,081 |
| bbolt-borrowed | mixed | 1 | 717; 566; 829 | 717 |
| bbolt-borrowed | mixed | 8 | 5,543; 5,139; 6,226 | 5,543 |
| bbolt-borrowed | mixed | 64 | 29,846; 27,543; 35,516 | 29,846 |
| badger | get | 1 | 280,102; 271,795; 279,754 | 279,754 |
| badger | get | 8 | 515,488; 459,589; 495,909 | 495,909 |
| badger | get | 64 | 521,509; 463,931; 503,924 | 503,924 |
| badger | set | 1 | 1,192; 541; 1,466 | 1,192 |
| badger | set | 8 | 969; 443; 1,555 | 969 |
| badger | set | 64 | 866; 383; 1,617 | 866 |
| badger | mixed | 1 | 6,573; 3,822; 13,572 | 6,573 |
| badger | mixed | 8 | 6,551; 4,530; 15,093 | 6,551 |
| badger | mixed | 64 | 6,974; 5,043; 16,194 | 6,974 |
| pebble | get | 1 | 70,590; 71,524; 72,616 | 71,524 |
| pebble | get | 8 | 141,528; 147,261; 144,956 | 144,956 |
| pebble | get | 64 | 218,862; 230,705; 228,697 | 228,697 |
| pebble | set | 1 | 303; 431; 509 | 431 |
| pebble | set | 8 | 939; 1,961; 3,038 | 1,961 |
| pebble | set | 64 | 27,857; 40,811; 50,525 | 40,811 |
| pebble | mixed | 1 | 6,239; 9,172; 11,953 | 9,172 |
| pebble | mixed | 8 | 32,137; 50,633; 58,304 | 50,633 |
| pebble | mixed | 64 | 117,774; 133,092; 139,376 | 133,092 |
| redis | get | 1 | 34,385; 34,688; 35,935 | 34,688 |
| redis | get | 8 | 112,192; 110,784; 113,870 | 112,192 |
| redis | get | 64 | 140,241; 136,953; 140,967 | 140,241 |
| redis | set | 1 | 370; 377; 505 | 377 |
| redis | set | 8 | 1,396; 1,648; 2,233 | 1,648 |
| redis | set | 64 | 8,264; 11,872; 14,955 | 11,872 |
| redis | mixed | 1 | 2,936; 4,077; 4,965 | 4,077 |
| redis | mixed | 8 | 3,971; 4,322; 6,329 | 4,322 |
| redis | mixed | 64 | 11,022; 11,568; 16,756 | 11,568 |
| redis-tcp | get | 1 | 30,846; 30,771; 31,690 | 30,846 |
| redis-tcp | get | 8 | 85,326; 85,886; 86,970 | 85,886 |
| redis-tcp | get | 64 | 98,981; 99,894; 102,610 | 99,894 |
| redis-tcp | set | 1 | 313; 421; 506 | 421 |
| redis-tcp | set | 8 | 1,610; 1,570; 2,084 | 1,610 |
| redis-tcp | set | 64 | 15,683; 17,177; 20,157 | 17,177 |
| redis-tcp | mixed | 1 | 3,081; 3,535; 4,500 | 3,535 |
| redis-tcp | mixed | 8 | 4,538; 4,276; 6,110 | 4,538 |
| redis-tcp | mixed | 64 | 14,786; 16,470; 24,143 | 16,470 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 57.8 | 16.51 |
| tinystore-baseline | 65.9 | 16.45 |
| tinystore-sidecar | 87.3 | 16.57 |
| tinystore-server | 87.8 | 16.48 |
| sqlite | 294.2 | 17.25 |
| bbolt | 71.8 | 48.03 |
| bbolt-borrowed | 71.4 | 48.03 |
| badger | 170.3 | 19.96 |
| pebble | 68.4 | 29.73 |
| redis | 79.8 | 31.63 |
| redis-tcp | 79.5 | 37.14 |

## metrics


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | ingest | 1 | 407,259; 423,826; 401,755 | 407,259 |
| tinystore | settle | 1 | 0; 0; 0 | 0 |
| tinystore | read-series | 1 | 3,414; 3,545; 3,469 | 3,469 |
| tinystore | read-series | 8 | 6,061; 6,012; 5,975 | 6,012 |
| tinystore | read-wide | 1 | 21; 21; 21 | 21 |
| tinystore-baseline | ingest | 1 | 351,903; 357,158; 362,983 | 357,158 |
| tinystore-baseline | settle | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | read-series | 1 | 3,298; 3,108; 3,164 | 3,164 |
| tinystore-baseline | read-series | 8 | 4,739; 4,798; 4,778 | 4,778 |
| tinystore-baseline | read-wide | 1 | 20; 20; 21 | 20 |
| prometheus | ingest | 1 | 4,165,253; 4,102,067; 4,064,088 | 4,102,067 |
| prometheus | settle | 1 | 0; 0; 0 | 0 |
| prometheus | read-series | 1 | 32,931; 33,802; 34,246 | 33,802 |
| prometheus | read-series | 8 | 137,388; 140,286; 143,370 | 140,286 |
| prometheus | read-wide | 1 | 35; 36; 36 | 36 |
| victoria | ingest | 1 | 2,954,890; 2,965,550; 2,937,709 | 2,954,890 |
| victoria | settle | 1 | 0; 0; 0 | 0 |
| victoria | read-series | 1 | 1,319; 1,325; 1,307 | 1,319 |
| victoria | read-series | 8 | 5,454; 5,551; 5,465 | 5,465 |
| victoria | read-wide | 1 | 5; 5; 5 | 5 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 226.8 | 6.45 |
| tinystore-baseline | 226.6 | 6.45 |
| prometheus | 278.0 | 71.60 |
| victoria | 593.4 | 9.03 |

## records


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | append | 1 | 227,189; 212,466; 213,664 | 213,664 |
| tinystore | settle | 1 | 0; 0; 0 | 0 |
| tinystore | read-window | 1 | 609; 614; 611 | 611 |
| tinystore | read-window | 8 | 1,029; 1,028; 1,022 | 1,028 |
| tinystore-baseline | append | 1 | 162,996; 201,568; 198,286 | 198,286 |
| tinystore-baseline | settle | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | read-window | 1 | 594; 590; 597 | 594 |
| tinystore-baseline | read-window | 8 | 1,009; 1,011; 1,005 | 1,009 |
| sqlite | append | 1 | 85,846; 94,543; 95,397 | 94,543 |
| sqlite | settle | 1 | 0; 0; 0 | 0 |
| sqlite | read-window | 1 | 5,056; 6,344; 6,139 | 6,139 |
| sqlite | read-window | 8 | 28,252; 28,425; 27,998 | 28,252 |
| jsonl | append | 1 | 44,440; 50,065; 45,902 | 45,902 |
| jsonl | settle | 1 | 0; 0; 0 | 0 |
| jsonl | read-window | 1 | 2,600; 2,637; 2,534 | 2,600 |
| jsonl | read-window | 8 | 5,775; 6,101; 6,188 | 6,101 |
| jsonl-zstd | append | 1 | 39,237; 46,058; 51,539 | 46,058 |
| jsonl-zstd | settle | 1 | 0; 0; 0 | 0 |
| jsonl-zstd | read-window | 1 | 641; 644; 592 | 641 |
| jsonl-zstd | read-window | 8 | 1,999; 2,029; 2,015 | 2,015 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 84.2 | 0.57 |
| tinystore-baseline | 83.5 | 0.58 |
| sqlite | 63.7 | 3.73 |
| jsonl | 80.7 | 3.19 |
| jsonl-zstd | 353.2 | 0.32 |

## sdk-kv


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| bun-redis | get | 1 | 38,625; 41,315 | 39,970 |
| bun-redis | get | 8 | 196,455; 199,311 | 197,883 |
| bun-redis | get | 64 | 263,173; 260,855 | 262,014 |
| bun-redis | set | 1 | 451; 479 | 465 |
| bun-redis | set | 8 | 1,783; 1,992 | 1,887 |
| bun-redis | set | 64 | 15,481; 14,557 | 15,019 |
| bun-redis | mixed | 1 | 3,623; 4,473 | 4,048 |
| bun-redis | mixed | 8 | 5,436; 5,996 | 5,716 |
| bun-redis | mixed | 64 | 17,455; 19,115 | 18,285 |
| bun-redis-tcp | get | 1 | 36,214; 34,339 | 35,277 |
| bun-redis-tcp | get | 8 | 120,983; 104,358 | 112,671 |
| bun-redis-tcp | get | 64 | 144,355; 143,889 | 144,122 |
| bun-redis-tcp | set | 1 | 382; 500 | 441 |
| bun-redis-tcp | set | 8 | 1,447; 2,209 | 1,828 |
| bun-redis-tcp | set | 64 | 11,312; 15,340 | 13,326 |
| bun-redis-tcp | mixed | 1 | 2,988; 4,401 | 3,694 |
| bun-redis-tcp | mixed | 8 | 3,961; 5,178 | 4,569 |
| bun-redis-tcp | mixed | 64 | 13,965; 17,677 | 15,821 |
| bun-tinystore | get | 1 | 11,395; 11,316 | 11,356 |
| bun-tinystore | get | 8 | 42,037; 40,995 | 41,516 |
| bun-tinystore | get | 64 | 68,408; 67,891 | 68,149 |
| bun-tinystore | set | 1 | 1,004; 1,028 | 1,016 |
| bun-tinystore | set | 8 | 3,938; 4,586 | 4,262 |
| bun-tinystore | set | 64 | 3,792; 5,554 | 4,673 |
| bun-tinystore | mixed | 1 | 4,568; 5,339 | 4,954 |
| bun-tinystore | mixed | 8 | 23,807; 23,344 | 23,575 |
| bun-tinystore | mixed | 64 | 50,593; 51,749 | 51,171 |
| bun-tinystore-server | get | 1 | 10,712; 10,766 | 10,739 |
| bun-tinystore-server | get | 8 | 36,950; 37,061 | 37,006 |
| bun-tinystore-server | get | 64 | 66,347; 66,384 | 66,366 |
| bun-tinystore-server | set | 1 | 614; 1,008 | 811 |
| bun-tinystore-server | set | 8 | 2,728; 4,642 | 3,685 |
| bun-tinystore-server | set | 64 | 5,991; 5,563 | 5,777 |
| bun-tinystore-server | mixed | 1 | 3,442; 5,015 | 4,228 |
| bun-tinystore-server | mixed | 8 | 18,169; 21,934 | 20,051 |
| bun-tinystore-server | mixed | 64 | 45,998; 49,359 | 47,679 |
| python-redis | get | 1 | 15,809; 15,380 | 15,594 |
| python-redis | get | 8 | 17,865; 17,637 | 17,751 |
| python-redis | get | 64 | 18,472; 18,353 | 18,412 |
| python-redis | set | 1 | 420; 535 | 477 |
| python-redis | set | 8 | 1,859; 2,262 | 2,060 |
| python-redis | set | 64 | 10,953; 14,948 | 12,951 |
| python-redis | mixed | 1 | 3,591; 4,122 | 3,856 |
| python-redis | mixed | 8 | 4,045; 5,321 | 4,683 |
| python-redis | mixed | 64 | 13,614; 15,258 | 14,436 |
| python-redis-tcp | get | 1 | 14,150; 14,124 | 14,137 |
| python-redis-tcp | get | 8 | 15,698; 15,717 | 15,707 |
| python-redis-tcp | get | 64 | 16,333; 16,239 | 16,286 |
| python-redis-tcp | set | 1 | 469; 467 | 468 |
| python-redis-tcp | set | 8 | 1,952; 2,185 | 2,068 |
| python-redis-tcp | set | 64 | 12,007; 12,852 | 12,430 |
| python-redis-tcp | mixed | 1 | 3,540; 3,641 | 3,590 |
| python-redis-tcp | mixed | 8 | 4,748; 4,847 | 4,797 |
| python-redis-tcp | mixed | 64 | 14,748; 14,069 | 14,408 |
| python-tinystore | get | 1 | 5,887; 5,177 | 5,532 |
| python-tinystore | get | 8 | 14,424; 13,731 | 14,077 |
| python-tinystore | get | 64 | 22,155; 21,989 | 22,072 |
| python-tinystore | set | 1 | 830; 936 | 883 |
| python-tinystore | set | 8 | 3,512; 4,006 | 3,759 |
| python-tinystore | set | 64 | 5,986; 6,336 | 6,161 |
| python-tinystore | mixed | 1 | 3,623; 4,144 | 3,884 |
| python-tinystore | mixed | 8 | 13,392; 13,662 | 13,527 |
| python-tinystore | mixed | 64 | 20,661; 20,714 | 20,687 |
| python-tinystore-server | get | 1 | 4,614; 4,403 | 4,508 |
| python-tinystore-server | get | 8 | 13,152; 13,201 | 13,177 |
| python-tinystore-server | get | 64 | 21,839; 21,938 | 21,889 |
| python-tinystore-server | set | 1 | 929; 944 | 936 |
| python-tinystore-server | set | 8 | 3,961; 4,071 | 4,016 |
| python-tinystore-server | set | 64 | 5,499; 5,582 | 5,540 |
| python-tinystore-server | mixed | 1 | 3,619; 3,846 | 3,733 |
| python-tinystore-server | mixed | 8 | 13,064; 13,234 | 13,149 |
| python-tinystore-server | mixed | 64 | 20,252; 20,703 | 20,477 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| bun-redis | 111.5 | 0.00 |
| bun-redis-tcp | 98.0 | 0.00 |
| bun-tinystore | 124.4 | 0.00 |
| bun-tinystore-server | 122.7 | 0.00 |
| python-redis | 68.9 | 0.00 |
| python-redis-tcp | 68.8 | 0.00 |
| python-tinystore | 79.3 | 0.00 |
| python-tinystore-server | 78.2 | 0.00 |

## sqldb-mattn


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| mattn | get | 1 | 50,678; 51,794; 51,707 | 51,707 |
| mattn | get | 8 | 249,002; 244,703; 244,670 | 244,703 |
| mattn | get | 64 | 276,036; 275,780; 277,928 | 276,036 |
| mattn | insert | 1 | 440; 482; 423 | 440 |
| mattn | insert | 8 | 593; 548; 499 | 548 |
| mattn | insert | 64 | 590; 473; 468 | 473 |
| mattn | mixed | 1 | 4,515; 4,183; 3,944 | 4,183 |
| mattn | mixed | 8 | 4,359; 4,953; 4,477 | 4,477 |
| mattn | mixed | 64 | 4,073; 4,854; 4,151 | 4,151 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| mattn | 198.1 | 34.73 |

## sqldb-served


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | get | 1 | 62,510; 62,726; 62,742 | 62,726 |
| tinystore | get | 8 | 205,088; 205,458; 208,815 | 205,458 |
| tinystore | get | 64 | 207,289; 208,362; 204,319 | 207,289 |
| tinystore | insert | 1 | 1,476; 1,349; 1,487 | 1,476 |
| tinystore | insert | 8 | 9,178; 9,094; 9,103 | 9,103 |
| tinystore | insert | 64 | 41,762; 43,158; 43,426 | 43,158 |
| tinystore | mixed | 1 | 10,090; 11,464; 11,293 | 11,293 |
| tinystore | mixed | 8 | 40,221; 43,226; 40,745 | 40,745 |
| tinystore | mixed | 64 | 51,234; 48,675; 48,922 | 48,922 |
| tinystore-sidecar | get | 1 | 9,886; 9,893; 9,886 | 9,886 |
| tinystore-sidecar | get | 8 | 57,792; 58,289; 57,660 | 57,792 |
| tinystore-sidecar | get | 64 | 102,234; 103,042; 102,890 | 102,890 |
| tinystore-sidecar | insert | 1 | 1,026; 1,136; 1,222 | 1,136 |
| tinystore-sidecar | insert | 8 | 6,376; 7,675; 7,045 | 7,045 |
| tinystore-sidecar | insert | 64 | 30,397; 31,675; 32,119 | 31,675 |
| tinystore-sidecar | mixed | 1 | 4,964; 5,095; 5,489 | 5,095 |
| tinystore-sidecar | mixed | 8 | 26,475; 28,475; 27,364 | 27,364 |
| tinystore-sidecar | mixed | 64 | 50,696; 51,009; 51,129 | 51,009 |
| tinystore-server | get | 1 | 4,120; 4,111; 4,121 | 4,120 |
| tinystore-server | get | 8 | 20,654; 20,764; 20,709 | 20,709 |
| tinystore-server | get | 64 | 31,896; 31,742; 32,162 | 31,896 |
| tinystore-server | insert | 1 | 852; 949; 976 | 949 |
| tinystore-server | insert | 8 | 4,819; 4,775; 5,115 | 4,819 |
| tinystore-server | insert | 64 | 17,579; 17,481; 18,368 | 17,579 |
| tinystore-server | mixed | 1 | 2,952; 2,937; 2,996 | 2,952 |
| tinystore-server | mixed | 8 | 15,379; 14,895; 15,588 | 15,379 |
| tinystore-server | mixed | 64 | 28,125; 27,589; 27,956 | 27,956 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 54.9 | 127.68 |
| tinystore-sidecar | 83.6 | 103.00 |
| tinystore-server | 84.1 | 73.25 |

## sqldb


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | get | 1 | 62,231; 62,519; 60,973 | 62,231 |
| tinystore | get | 8 | 208,224; 203,073; 208,873 | 208,224 |
| tinystore | get | 64 | 206,480; 205,079; 207,454 | 206,480 |
| tinystore | insert | 1 | 1,382; 1,573; 1,385 | 1,385 |
| tinystore | insert | 8 | 9,205; 8,560; 9,169 | 9,169 |
| tinystore | insert | 64 | 41,463; 39,735; 39,514 | 39,735 |
| tinystore | mixed | 1 | 11,846; 11,013; 11,126 | 11,126 |
| tinystore | mixed | 8 | 41,155; 42,565; 40,841 | 41,155 |
| tinystore | mixed | 64 | 49,701; 49,422; 49,335 | 49,422 |
| tinystore-baseline | get | 1 | 53,397; 52,656; 52,775 | 52,775 |
| tinystore-baseline | get | 8 | 160,977; 160,250; 162,342 | 160,977 |
| tinystore-baseline | get | 64 | 160,959; 158,336; 160,304 | 160,304 |
| tinystore-baseline | insert | 1 | 448; 493; 521 | 493 |
| tinystore-baseline | insert | 8 | 3,292; 3,013; 3,787 | 3,292 |
| tinystore-baseline | insert | 64 | 19,953; 20,448; 20,647 | 20,448 |
| tinystore-baseline | mixed | 1 | 4,345; 4,689; 4,481 | 4,481 |
| tinystore-baseline | mixed | 8 | 19,447; 20,677; 21,030 | 20,677 |
| tinystore-baseline | mixed | 64 | 50,706; 51,248; 50,869 | 50,869 |
| sqlite | get | 1 | 37,077; 37,134; 36,273 | 37,077 |
| sqlite | get | 8 | 140,411; 142,006; 142,231 | 142,006 |
| sqlite | get | 64 | 114,785; 121,268; 115,206 | 115,206 |
| sqlite | insert | 1 | 380; 555; 559 | 555 |
| sqlite | insert | 8 | 541; 486; 463 | 486 |
| sqlite | insert | 64 | 477; 507; 500 | 500 |
| sqlite | mixed | 1 | 3,216; 4,516; 4,243 | 4,243 |
| sqlite | mixed | 8 | 4,624; 5,156; 5,021 | 5,021 |
| sqlite | mixed | 64 | 5,257; 4,917; 4,866 | 4,917 |
| postgres | get | 1 | 20,699; 20,746; 19,851 | 20,699 |
| postgres | get | 8 | 70,538; 71,143; 71,712 | 71,143 |
| postgres | get | 64 | 108,322; 108,947; 109,543 | 108,947 |
| postgres | insert | 1 | 1,186; 1,464; 1,334 | 1,334 |
| postgres | insert | 8 | 4,427; 5,669; 6,125 | 5,669 |
| postgres | insert | 64 | 39,648; 37,714; 36,742 | 37,714 |
| postgres | mixed | 1 | 7,439; 7,851; 8,306 | 7,851 |
| postgres | mixed | 8 | 30,237; 41,010; 39,347 | 39,347 |
| postgres | mixed | 64 | 94,117; 93,998; 93,278 | 93,998 |
| ncruces | get | 1 | 44,773; 44,061; 46,184 | 44,773 |
| ncruces | get | 8 | 216,486; 212,032; 214,487 | 214,487 |
| ncruces | get | 64 | 205,559; 205,324; 205,232 | 205,324 |
| ncruces | insert | 1 | 1,255; 1,535; 1,451 | 1,451 |
| ncruces | insert | 8 | 1,164; 1,565; 1,201 | 1,201 |
| ncruces | insert | 64 | 1,079; 1,379; 1,381 | 1,379 |
| ncruces | mixed | 1 | 7,514; 10,358; 9,722 | 9,722 |
| ncruces | mixed | 8 | 10,032; 13,169; 12,583 | 12,583 |
| ncruces | mixed | 64 | 9,651; 13,277; 12,161 | 12,161 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 55.2 | 121.73 |
| tinystore-baseline | 59.2 | 74.36 |
| sqlite | 295.2 | 34.26 |
| postgres | 60.6 | 352.15 |
| ncruces | 219.6 | 38.67 |

## stack-latency

('tinystore-batch-baseline', 'request-max', 64), repeat 1: {'logs_stored': 420103, 'logs_dropped': 0, 'jobs_enqueued': 41894, 'jobs_handled': 41862, 'jobs_waiting': 32}
('services', 'request-50%', 1024), repeat 1: {'logs_stored': 146673, 'logs_dropped': 0, 'jobs_enqueued': 14551, 'jobs_handled': 14539, 'jobs_waiting': 12}
('services', 'request-75%', 1024), repeat 2: {'logs_stored': 228666, 'logs_dropped': 0, 'jobs_enqueued': 22652, 'jobs_handled': 22615, 'jobs_waiting': 37}

| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore-batch | request-max | 64 | 40,402; 39,994 | 40,198 |
| tinystore-batch | request-25% | 1024 | 10,098; 9,992 | 10,045 |
| tinystore-batch | request-50% | 1024 | 20,196; 19,995 | 20,095 |
| tinystore-batch | request-75% | 1024 | 30,262; 29,864 | 30,063 |
| tinystore-batch | request-90% | 1024 | 36,315; 35,892 | 36,104 |
| tinystore-batch-baseline | request-max | 64 | 34,990; 31,877 | 33,434 |
| tinystore-batch-baseline | request-25% | 1024 | 8,746; 7,964 | 8,355 |
| tinystore-batch-baseline | request-50% | 1024 | 17,488; 15,931 | 16,710 |
| tinystore-batch-baseline | request-75% | 1024 | 26,228; 23,894 | 25,061 |
| tinystore-batch-baseline | request-90% | 1024 | 31,452; 28,658 | 30,055 |
| services | request-max | 64 | 29,334; 30,489 | 29,911 |
| services | request-25% | 1024 | 7,330; 7,620 | 7,475 |
| services | request-50% | 1024 | 14,659; 15,233 | 14,946 |
| services | request-75% | 1024 | 21,693; 22,850 | 22,271 |
| services | request-90% | 1024 | 26,397; 27,425 | 26,911 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore-batch | 116.9 | 502.96 |
| tinystore-batch-baseline | 151.5 | 415.72 |
| services | 147.4 | 594.04 |

## stack-steady

('tinystore-batch-baseline', 'request', 64), repeat 2: {'logs_stored': 5637692, 'logs_dropped': 0, 'jobs_enqueued': 563458, 'jobs_handled': 563426, 'jobs_waiting': 32}

| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore-batch | request | 64 | 35,151; 35,169 | 35,160 |
| tinystore-batch-baseline | request | 64 | 31,493; 31,311 | 31,402 |
| services | request | 64 | 24,418; 27,603 | 26,011 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore-batch | 111.0 | 2101.11 |
| tinystore-batch-baseline | 141.4 | 1880.84 |
| services | 153.4 | 2378.79 |

## stack

('tinystore-batch', 'request', 256), repeat 1: {'logs_stored': 246837, 'logs_dropped': 0, 'jobs_enqueued': 24728, 'jobs_handled': 21691, 'jobs_waiting': 3037}
('tinystore-batch-baseline', 'request', 256), repeat 1: {'logs_stored': 249658, 'logs_dropped': 0, 'jobs_enqueued': 24856, 'jobs_handled': 24684, 'jobs_waiting': 172}
('tinystore-sidecar', 'request', 8), repeat 1: {'logs_stored': 63399, 'logs_dropped': 0, 'jobs_enqueued': 6412, 'jobs_handled': 6411, 'jobs_waiting': 1}
('tinystore-sidecar', 'request', 256), repeat 1: {'logs_stored': 231429, 'logs_dropped': 0, 'jobs_enqueued': 23086, 'jobs_handled': 22978, 'jobs_waiting': 108}
('tinystore-server', 'request', 64), repeat 1: {'logs_stored': 100930, 'logs_dropped': 0, 'jobs_enqueued': 10160, 'jobs_handled': 10156, 'jobs_waiting': 4}
('tinystore-server', 'request', 256), repeat 1: {'logs_stored': 121816, 'logs_dropped': 0, 'jobs_enqueued': 12031, 'jobs_handled': 12025, 'jobs_waiting': 10}
('tinystore-sidecar', 'request', 8), repeat 2: {'logs_stored': 68785, 'logs_dropped': 0, 'jobs_enqueued': 6968, 'jobs_handled': 6967, 'jobs_waiting': 1}
('tinystore-sidecar', 'request', 256), repeat 2: {'logs_stored': 226179, 'logs_dropped': 0, 'jobs_enqueued': 22490, 'jobs_handled': 22472, 'jobs_waiting': 18}
('tinystore-baseline', 'request', 8), repeat 2: {'logs_stored': 49141, 'logs_dropped': 0, 'jobs_enqueued': 4952, 'jobs_handled': 4951, 'jobs_waiting': 1}
('tinystore-baseline', 'request', 256), repeat 2: {'logs_stored': 251403, 'logs_dropped': 0, 'jobs_enqueued': 25087, 'jobs_handled': 24716, 'jobs_waiting': 371}
('tinystore', 'request', 256), repeat 2: {'logs_stored': 250497, 'logs_dropped': 0, 'jobs_enqueued': 25066, 'jobs_handled': 24959, 'jobs_waiting': 107}
('tinystore-batch-baseline', 'request', 256), repeat 2: {'logs_stored': 246444, 'logs_dropped': 0, 'jobs_enqueued': 24525, 'jobs_handled': 24446, 'jobs_waiting': 79}
('tinystore-batch', 'request', 256), repeat 2: {'logs_stored': 244898, 'logs_dropped': 0, 'jobs_enqueued': 24510, 'jobs_handled': 22275, 'jobs_waiting': 2235}
('tinystore-batch', 'request', 256), repeat 3: {'logs_stored': 244630, 'logs_dropped': 0, 'jobs_enqueued': 24513, 'jobs_handled': 21526, 'jobs_waiting': 2987}
('tinystore', 'request', 64), repeat 3: {'logs_stored': 248253, 'logs_dropped': 0, 'jobs_enqueued': 24709, 'jobs_handled': 24707, 'jobs_waiting': 2}
('tinystore-baseline', 'request', 8), repeat 3: {'logs_stored': 54756, 'logs_dropped': 0, 'jobs_enqueued': 5518, 'jobs_handled': 5517, 'jobs_waiting': 1}
('tinystore-baseline', 'request', 256), repeat 3: {'logs_stored': 237232, 'logs_dropped': 0, 'jobs_enqueued': 23628, 'jobs_handled': 23532, 'jobs_waiting': 96}
('tinystore-sidecar', 'request', 64), repeat 3: {'logs_stored': 195950, 'logs_dropped': 0, 'jobs_enqueued': 19501, 'jobs_handled': 19495, 'jobs_waiting': 6}
('tinystore-server', 'request', 8), repeat 3: {'logs_stored': 48663, 'logs_dropped': 0, 'jobs_enqueued': 4890, 'jobs_handled': 4889, 'jobs_waiting': 1}
('tinystore-server', 'request', 256), repeat 3: {'logs_stored': 121416, 'logs_dropped': 0, 'jobs_enqueued': 12030, 'jobs_handled': 12026, 'jobs_waiting': 4}

| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore-batch | request | 8 | 22,527; 24,062; 24,096 | 24,062 |
| tinystore-batch | request | 64 | 44,295; 42,916; 43,178 | 43,178 |
| tinystore-batch | request | 256 | 40,308; 39,710; 39,865 | 39,865 |
| tinystore-batch | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore-batch | restore | 1 | 0; 0; 0 | 0 |
| tinystore-batch-baseline | request | 8 | 12,301; 13,467; 13,516 | 13,467 |
| tinystore-batch-baseline | request | 64 | 34,161; 32,580; 35,455 | 34,161 |
| tinystore-batch-baseline | request | 256 | 41,051; 40,932; 40,332 | 40,932 |
| tinystore-batch-baseline | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore-batch-baseline | restore | 1 | 0; 0; 0 | 0 |
| tinystore | request | 8 | 17,208; 17,851; 21,080 | 17,851 |
| tinystore | request | 64 | 41,953; 39,918; 37,669 | 39,918 |
| tinystore | request | 256 | 41,098; 41,019; 41,178 | 41,098 |
| tinystore | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore | restore | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | request | 8 | 8,165; 8,341; 9,336 | 8,341 |
| tinystore-baseline | request | 64 | 32,169; 31,812; 34,568 | 32,169 |
| tinystore-baseline | request | 256 | 40,154; 41,319; 37,925 | 40,154 |
| tinystore-baseline | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | restore | 1 | 0; 0; 0 | 0 |
| tinystore-sidecar | request | 8 | 11,253; 12,338; 13,198 | 12,338 |
| tinystore-sidecar | request | 64 | 32,285; 32,638; 32,517 | 32,517 |
| tinystore-sidecar | request | 256 | 37,816; 36,962; 35,475 | 36,962 |
| tinystore-server | request | 8 | 7,516; 8,489; 8,363 | 8,363 |
| tinystore-server | request | 64 | 16,803; 16,885; 16,856 | 16,856 |
| tinystore-server | request | 256 | 20,262; 20,307; 20,106 | 20,262 |
| services | request | 8 | 12,865; 13,040; 14,130 | 13,040 |
| services | request | 64 | 28,372; 26,859; 28,567 | 28,372 |
| services | request | 256 | 31,667; 34,751; 36,784 | 34,751 |
| services | backup-bytes | 1 | 0; 0; 0 | 0 |
| services | restore | 1 | 0; 0; 0 | 0 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore-batch | 104.4 | 228.46 |
| tinystore-batch-baseline | 134.4 | 184.78 |
| tinystore | 105.1 | 212.67 |
| tinystore-baseline | 133.2 | 172.52 |
| tinystore-sidecar | 149.1 | 172.62 |
| tinystore-server | 139.4 | 97.39 |
| services | 128.9 | 270.04 |

## weight

