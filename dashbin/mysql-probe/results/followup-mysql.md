# the second review's questions on MySql

- sql_mode '': reads as Bytes ""; `SELECT 'x' AS a -- '` → Bytes "x' AS a -- "; `SELECT "y" AS b` → Bytes "y"
- sql_mode 'NO_BACKSLASH_ESCAPES': reads as Bytes "NO_BACKSLASH_ESCAPES"; `SELECT 'x' AS a -- '` → Bytes "x\\"; `SELECT "y" AS b` → Bytes "y"
- sql_mode 'ANSI_QUOTES': reads as Bytes "ANSI_QUOTES"; `SELECT 'x' AS a -- '` → Bytes "x' AS a -- "; `SELECT "y" AS b` → failed: 1054 Unknown column 'y' in 'field list'
- sql_mode 'ANSI': reads as Bytes "REAL_AS_FLOAT,PIPES_AS_CONCAT,ANSI_QUOTES,IGNORE_SPACE,ONLY_FULL_GROUP_BY,ANSI"; `SELECT 'x' AS a -- '` → Bytes "x' AS a -- "; `SELECT "y" AS b` → failed: 1054 Unknown column 'y' in 'field list'
- sql_mode 'ORACLE': refused, 1231 Variable 'sql_mode' can't be set to the value of 'ORACLE'
- sql_mode 'MSSQL': refused, 1231 Variable 'sql_mode' can't be set to the value of 'MSSQL'

- `SET NAMES utf8mb4, SESSION transaction_read_only = ON, time_zone = '+00:00'`: accepted; then Bytes "utf8mb4 1 +00:00"

- `GET_LOCK` in a read: took Bytes "1"; free after ROLLBACK: Bytes "0"; free after the connection closed: Bytes "1"

- `KILL QUERY` sent after its statement ended, five times: ["kill killed, then Bytes \"next statement ran\"", "kill killed, then Bytes \"next statement ran\"", "kill killed, then Bytes \"next statement ran\"", "kill killed, then Bytes \"next statement ran\"", "kill killed, then Bytes \"next statement ran\""]

- a statement killed inside START TRANSACTION READ ONLY: 1317 Query execution was interrupted after 506.14ms; then `SET TRANSACTION READ WRITE` says 1568 Transaction characteristics can't be changed while a transaction is in progress; session read-only Bytes "1"
