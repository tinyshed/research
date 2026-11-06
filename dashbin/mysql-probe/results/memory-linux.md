## mysql, mysql_async
- connected, in its session: peak 3428 kB
- 10000 rows and one more streamed, a row at a time: peak 3480 kB (50005 values)
- the same rows held at once as mysql_async's rows: peak 5868 kB (10001 rows)
## mysql, sqlx
- connected, in its session: peak 1220 kB
- 10000 rows and one more streamed, a row at a time: peak 1256 kB (50005 values)
- the same rows held at once as sqlx's rows: peak 4076 kB (10001 rows)
## mariadb, mysql_async
- connected, in its session: peak 3368 kB
- 10000 rows and one more streamed, a row at a time: peak 3436 kB (50005 values)
- the same rows held at once as mysql_async's rows: peak 5828 kB (10001 rows)
## mariadb, sqlx
- connected, in its session: peak 1196 kB
- 10000 rows and one more streamed, a row at a time: peak 1232 kB (50005 values)
- the same rows held at once as sqlx's rows: peak 4052 kB (10001 rows)
## latency on the compose network, mysql_async told its settings
- connect and authenticate: (580.993µs, 641.574µs)
- a read whole, (b) prepared statement: (1.888229ms, 2.18367ms)
- a read whole, (a) prepared SET, statement as text: (2.699903ms, 2.923674ms)
- connect and authenticate: (532.247µs, 596.97µs)
- a read whole, (b) prepared statement: (1.734739ms, 1.89334ms)
- a read whole, (a) prepared SET, statement as text: (1.775251ms, 1.941694ms)
