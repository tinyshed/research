# the row cap through sqlx on 13307

- 168 000 rows, kill: cut at 10001, 5166 rows drained, then error returned from database: 1317 (70100): Query execution was interrupted; 11.3947ms; close: closed in 47.9µs; nothing of reader's runs after 1.2719ms
- endless join, kill: cut at 10001, 61492 rows drained, then error returned from database: 1317 (70100): Query execution was interrupted; 22.8048ms; close: closed in 41.3µs; nothing of reader's runs after 1.1403ms
- endless join, dropped without a kill: cut at 10001 after 7.6854ms, connection dropped; nothing of reader's runs after 602.1µs
