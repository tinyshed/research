# stopping a statement on MySql

- the server's own timeout, 2 s: `SELECT COUNT(*) FROM probe.big a, probe.big b WHERE a.amount + b.amount < 0`: 3024 Query execution was interrupted, maximum statement execution time exceeded after 2.00s
- the statement's own longer timeout, the server's 2 s, our kill at 6 s: `SELECT /*+ MAX_EXECUTION_TIME(600000) */ COUNT(*) FROM probe.big a, probe.big b WHERE a.amount + b.amount < 0`: 1317 Query execution was interrupted after 6.02s
  kill as reader, who has no PROCESS or CONNECTION_ADMIN: (3.1241ms, "killed")
- reader kills writer's statement: (3.2758ms, "1095 You are not owner of thread 900")
- writer kills its own: (2.1456ms, "killed")
  writer's statement: 1317 Query execution was interrupted after 521.97ms
- a client killed while its statement runs (60 s timeout): running before ["[Int(903), Int(2), Bytes(\"executin..\"), Bytes(\"SELECT C..\")]"]; still running after 15 s: ["[Int(903), Int(17), Bytes(\"executin..\"), Bytes(\"SELECT C..\")]"]
- a future dropped while its statement runs (60 s timeout): still running after 15 s: ["[Int(913), Int(16), Bytes(\"executin..\"), Bytes(\"SELECT C..\")]", "[Int(903), Int(33), Bytes(\"executin..\"), Bytes(\"SELECT C..\")]"]
- `SELECT SLEEP(5)` under a 1 s timeout: 1 rows after 1.00s → Bytes "session alive"
- `SHOW TABLES` timeout does not matter; `SELECT BENCHMARK(100000000, MD5('x'))` under 1 s: 1 rows after 1.00s
