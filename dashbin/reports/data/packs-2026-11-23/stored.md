| what | count | ms | each µs |
|---|---|---|---|
| 3 475 icons put, one transaction | 3475 | 282.2 | 81.2 |
| icons put, a transaction each | 200 | 311.4 | 1557.0 |
| country.bin put, 5045 KiB | 1 | 9.2 | 9155.0 |
| asn.bin put, 7739 KiB | 1 | 11.8 | 11762.0 |
| domains.tsv put, 1046 KiB | 1 | 5.4 | 5353.5 |
| the store's directory, KiB | 27066 | | |
| icons read whole, 4883 KiB | 3475 | 25.2 | 7.3 |
| the store opened again | 1 | 0.1 | 92.5 |
| country.bin read whole, its SHA-256 checked | 1 | 3.0 | 3022.0 |
| country.bin made a table from those bytes | 1 | 0.6 | 614.2 |
| asn.bin read whole, its SHA-256 checked | 1 | 4.4 | 4378.0 |
| asn.bin made a table from those bytes | 1 | 1.0 | 989.3 |
| domains inserted as rows | 40843 | 79.1 | 1.9 |
| v4 countries inserted as rows | 362998 | 666.9 | 1.8 |
| the store's directory with the rows, KiB | 37281 | | |
| a domain looked up in rows, 80000 found | 100000 | 829.2 | 8.3 |
| a v4 address looked up in rows, 100000 found | 100000 | 353.2 | 3.5 |
