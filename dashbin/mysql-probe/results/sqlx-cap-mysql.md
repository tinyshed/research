# the row cap through sqlx on 13306

- 168 000 rows, kill: cut at 10001, 7901 rows drained, then error returned from database: 1317 (70100): Query execution was interrupted; 13.6106ms; close: closed in 68.7µs; nothing of reader's runs after 1.33ms
- endless join, kill: cut at 10001, 39146 rows drained, then error returned from database: 1317 (70100): Query execution was interrupted; 21.9811ms; close: closed in 27.8µs; nothing of reader's runs after 527.8µs
- endless join, dropped without a kill: cut at 10001 after 10.5209ms, connection dropped; nothing of reader's runs after 210.3024ms
