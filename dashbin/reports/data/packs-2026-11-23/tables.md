| table | rows | file KiB | load ms | held KiB | lookup ns | found |
|---|---|---|---|---|---|---|
| domains, a hash map | 40843 | 1046 | 5.8 | 2293 | 94 | 80% |
| domains, one sorted text | 40843 | 1046 | 0.6 | 1302 | 790 | 80% |
| domains, one text and slots | 40843 | 1046 | 3.0 | 1091 | 157 | 80% |
| country, sorted ranges | 732796 | 31724 | 69.2 | 17259 | 38 | 100% |
| country, starts alone, from text | 732796 | 31724 | 82.1 | 5363 | 39 | 100% |
| country, starts alone, from bytes | 732796 | 5045 | 1.3 | 5054 | 39 | 100% |
| asn, sorted ranges | 485292 | 30134 | 70.0 | 12191 | 38 | 36% |
| asn, starts alone, from text | 485292 | 30134 | 79.3 | 8492 | 45 | 36% |
| asn, starts alone, from bytes | 485292 | 7739 | 2.1 | 7742 | 45 | 36% |
