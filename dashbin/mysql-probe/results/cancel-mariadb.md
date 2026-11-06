# stopping a statement on MariaDb

- the server's own timeout, 2 s: `SELECT COUNT(*) FROM probe.big a, probe.big b WHERE a.amount + b.amount < 0`: 1969 Query execution was interrupted (max_statement_time exceeded) after 2.00s
- the statement's own longer timeout, the server's 2 s, our kill at 6 s: `SET STATEMENT max_statement_time = 600 FOR SELECT COUNT(*) FROM probe.big a, probe.big b WHERE a.amount + b.amount < 0`: 1317 Query execution was interrupted after 6.02s
  kill as reader, who has no PROCESS or CONNECTION_ADMIN: (3.2998ms, "killed")
- reader kills writer's statement: (3.3192ms, "1095 You are not owner of thread 1696")
- writer kills its own: (2.4484ms, "killed")
  writer's statement: 1317 Query execution was interrupted after 515.51ms
- a client killed while its statement runs (60 s timeout): running before ["[Int(1699), Int(2), Bytes(\"Sending ..\"), Bytes(\"SELECT C..\")]"]; still running after 15 s: ["[Int(1699), Int(17), Bytes(\"Sending ..\"), Bytes(\"SELECT C..\")]"]
- a future dropped while its statement runs (60 s timeout): still running after 15 s: ["[Int(1718), Int(16), Bytes(\"Sending ..\"), Bytes(\"SELECT C..\")]", "[Int(1699), Int(33), Bytes(\"Sending ..\"), Bytes(\"SELECT C..\")]"]
- `SELECT SLEEP(5)` under a 1 s timeout: 1969 Query execution was interrupted (max_statement_time exceeded) after 1.00s → Bytes "session alive"
- `SHOW TABLES` timeout does not matter; `SELECT BENCHMARK(100000000, MD5('x'))` under 1 s: 1 rows after 1.00s
