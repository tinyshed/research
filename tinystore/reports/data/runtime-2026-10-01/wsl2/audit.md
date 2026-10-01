
## blobs-served


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | put-4k | 1 | 598; 596; 595 | 596 |
| tinystore | get-4k | 1 | 78,654; 77,940; 78,155 | 78,155 |
| tinystore | put-4k | 64 | 11,125; 11,869; 11,207 | 11,207 |
| tinystore | get-4k | 64 | 190,954; 183,013; 188,693 | 188,693 |
| tinystore | put-1m | 1 | 100; 103; 101 | 101 |
| tinystore | get-1m | 1 | 1,739; 1,758; 1,739 | 1,739 |
| tinystore | put-1m | 8 | 365; 350; 357 | 357 |
| tinystore | get-1m | 8 | 13,033; 13,328; 13,326 | 13,326 |
| tinystore-sidecar | put-4k | 1 | 540; 540; 529 | 540 |
| tinystore-sidecar | get-4k | 1 | 5,255; 5,253; 5,241 | 5,253 |
| tinystore-sidecar | put-4k | 64 | 10,986; 11,005; 11,318 | 11,005 |
| tinystore-sidecar | get-4k | 64 | 163,125; 167,193; 167,304 | 167,193 |
| tinystore-sidecar | put-1m | 1 | 95; 95; 93 | 95 |
| tinystore-sidecar | get-1m | 1 | 1,108; 1,111; 1,032 | 1,108 |
| tinystore-sidecar | put-1m | 8 | 339; 324; 325 | 325 |
| tinystore-sidecar | get-1m | 8 | 1,208; 1,166; 1,201 | 1,201 |
| tinystore-server | put-4k | 1 | 542; 541; 540 | 541 |
| tinystore-server | get-4k | 1 | 5,266; 5,521; 5,280 | 5,280 |
| tinystore-server | put-4k | 64 | 10,316; 11,424; 9,598 | 10,316 |
| tinystore-server | get-4k | 64 | 99,903; 100,806; 100,473 | 100,473 |
| tinystore-server | put-1m | 1 | 97; 98; 95 | 97 |
| tinystore-server | get-1m | 1 | 899; 898; 898 | 898 |
| tinystore-server | put-1m | 8 | 334; 327; 326 | 327 |
| tinystore-server | get-1m | 8 | 1,950; 1,964; 1,966 | 1,964 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 58.5 | 3215.73 |
| tinystore-sidecar | 204.4 | 2955.00 |
| tinystore-server | 183.0 | 3002.87 |

## blobs


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | put-4k | 1 | 594; 596; 599 | 596 |
| tinystore | get-4k | 1 | 77,951; 77,939; 77,539 | 77,939 |
| tinystore | put-4k | 64 | 11,638; 11,759; 11,838 | 11,759 |
| tinystore | get-4k | 64 | 184,258; 183,412; 188,818 | 184,258 |
| tinystore | put-1m | 1 | 106; 98; 99 | 99 |
| tinystore | get-1m | 1 | 1,666; 1,751; 1,730 | 1,730 |
| tinystore | put-1m | 8 | 386; 356; 331 | 356 |
| tinystore | get-1m | 8 | 12,454; 13,120; 13,141 | 13,120 |
| tinystore-baseline | put-4k | 1 | 359; 352; 364 | 359 |
| tinystore-baseline | get-4k | 1 | 67,979; 69,656; 68,825 | 68,825 |
| tinystore-baseline | put-4k | 64 | 8,451; 8,559; 8,523 | 8,523 |
| tinystore-baseline | get-4k | 64 | 121,862; 119,249; 117,302 | 119,249 |
| tinystore-baseline | put-1m | 1 | 92; 92; 85 | 92 |
| tinystore-baseline | get-1m | 1 | 1,742; 1,715; 1,741 | 1,741 |
| tinystore-baseline | put-1m | 8 | 350; 325; 321 | 325 |
| tinystore-baseline | get-1m | 8 | 13,068; 13,211; 13,249 | 13,211 |
| files | put-4k | 1 | 159; 158; 157 | 158 |
| files | get-4k | 1 | 135,710; 135,654; 137,822 | 135,710 |
| files | put-4k | 64 | 4,077; 4,099; 4,094 | 4,094 |
| files | get-4k | 64 | 411,092; 410,168; 424,447 | 411,092 |
| files | put-1m | 1 | 142; 138; 125 | 138 |
| files | get-1m | 1 | 8,680; 7,058; 8,015 | 8,015 |
| files | put-1m | 8 | 556; 591; 467 | 556 |
| files | get-1m | 8 | 13,966; 13,779; 13,453 | 13,779 |
| sqlite | put-4k | 1 | 362; 366; 366 | 366 |
| sqlite | get-4k | 1 | 76,834; 77,519; 78,545 | 77,519 |
| sqlite | put-4k | 64 | 374; 376; 375 | 375 |
| sqlite | get-4k | 64 | 144,208; 137,366; 139,800 | 139,800 |
| sqlite | put-1m | 1 | 128; 130; 123 | 128 |
| sqlite | get-1m | 1 | 1,978; 1,775; 2,046 | 1,978 |
| sqlite | put-1m | 8 | 129; 125; 123 | 125 |
| sqlite | get-1m | 8 | 4,164; 4,219; 4,231 | 4,219 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 58.9 | 3177.94 |
| tinystore-baseline | 63.0 | 2912.07 |
| files | 223.1 | 4427.22 |
| sqlite | 275.8 | 1677.34 |

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
| tinystore | enqueue | 1 | 593; 597; 595 | 595 |
| tinystore | enqueue | 8 | 4,594; 4,594; 4,565 | 4,594 |
| tinystore | enqueue | 64 | 26,313; 25,882; 26,006 | 26,006 |
| tinystore | drain | 8 | 4,614; 4,603; 4,634 | 4,614 |
| tinystore-sidecar | enqueue | 1 | 537; 537; 533 | 537 |
| tinystore-sidecar | enqueue | 8 | 4,237; 4,220; 4,071 | 4,220 |
| tinystore-sidecar | enqueue | 64 | 24,056; 24,255; 23,849 | 24,056 |
| tinystore-sidecar | drain | 8 | 2,318; 2,314; 2,304 | 2,314 |
| tinystore-server | enqueue | 1 | 542; 541; 522 | 541 |
| tinystore-server | enqueue | 8 | 4,242; 4,167; 4,173 | 4,173 |
| tinystore-server | enqueue | 64 | 24,152; 23,980; 23,747 | 23,980 |
| tinystore-server | drain | 8 | 2,282; 2,301; 2,293 | 2,293 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 43.0 | 14.70 |
| tinystore-sidecar | 72.3 | 13.62 |
| tinystore-server | 72.6 | 13.54 |

## jobs


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | enqueue | 1 | 602; 598; 601 | 601 |
| tinystore | enqueue | 8 | 4,617; 4,598; 4,620 | 4,617 |
| tinystore | enqueue | 64 | 25,956; 25,992; 25,549 | 25,956 |
| tinystore | drain | 8 | 4,622; 4,589; 4,648 | 4,622 |
| tinystore-baseline | enqueue | 1 | 367; 377; 369 | 369 |
| tinystore-baseline | enqueue | 8 | 2,743; 2,826; 2,827 | 2,826 |
| tinystore-baseline | enqueue | 64 | 15,382; 15,436; 15,019 | 15,382 |
| tinystore-baseline | drain | 8 | 2,761; 2,709; 2,718 | 2,718 |
| goqite | enqueue | 1 | 380; 361; 374 | 374 |
| goqite | enqueue | 8 | 368; 361; 369 | 368 |
| goqite | enqueue | 64 | 363; 364; 356 | 363 |
| goqite | drain | 8 | 184; 184; 181 | 184 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 43.1 | 14.66 |
| tinystore-baseline | 41.7 | 8.66 |
| goqite | 43.2 | 1.68 |

## kv


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | get | 1 | 124,239; 124,159; 123,775 | 124,159 |
| tinystore | get | 8 | 330,680; 341,455; 342,532 | 341,455 |
| tinystore | get | 64 | 374,988; 388,085; 384,797 | 384,797 |
| tinystore | set | 1 | 345; 627; 622 | 622 |
| tinystore | set | 8 | 2,533; 4,322; 4,328 | 4,322 |
| tinystore | set | 64 | 16,775; 19,304; 19,282 | 19,282 |
| tinystore | mixed | 1 | 5,548; 5,724; 5,749 | 5,724 |
| tinystore | mixed | 8 | 33,914; 33,595; 34,320 | 33,914 |
| tinystore | mixed | 64 | 117,323; 120,328; 120,923 | 120,328 |
| tinystore-baseline | get | 1 | 126,032; 125,952; 123,530 | 125,952 |
| tinystore-baseline | get | 8 | 307,033; 309,595; 316,281 | 309,595 |
| tinystore-baseline | get | 64 | 311,996; 315,502; 313,055 | 313,055 |
| tinystore-baseline | set | 1 | 355; 354; 367 | 355 |
| tinystore-baseline | set | 8 | 2,344; 2,343; 2,335 | 2,343 |
| tinystore-baseline | set | 64 | 11,169; 11,193; 11,245 | 11,193 |
| tinystore-baseline | mixed | 1 | 3,269; 3,203; 3,313 | 3,269 |
| tinystore-baseline | mixed | 8 | 18,535; 18,552; 18,448 | 18,535 |
| tinystore-baseline | mixed | 64 | 69,911; 70,728; 70,818 | 70,728 |
| tinystore-sidecar | get | 1 | 5,547; 5,556; 5,542 | 5,547 |
| tinystore-sidecar | get | 8 | 45,029; 45,222; 45,069 | 45,069 |
| tinystore-sidecar | get | 64 | 273,278; 279,369; 274,117 | 274,117 |
| tinystore-sidecar | set | 1 | 563; 569; 562 | 563 |
| tinystore-sidecar | set | 8 | 3,904; 3,943; 3,957 | 3,943 |
| tinystore-sidecar | set | 64 | 18,393; 18,352; 18,398 | 18,393 |
| tinystore-sidecar | mixed | 1 | 2,813; 2,850; 2,871 | 2,850 |
| tinystore-sidecar | mixed | 8 | 18,397; 18,547; 18,548 | 18,547 |
| tinystore-sidecar | mixed | 64 | 109,779; 106,920; 110,464 | 109,779 |
| tinystore-server | get | 1 | 5,797; 5,762; 5,824 | 5,797 |
| tinystore-server | get | 8 | 38,015; 38,218; 37,747 | 38,015 |
| tinystore-server | get | 64 | 141,686; 141,079; 141,747 | 141,686 |
| tinystore-server | set | 1 | 566; 570; 572 | 570 |
| tinystore-server | set | 8 | 3,895; 3,954; 3,941 | 3,941 |
| tinystore-server | set | 64 | 18,313; 14,945; 18,519 | 18,313 |
| tinystore-server | mixed | 1 | 2,953; 2,979; 2,945 | 2,953 |
| tinystore-server | mixed | 8 | 17,791; 18,019; 17,960 | 17,960 |
| tinystore-server | mixed | 64 | 86,916; 88,028; 89,117 | 88,028 |
| sqlite | get | 1 | 97,826; 94,488; 93,481 | 94,488 |
| sqlite | get | 8 | 250,914; 249,680; 251,164 | 250,914 |
| sqlite | get | 64 | 141,871; 144,803; 144,386 | 144,386 |
| sqlite | set | 1 | 373; 373; 376 | 373 |
| sqlite | set | 8 | 369; 335; 367 | 367 |
| sqlite | set | 64 | 365; 356; 367 | 365 |
| sqlite | mixed | 1 | 3,300; 3,256; 3,279 | 3,279 |
| sqlite | mixed | 8 | 3,558; 3,521; 3,444 | 3,521 |
| sqlite | mixed | 64 | 3,169; 3,511; 3,335 | 3,335 |
| bbolt | get | 1 | 1,006,896; 1,008,560; 988,054 | 1,006,896 |
| bbolt | get | 8 | 980,390; 1,038,150; 1,021,373 | 1,021,373 |
| bbolt | get | 64 | 1,072,313; 1,067,066; 1,033,358 | 1,067,066 |
| bbolt | set | 1 | 75; 75; 75 | 75 |
| bbolt | set | 8 | 596; 596; 596 | 596 |
| bbolt | set | 64 | 4,417; 4,406; 4,407 | 4,407 |
| bbolt | mixed | 1 | 763; 710; 758 | 758 |
| bbolt | mixed | 8 | 5,802; 6,039; 5,774 | 5,802 |
| bbolt | mixed | 64 | 43,803; 43,527; 43,535 | 43,535 |
| bbolt-borrowed | get | 1 | 1,100,971; 1,102,227; 1,105,582 | 1,102,227 |
| bbolt-borrowed | get | 8 | 1,080,753; 1,098,792; 1,112,969 | 1,098,792 |
| bbolt-borrowed | get | 64 | 1,151,891; 1,131,515; 1,167,146 | 1,151,891 |
| bbolt-borrowed | set | 1 | 74; 75; 75 | 75 |
| bbolt-borrowed | set | 8 | 597; 597; 596 | 597 |
| bbolt-borrowed | set | 64 | 4,406; 4,405; 4,407 | 4,406 |
| bbolt-borrowed | mixed | 1 | 709; 691; 781 | 709 |
| bbolt-borrowed | mixed | 8 | 5,988; 6,047; 5,807 | 5,988 |
| bbolt-borrowed | mixed | 64 | 43,297; 43,623; 43,193 | 43,297 |
| badger | get | 1 | 632,466; 663,387; 675,465 | 663,387 |
| badger | get | 8 | 878,703; 885,137; 891,080 | 885,137 |
| badger | get | 64 | 932,969; 901,780; 945,610 | 932,969 |
| badger | set | 1 | 482; 497; 463 | 482 |
| badger | set | 8 | 632; 624; 615 | 624 |
| badger | set | 64 | 692; 640; 638 | 640 |
| badger | mixed | 1 | 4,903; 4,769; 4,753 | 4,769 |
| badger | mixed | 8 | 5,132; 4,680; 4,620 | 4,680 |
| badger | mixed | 64 | 6,300; 5,717; 6,007 | 6,007 |
| pebble | get | 1 | 140,861; 139,776; 138,682 | 139,776 |
| pebble | get | 8 | 263,205; 259,658; 260,180 | 260,180 |
| pebble | get | 64 | 380,904; 383,358; 376,677 | 380,904 |
| pebble | set | 1 | 321; 318; 324 | 321 |
| pebble | set | 8 | 1,306; 1,296; 1,279 | 1,296 |
| pebble | set | 64 | 20,343; 20,416; 20,576 | 20,416 |
| pebble | mixed | 1 | 5,948; 5,959; 5,881 | 5,948 |
| pebble | mixed | 8 | 25,683; 25,696; 25,675 | 25,683 |
| pebble | mixed | 64 | 178,393; 180,350; 172,369 | 178,393 |
| redis | get | 1 | 13,745; 13,512; 13,637 | 13,637 |
| redis | get | 8 | 66,993; 66,368; 66,910 | 66,910 |
| redis | get | 64 | 183,602; 179,887; 182,954 | 182,954 |
| redis | set | 1 | 301; 308; 299 | 301 |
| redis | set | 8 | 1,234; 1,232; 1,238 | 1,234 |
| redis | set | 64 | 9,427; 9,431; 9,461 | 9,431 |
| redis | mixed | 1 | 2,528; 2,510; 2,619 | 2,528 |
| redis | mixed | 8 | 3,413; 3,382; 3,509 | 3,413 |
| redis | mixed | 64 | 10,325; 10,368; 10,478 | 10,368 |
| redis-tcp | get | 1 | 11,998; 11,990; 11,983 | 11,990 |
| redis-tcp | get | 8 | 63,875; 63,231; 63,943 | 63,875 |
| redis-tcp | get | 64 | 147,265; 149,184; 148,221 | 148,221 |
| redis-tcp | set | 1 | 314; 314; 307 | 314 |
| redis-tcp | set | 8 | 1,227; 1,235; 1,260 | 1,235 |
| redis-tcp | set | 64 | 9,635; 9,656; 9,694 | 9,656 |
| redis-tcp | mixed | 1 | 2,386; 2,382; 2,405 | 2,386 |
| redis-tcp | mixed | 8 | 3,343; 3,296; 3,338 | 3,338 |
| redis-tcp | mixed | 64 | 9,928; 10,074; 10,209 | 10,074 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 60.0 | 16.20 |
| tinystore-baseline | 67.3 | 16.28 |
| tinystore-sidecar | 90.0 | 16.40 |
| tinystore-server | 90.7 | 16.44 |
| sqlite | 297.6 | 17.26 |
| bbolt | 73.2 | 48.03 |
| bbolt-borrowed | 72.8 | 48.03 |
| badger | 170.6 | 18.47 |
| pebble | 73.5 | 29.72 |
| redis | 82.8 | 28.33 |
| redis-tcp | 83.1 | 28.50 |

## metrics


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | ingest | 1 | 983,384; 981,931; 974,939 | 981,931 |
| tinystore | settle | 1 | 0; 0; 0 | 0 |
| tinystore | read-series | 1 | 9,914; 9,713; 9,579 | 9,713 |
| tinystore | read-series | 8 | 17,471; 17,451; 17,330 | 17,451 |
| tinystore | read-wide | 1 | 45; 45; 45 | 45 |
| tinystore-baseline | ingest | 1 | 839,702; 845,617; 834,670 | 839,702 |
| tinystore-baseline | settle | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | read-series | 1 | 8,599; 8,865; 8,867 | 8,865 |
| tinystore-baseline | read-series | 8 | 12,203; 12,234; 12,157 | 12,203 |
| tinystore-baseline | read-wide | 1 | 46; 46; 45 | 46 |
| prometheus | ingest | 1 | 10,496,620; 10,767,047; 10,283,402 | 10,496,620 |
| prometheus | settle | 1 | 0; 0; 0 | 0 |
| prometheus | read-series | 1 | 71,583; 69,828; 66,131 | 69,828 |
| prometheus | read-series | 8 | 455,108; 446,446; 425,502 | 446,446 |
| prometheus | read-wide | 1 | 72; 73; 71 | 72 |
| victoria | ingest | 1 | 7,134,964; 7,123,819; 6,968,012 | 7,123,819 |
| victoria | settle | 1 | 0; 0; 0 | 0 |
| victoria | read-series | 1 | 2,369; 2,315; 2,325 | 2,325 |
| victoria | read-series | 8 | 12,482; 12,710; 12,875 | 12,710 |
| victoria | read-wide | 1 | 10; 12; 11 | 11 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 240.6 | 6.45 |
| tinystore-baseline | 237.3 | 6.45 |
| prometheus | 284.6 | 71.61 |
| victoria | 687.8 | 9.03 |

## records


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | append | 1 | 315,476; 327,910; 332,345 | 327,910 |
| tinystore | settle | 1 | 0; 0; 0 | 0 |
| tinystore | read-window | 1 | 3,368; 3,425; 3,407 | 3,407 |
| tinystore | read-window | 8 | 5,795; 5,819; 5,818 | 5,818 |
| tinystore-baseline | append | 1 | 221,412; 210,321; 228,626 | 221,412 |
| tinystore-baseline | settle | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | read-window | 1 | 2,404; 2,434; 2,425 | 2,425 |
| tinystore-baseline | read-window | 8 | 5,129; 5,158; 5,237 | 5,158 |
| sqlite | append | 1 | 122,861; 123,458; 125,228 | 123,458 |
| sqlite | settle | 1 | 0; 0; 0 | 0 |
| sqlite | read-window | 1 | 44,709; 44,731; 45,276 | 44,731 |
| sqlite | read-window | 8 | 166,043; 168,928; 168,016 | 168,016 |
| jsonl | append | 1 | 24,982; 24,686; 24,813 | 24,813 |
| jsonl | settle | 1 | 0; 0; 0 | 0 |
| jsonl | read-window | 1 | 2,319; 2,305; 2,336 | 2,319 |
| jsonl | read-window | 8 | 10,436; 10,623; 10,641 | 10,623 |
| jsonl-zstd | append | 1 | 21,968; 24,606; 24,585 | 24,585 |
| jsonl-zstd | settle | 1 | 0; 0; 0 | 0 |
| jsonl-zstd | read-window | 1 | 805; 791; 811 | 805 |
| jsonl-zstd | read-window | 8 | 2,631; 2,637; 2,662 | 2,637 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 379.5 | 12.34 |
| tinystore-baseline | 383.5 | 12.35 |
| sqlite | 368.1 | 164.43 |
| jsonl | 436.4 | 103.14 |
| jsonl-zstd | 628.3 | 8.43 |

## sdk-kv


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| bun-redis | get | 1 | 19,518; 19,505 | 19,512 |
| bun-redis | get | 8 | 138,594; 142,861 | 140,727 |
| bun-redis | get | 64 | 344,140; 353,046 | 348,593 |
| bun-redis | set | 1 | 311; 318 | 314 |
| bun-redis | set | 8 | 1,235; 1,246 | 1,240 |
| bun-redis | set | 64 | 9,957; 10,105 | 10,031 |
| bun-redis | mixed | 1 | 2,702; 2,829 | 2,766 |
| bun-redis | mixed | 8 | 3,538; 3,661 | 3,599 |
| bun-redis | mixed | 64 | 10,737; 11,096 | 10,917 |
| bun-redis-tcp | get | 1 | 15,173; 15,495 | 15,334 |
| bun-redis-tcp | get | 8 | 88,102; 90,784 | 89,443 |
| bun-redis-tcp | get | 64 | 161,510; 164,399 | 162,955 |
| bun-redis-tcp | set | 1 | 308; 311 | 310 |
| bun-redis-tcp | set | 8 | 1,225; 1,221 | 1,223 |
| bun-redis-tcp | set | 64 | 9,759; 9,939 | 9,849 |
| bun-redis-tcp | mixed | 1 | 2,557; 2,601 | 2,579 |
| bun-redis-tcp | mixed | 8 | 3,373; 3,397 | 3,385 |
| bun-redis-tcp | mixed | 64 | 11,237; 11,137 | 11,187 |
| bun-tinystore | get | 1 | 7,117; 7,213 | 7,165 |
| bun-tinystore | get | 8 | 48,494; 47,531 | 48,012 |
| bun-tinystore | get | 64 | 140,731; 136,128 | 138,429 |
| bun-tinystore | set | 1 | 577; 574 | 576 |
| bun-tinystore | set | 8 | 3,836; 3,917 | 3,877 |
| bun-tinystore | set | 64 | 15,950; 16,882 | 16,416 |
| bun-tinystore | mixed | 1 | 3,212; 3,229 | 3,221 |
| bun-tinystore | mixed | 8 | 19,183; 19,237 | 19,210 |
| bun-tinystore | mixed | 64 | 91,399; 93,327 | 92,363 |
| bun-tinystore-server | get | 1 | 6,907; 6,947 | 6,927 |
| bun-tinystore-server | get | 8 | 39,125; 39,162 | 39,143 |
| bun-tinystore-server | get | 64 | 115,472; 115,301 | 115,386 |
| bun-tinystore-server | set | 1 | 572; 579 | 576 |
| bun-tinystore-server | set | 8 | 3,781; 3,824 | 3,802 |
| bun-tinystore-server | set | 64 | 16,809; 16,143 | 16,476 |
| bun-tinystore-server | mixed | 1 | 3,093; 3,220 | 3,156 |
| bun-tinystore-server | mixed | 8 | 18,050; 18,203 | 18,126 |
| bun-tinystore-server | mixed | 64 | 82,156; 82,685 | 82,420 |
| python-redis | get | 1 | 10,549; 10,842 | 10,695 |
| python-redis | get | 8 | 17,389; 17,842 | 17,615 |
| python-redis | get | 64 | 18,232; 18,713 | 18,473 |
| python-redis | set | 1 | 301; 304 | 303 |
| python-redis | set | 8 | 1,233; 1,241 | 1,237 |
| python-redis | set | 64 | 8,980; 9,106 | 9,043 |
| python-redis | mixed | 1 | 2,515; 2,421 | 2,468 |
| python-redis | mixed | 8 | 3,253; 3,086 | 3,169 |
| python-redis | mixed | 64 | 10,171; 9,986 | 10,079 |
| python-redis-tcp | get | 1 | 9,528; 9,724 | 9,626 |
| python-redis-tcp | get | 8 | 15,649; 15,674 | 15,662 |
| python-redis-tcp | get | 64 | 16,158; 16,036 | 16,097 |
| python-redis-tcp | set | 1 | 311; 312 | 311 |
| python-redis-tcp | set | 8 | 1,268; 1,206 | 1,237 |
| python-redis-tcp | set | 64 | 9,333; 9,396 | 9,365 |
| python-redis-tcp | mixed | 1 | 2,351; 2,412 | 2,382 |
| python-redis-tcp | mixed | 8 | 3,104; 2,958 | 3,031 |
| python-redis-tcp | mixed | 64 | 9,804; 9,960 | 9,882 |
| python-tinystore | get | 1 | 5,244; 4,795 | 5,020 |
| python-tinystore | get | 8 | 29,518; 28,700 | 29,109 |
| python-tinystore | get | 64 | 57,674; 56,827 | 57,250 |
| python-tinystore | set | 1 | 560; 561 | 560 |
| python-tinystore | set | 8 | 3,658; 3,714 | 3,686 |
| python-tinystore | set | 64 | 14,176; 14,294 | 14,235 |
| python-tinystore | mixed | 1 | 2,792; 2,958 | 2,875 |
| python-tinystore | mixed | 8 | 16,260; 16,591 | 16,425 |
| python-tinystore | mixed | 64 | 49,961; 52,779 | 51,370 |
| python-tinystore-server | get | 1 | 4,808; 4,790 | 4,799 |
| python-tinystore-server | get | 8 | 26,158; 26,373 | 26,266 |
| python-tinystore-server | get | 64 | 54,621; 57,304 | 55,962 |
| python-tinystore-server | set | 1 | 562; 563 | 562 |
| python-tinystore-server | set | 8 | 3,665; 3,706 | 3,685 |
| python-tinystore-server | set | 64 | 13,590; 13,564 | 13,577 |
| python-tinystore-server | mixed | 1 | 2,759; 2,806 | 2,782 |
| python-tinystore-server | mixed | 8 | 15,815; 16,527 | 16,171 |
| python-tinystore-server | mixed | 64 | 49,707; 52,181 | 50,944 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| bun-redis | 120.5 | 0.00 |
| bun-redis-tcp | 100.6 | 0.00 |
| bun-tinystore | 132.2 | 0.00 |
| bun-tinystore-server | 127.3 | 0.00 |
| python-redis | 69.8 | 0.00 |
| python-redis-tcp | 69.5 | 0.00 |
| python-tinystore | 85.9 | 0.00 |
| python-tinystore-server | 85.1 | 0.00 |

## sqldb-mattn


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| mattn | get | 1 | 131,243; 130,956; 131,517 | 131,243 |
| mattn | get | 8 | 328,838; 334,139; 325,171 | 328,838 |
| mattn | get | 64 | 643,268; 638,850; 628,935 | 638,850 |
| mattn | insert | 1 | 373; 380; 389 | 380 |
| mattn | insert | 8 | 369; 388; 372 | 372 |
| mattn | insert | 64 | 369; 383; 378 | 378 |
| mattn | mixed | 1 | 3,464; 3,454; 3,443 | 3,454 |
| mattn | mixed | 8 | 3,444; 3,489; 3,500 | 3,489 |
| mattn | mixed | 64 | 3,470; 3,553; 3,658 | 3,553 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| mattn | 207.0 | 34.15 |

## sqldb-served


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | get | 1 | 143,753; 141,768; 141,066 | 141,768 |
| tinystore | get | 8 | 313,610; 311,485; 318,372 | 313,610 |
| tinystore | get | 64 | 373,281; 362,931; 367,983 | 367,983 |
| tinystore | insert | 1 | 621; 630; 637 | 630 |
| tinystore | insert | 8 | 4,686; 4,706; 4,752 | 4,706 |
| tinystore | insert | 64 | 31,880; 32,085; 31,894 | 31,894 |
| tinystore | mixed | 1 | 5,999; 5,958; 5,870 | 5,958 |
| tinystore | mixed | 8 | 34,601; 34,985; 34,977 | 34,977 |
| tinystore | mixed | 64 | 139,113; 138,738; 139,491 | 139,113 |
| tinystore-sidecar | get | 1 | 5,433; 5,370; 5,397 | 5,397 |
| tinystore-sidecar | get | 8 | 49,296; 48,814; 49,632 | 49,296 |
| tinystore-sidecar | get | 64 | 239,930; 235,900; 236,612 | 236,612 |
| tinystore-sidecar | insert | 1 | 571; 566; 570 | 570 |
| tinystore-sidecar | insert | 8 | 4,305; 4,308; 4,273 | 4,305 |
| tinystore-sidecar | insert | 64 | 28,412; 27,962; 28,395 | 28,395 |
| tinystore-sidecar | mixed | 1 | 2,735; 2,783; 2,791 | 2,783 |
| tinystore-sidecar | mixed | 8 | 18,082; 18,068; 18,115 | 18,082 |
| tinystore-sidecar | mixed | 64 | 113,743; 112,411; 112,898 | 112,898 |
| tinystore-server | get | 1 | 4,169; 4,146; 4,169 | 4,169 |
| tinystore-server | get | 8 | 25,374; 25,370; 25,299 | 25,370 |
| tinystore-server | get | 64 | 63,198; 63,838; 62,896 | 63,198 |
| tinystore-server | insert | 1 | 543; 549; 559 | 549 |
| tinystore-server | insert | 8 | 3,783; 3,904; 3,853 | 3,853 |
| tinystore-server | insert | 64 | 16,759; 16,857; 16,791 | 16,791 |
| tinystore-server | mixed | 1 | 2,453; 2,459; 2,462 | 2,459 |
| tinystore-server | mixed | 8 | 15,440; 15,302; 15,224 | 15,302 |
| tinystore-server | mixed | 64 | 54,326; 55,322; 54,575 | 54,575 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 58.0 | 98.33 |
| tinystore-sidecar | 87.7 | 90.87 |
| tinystore-server | 88.3 | 69.61 |

## sqldb


| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore | get | 1 | 140,591; 142,286; 143,097 | 142,286 |
| tinystore | get | 8 | 313,100; 318,369; 313,338 | 313,338 |
| tinystore | get | 64 | 371,622; 375,647; 364,419 | 371,622 |
| tinystore | insert | 1 | 632; 632; 636 | 632 |
| tinystore | insert | 8 | 4,532; 4,763; 4,787 | 4,763 |
| tinystore | insert | 64 | 32,021; 32,140; 31,985 | 32,021 |
| tinystore | mixed | 1 | 5,899; 6,102; 6,045 | 6,045 |
| tinystore | mixed | 8 | 34,930; 35,018; 35,105 | 35,018 |
| tinystore | mixed | 64 | 137,555; 138,909; 138,655 | 138,655 |
| tinystore-baseline | get | 1 | 131,416; 133,035; 134,999 | 133,035 |
| tinystore-baseline | get | 8 | 304,254; 302,693; 296,810 | 302,693 |
| tinystore-baseline | get | 64 | 335,370; 338,555; 342,857 | 338,555 |
| tinystore-baseline | insert | 1 | 372; 382; 374 | 374 |
| tinystore-baseline | insert | 8 | 2,817; 2,787; 2,777 | 2,787 |
| tinystore-baseline | insert | 64 | 18,412; 17,993; 17,968 | 17,993 |
| tinystore-baseline | mixed | 1 | 3,298; 3,262; 3,438 | 3,298 |
| tinystore-baseline | mixed | 8 | 19,569; 19,366; 19,403 | 19,403 |
| tinystore-baseline | mixed | 64 | 79,795; 79,806; 80,652 | 79,806 |
| sqlite | get | 1 | 100,883; 101,020; 104,249 | 101,020 |
| sqlite | get | 8 | 293,611; 296,403; 299,247 | 296,403 |
| sqlite | get | 64 | 205,389; 210,369; 216,504 | 210,369 |
| sqlite | insert | 1 | 377; 380; 360 | 377 |
| sqlite | insert | 8 | 375; 377; 381 | 377 |
| sqlite | insert | 64 | 377; 386; 376 | 377 |
| sqlite | mixed | 1 | 3,401; 3,396; 3,286 | 3,396 |
| sqlite | mixed | 8 | 3,327; 3,333; 3,592 | 3,333 |
| sqlite | mixed | 64 | 3,352; 3,631; 3,273 | 3,352 |
| postgres | get | 1 | 7,976; 7,902; 7,886 | 7,902 |
| postgres | get | 8 | 59,896; 59,980; 60,296 | 59,980 |
| postgres | get | 64 | 317,820; 320,560; 320,215 | 320,215 |
| postgres | insert | 1 | 609; 613; 603 | 609 |
| postgres | insert | 8 | 2,563; 2,548; 2,570 | 2,563 |
| postgres | insert | 64 | 20,523; 20,445; 20,555 | 20,523 |
| postgres | mixed | 1 | 3,525; 3,568; 3,519 | 3,525 |
| postgres | mixed | 8 | 21,868; 21,906; 21,832 | 21,868 |
| postgres | mixed | 64 | 136,766; 141,683; 141,912 | 141,683 |
| ncruces | get | 1 | 117,322; 117,570; 117,361 | 117,361 |
| ncruces | get | 8 | 333,681; 333,186; 333,194 | 333,194 |
| ncruces | get | 64 | 461,935; 461,949; 460,399 | 461,935 |
| ncruces | insert | 1 | 660; 650; 652 | 652 |
| ncruces | insert | 8 | 640; 647; 649 | 647 |
| ncruces | insert | 64 | 642; 645; 644 | 644 |
| ncruces | mixed | 1 | 6,049; 5,959; 6,027 | 6,027 |
| ncruces | mixed | 8 | 6,070; 6,026; 6,011 | 6,026 |
| ncruces | mixed | 64 | 6,113; 6,172; 6,015 | 6,113 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore | 58.5 | 98.36 |
| tinystore-baseline | 62.0 | 69.10 |
| sqlite | 299.2 | 33.89 |
| postgres | 74.0 | 263.50 |
| ncruces | 217.1 | 35.14 |

## stack

('tinystore-batch', 'request', 256), repeat 1: {'logs_stored': 686779, 'logs_dropped': 0, 'jobs_enqueued': 68839, 'jobs_handled': 63876, 'jobs_waiting': 4963}
('tinystore', 'request', 256), repeat 1: {'logs_stored': 709705, 'logs_dropped': 0, 'jobs_enqueued': 71029, 'jobs_handled': 71016, 'jobs_waiting': 13}
('tinystore-baseline', 'request', 8), repeat 1: {'logs_stored': 24858, 'logs_dropped': 0, 'jobs_enqueued': 2530, 'jobs_handled': 2528, 'jobs_waiting': 2}
('tinystore-baseline', 'request', 64), repeat 1: {'logs_stored': 195218, 'logs_dropped': 0, 'jobs_enqueued': 19410, 'jobs_handled': 19402, 'jobs_waiting': 10}
('tinystore-sidecar', 'request', 64), repeat 1: {'logs_stored': 227941, 'logs_dropped': 0, 'jobs_enqueued': 22699, 'jobs_handled': 22655, 'jobs_waiting': 44}
('tinystore-server', 'request', 256), repeat 1: {'logs_stored': 314066, 'logs_dropped': 0, 'jobs_enqueued': 31342, 'jobs_handled': 31323, 'jobs_waiting': 19}
('services', 'request', 8), repeat 1: {'logs_stored': 48293, 'logs_dropped': 0, 'jobs_enqueued': 4862, 'jobs_handled': 4857, 'jobs_waiting': 5}
('tinystore-server', 'request', 256), repeat 2: {'logs_stored': 313242, 'logs_dropped': 0, 'jobs_enqueued': 31243, 'jobs_handled': 31235, 'jobs_waiting': 8}
('tinystore-sidecar', 'request', 8), repeat 2: {'logs_stored': 32365, 'logs_dropped': 0, 'jobs_enqueued': 3248, 'jobs_handled': 3246, 'jobs_waiting': 2}
('tinystore-batch-baseline', 'request', 256), repeat 2: {'logs_stored': 458932, 'logs_dropped': 0, 'jobs_enqueued': 46135, 'jobs_handled': 45838, 'jobs_waiting': 297}
('tinystore-batch', 'request', 256), repeat 2: {'logs_stored': 677802, 'logs_dropped': 0, 'jobs_enqueued': 68022, 'jobs_handled': 64358, 'jobs_waiting': 3664}
('tinystore-batch', 'request', 256), repeat 3: {'logs_stored': 677584, 'logs_dropped': 0, 'jobs_enqueued': 67991, 'jobs_handled': 62054, 'jobs_waiting': 5937}
('tinystore-batch-baseline', 'request', 256), repeat 3: {'logs_stored': 466986, 'logs_dropped': 0, 'jobs_enqueued': 46926, 'jobs_handled': 44478, 'jobs_waiting': 2448}
('tinystore-server', 'request', 64), repeat 3: {'logs_stored': 158465, 'logs_dropped': 0, 'jobs_enqueued': 15871, 'jobs_handled': 15866, 'jobs_waiting': 5}
('tinystore-server', 'request', 256), repeat 3: {'logs_stored': 312147, 'logs_dropped': 0, 'jobs_enqueued': 31086, 'jobs_handled': 31077, 'jobs_waiting': 14}

| Contender | Stage | Workers | Passes/s | Median/s |
|---|---|---:|---:|---:|
| tinystore-batch | request | 8 | 18,189; 17,875; 18,156 | 18,156 |
| tinystore-batch | request | 64 | 81,531; 81,791; 81,325 | 81,531 |
| tinystore-batch | request | 256 | 113,425; 111,940; 112,533 | 112,533 |
| tinystore-batch | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore-batch | restore | 1 | 0; 0; 0 | 0 |
| tinystore-batch-baseline | request | 8 | 8,900; 8,991; 8,812 | 8,900 |
| tinystore-batch-baseline | request | 64 | 47,948; 47,509; 47,568 | 47,568 |
| tinystore-batch-baseline | request | 256 | 76,838; 76,268; 77,061 | 76,838 |
| tinystore-batch-baseline | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore-batch-baseline | restore | 1 | 0; 0; 0 | 0 |
| tinystore | request | 8 | 7,929; 7,793; 7,836 | 7,836 |
| tinystore | request | 64 | 51,344; 51,976; 52,027 | 51,976 |
| tinystore | request | 256 | 119,743; 119,893; 118,748 | 119,743 |
| tinystore | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore | restore | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | request | 8 | 4,063; 4,194; 4,105 | 4,105 |
| tinystore-baseline | request | 64 | 32,692; 32,471; 31,937 | 32,471 |
| tinystore-baseline | request | 256 | 76,205; 77,005; 75,890 | 76,205 |
| tinystore-baseline | backup-bytes | 1 | 0; 0; 0 | 0 |
| tinystore-baseline | restore | 1 | 0; 0; 0 | 0 |
| tinystore-sidecar | request | 8 | 5,584; 5,626; 5,663 | 5,626 |
| tinystore-sidecar | request | 64 | 37,834; 38,059; 38,179 | 38,059 |
| tinystore-sidecar | request | 256 | 95,019; 94,295; 94,944 | 94,944 |
| tinystore-server | request | 8 | 5,260; 5,191; 5,256 | 5,256 |
| tinystore-server | request | 64 | 26,433; 26,271; 26,437 | 26,433 |
| tinystore-server | request | 256 | 52,048; 51,896; 52,222 | 52,048 |
| services | request | 8 | 8,035; 7,976; 8,013 | 8,013 |
| services | request | 64 | 52,247; 55,625; 54,904 | 54,904 |
| services | request | 256 | 76,314; 79,772; 80,022 | 79,772 |
| services | backup-bytes | 1 | 0; 0; 0 | 0 |
| services | restore | 1 | 0; 0; 0 | 0 |

| Contender | Composite RAM MiB | Final disk MiB |
|---|---:|---:|
| tinystore-batch | 113.7 | 437.54 |
| tinystore-batch-baseline | 145.4 | 279.23 |
| tinystore | 113.8 | 368.24 |
| tinystore-baseline | 147.8 | 237.77 |
| tinystore-sidecar | 158.4 | 288.01 |
| tinystore-server | 152.7 | 176.35 |
| services | 171.5 | 473.06 |

## weight

