## mysql
- `EXPLAIN SELECT * FROM probe.big`: 0 columns at prepare
- `EXPLAIN FORMAT=JSON SELECT * FROM probe.big`: 0 columns at prepare
- `DESCRIBE probe.big`: 6 columns at prepare
- `SHOW CREATE TABLE probe.big`: 0 columns at prepare
- `SHOW COLUMNS FROM probe.big`: 6 columns at prepare
- `SELECT * FROM information_schema.columns WHERE table_schema = 'probe'`: 22 columns at prepare
- `SELECT @@version`: 1 columns at prepare
## mariadb
- `EXPLAIN SELECT * FROM probe.big`: 10 columns at prepare
- `EXPLAIN FORMAT=JSON SELECT * FROM probe.big`: 1 columns at prepare
- `DESCRIBE probe.big`: 6 columns at prepare
- `SHOW CREATE TABLE probe.big`: 2 columns at prepare
- `SHOW COLUMNS FROM probe.big`: 6 columns at prepare
- `SELECT * FROM information_schema.columns WHERE table_schema = 'probe'`: 24 columns at prepare
- `SELECT @@version`: 1 columns at prepare
