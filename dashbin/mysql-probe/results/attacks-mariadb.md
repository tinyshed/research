# attacks on MariaDb: state before [Bytes("1"), Bytes("0"), Bytes("0"), Bytes("2026-11-.."), Bytes("2026-11-.."), Bytes("2"), Bytes("151")]

| attack | statement | prepared | writer: ran | changed | reader: ran |
|---|---|---|---|---|---|
| select (control) | `SELECT x FROM probe.victim` | 1 columns | ran, 1 rows, last [Int(1)] | no | ran, 1 rows, last [Int(1)] |
| create table | `CREATE TABLE probe.t_new (x INT)` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| create table as select | `CREATE TABLE probe.t_new AS SELECT 1 AS x` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| drop table | `DROP TABLE probe.victim` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| truncate | `TRUNCATE TABLE probe.victim` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| rename table | `RENAME TABLE probe.victim TO probe.victim2` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| create procedure | `CREATE PROCEDURE probe.p2() SELECT 1` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| insert | `INSERT INTO probe.victim VALUES (9)` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| update | `UPDATE probe.victim SET x = 9` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| delete returning | `DELETE FROM probe.victim RETURNING x` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| insert returning | `INSERT INTO probe.victim VALUES (9) RETURNING x` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| two statements | `SELECT 1; DROP TABLE probe.victim` | refused: 1064 You have an error in your SQL syntax; check the manual that corresponds to your MariaDB server version for the right syntax to use near 'DROP TABLE probe.victim' at line 1 |  | no |  |
| drop in /*! */ | `SELECT 1 /*! ; DROP TABLE probe.victim */` | refused: 1064 You have an error in your SQL syntax; check the manual that corresponds to your MariaDB server version for the right syntax to use near 'DROP TABLE probe.victim */' at line 1 |  | no |  |
| function that writes | `SELECT probe.writes()` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| procedure: commit, read-write, insert | `CALL probe.escapes()` | 0 columns | ran, 1 rows, last [Bytes("escaped")] | **[Bytes("1,3"), Bytes("0"), Bytes("0"), Bytes("2026-11-.."), Bytes("2026-11-.."), Bytes("2"), Bytes("151")]** | failed: 1370 execute command denied to user 'reader'@'%' for routine 'probe.escapes' |
| session read-write | `SET SESSION transaction_read_only = OFF` | 0 columns | ran, 0 rows | no | ran, 0 rows |
| set transaction read write | `SET TRANSACTION READ WRITE` | 0 columns | failed: 1568 Transaction characteristics can't be changed while a transaction is in progress | no | failed: 1568 Transaction characteristics can't be changed while a transaction is in progress |
| start transaction read write | `START TRANSACTION READ WRITE` | 0 columns | ran, 0 rows | no | ran, 0 rows |
| commit | `COMMIT` | 0 columns | ran, 0 rows | no | ran, 0 rows |
| select for update | `SELECT x FROM probe.victim FOR UPDATE` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| lock in share mode | `SELECT x FROM probe.victim LOCK IN SHARE MODE` | 1 columns | ran, 1 rows, last [Int(1)] | no | ran, 1 rows, last [Int(1)] |
| lock tables | `LOCK TABLES probe.victim WRITE` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1044 Access denied for user 'reader'@'%' to database 'probe' |
| named lock | `SELECT GET_LOCK('probe', 0)` | 1 columns | ran, 1 rows, last [Int(1)] | no | ran, 1 rows, last [Int(1)] |
| into outfile | `SELECT x FROM probe.victim INTO OUTFILE '/tmp/victim.txt'` | refused: 1227 Access denied; you need (at least one of) the FILE privilege(s) for this operation |  | no |  |
| into variable | `SELECT x INTO @x FROM probe.victim` | 0 columns | ran, 0 rows | no | ran, 0 rows |
| user variable := | `SELECT @x := 5` | 1 columns | ran, 1 rows, last [Int(5)] | no | ran, 1 rows, last [Int(5)] |
| temporary table | `CREATE TEMPORARY TABLE probe.scratch (x INT)` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no |  |
| analyze table | `ANALYZE TABLE probe.victim` | 4 columns | ran, 1 rows, last [Bytes("probe.vi.."), Bytes("analyze"), Bytes("status"), Bytes("OK")] | no | failed: 1142 INSERT command denied to user 'reader'@'172.28.0.1' for table `probe`.`victim` |
| optimize table | `OPTIMIZE TABLE probe.victim` | 4 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| repair table | `REPAIR TABLE probe.victim` | 4 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| check table | `CHECK TABLE probe.victim` | 4 columns | ran, 1 rows, last [Bytes("probe.vi.."), Bytes("check"), Bytes("status"), Bytes("OK")] | no | ran, 1 rows, last [Bytes("probe.vi.."), Bytes("check"), Bytes("status"), Bytes("OK")] |
| checksum table | `CHECKSUM TABLE probe.victim` | 2 columns | ran, 1 rows, last [Bytes("probe.vi.."), Int(3459908756)] | no | ran, 1 rows, last [Bytes("probe.vi.."), Int(3459908756)] |
| cache index | `CACHE INDEX probe.victim IN default` | 4 columns | ran, 1 rows, last [Bytes("probe.vi.."), Bytes("assign_t.."), Bytes("note"), Bytes("The stor..")] | no | ran, 1 rows, last [Bytes("probe.vi.."), Bytes("assign_t.."), Bytes("note"), Bytes("The stor..")] |
| flush tables | `FLUSH TABLES` | 0 columns | failed: 1227 Access denied; you need (at least one of) the RELOAD privilege(s) for this operation | no | failed: 1227 Access denied; you need (at least one of) the RELOAD privilege(s) for this operation |
| kill | `KILL 999999` | 0 columns | failed: 1094 Unknown thread id: 999999 | no | failed: 1094 Unknown thread id: 999999 |
| set global | `SET GLOBAL max_connections = 99` | refused: 1227 Access denied; you need (at least one of) the CONNECTION ADMIN privilege(s) for this operation |  | no |  |
| set password | `SET PASSWORD = 'changed-by-probe'` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| alter own user | `ALTER USER CURRENT_USER() IDENTIFIED BY 'changed-by-probe'` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| grant | `GRANT SELECT ON probe.* TO reader` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| create user | `CREATE USER sneaky IDENTIFIED BY 'x'` | 0 columns | failed: 1792 Cannot execute statement in a READ ONLY transaction | no | failed: 1792 Cannot execute statement in a READ ONLY transaction |
| prepare inside | `PREPARE s FROM 'DROP TABLE probe.victim'` | refused: 1295 This command is not supported in the prepared statement protocol yet |  | no |  |
| do | `DO SLEEP(0)` | 0 columns | ran, 0 rows | no | ran, 0 rows |
| handler open | `HANDLER probe.victim OPEN` | 0 columns | ran, 0 rows | no | ran, 0 rows |
| xa start | `XA START 'probe'` | 0 columns | failed: 1400 XAER_OUTSIDE: Some work is done outside global transaction | no | failed: 1400 XAER_OUTSIDE: Some work is done outside global transaction |
| signal | `SIGNAL SQLSTATE '45000'` | 0 columns | failed: 1644 Unhandled user-defined exception condition | no | failed: 1644 Unhandled user-defined exception condition |
| load data local | `LOAD DATA LOCAL INFILE '/etc/hostname' INTO TABLE probe.victim` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| load_file | `SELECT LOAD_FILE('/etc/hostname') IS NOT NULL` | 1 columns | ran, 1 rows, last [Int(0)] | no | ran, 1 rows, last [Int(0)] |
| sequence nextval | `SELECT NEXTVAL(probe.seq)` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| set statement ... for select | `SET STATEMENT max_statement_time = 0 FOR SELECT 1` | 1 columns | ran, 1 rows, last [Int(1)] | no | ran, 1 rows, last [Int(1)] |
| set statement read-write for delete returning | `SET STATEMENT transaction_read_only = 0 FOR DELETE FROM probe.victim RETURNING x` | refused: 1792 Cannot execute statement in a READ ONLY transaction |  | no |  |
| hint set_var read-write | `SELECT /*+ SET_VAR(transaction_read_only = OFF) */ 1` | 1 columns | ran, 1 rows, last [Int(1)] | no | ran, 1 rows, last [Int(1)] |
| show | `SHOW TABLES FROM probe` | 1 columns | ran, 5 rows, last [Bytes("victim")] | no | ran, 5 rows, last [Bytes("victim")] |
| explain analyze | `EXPLAIN ANALYZE SELECT x FROM probe.victim` | refused: 1064 You have an error in your SQL syntax; check the manual that corresponds to your MariaDB server version for the right syntax to use near 'ANALYZE SELECT x FROM probe.victim' at line 1 |  | no |  |
| analyze select | `ANALYZE SELECT x FROM probe.victim` | 13 columns | ran, 1 rows, last [Int(1), Bytes("SIMPLE"), Bytes("victim"), Bytes("ALL"), Null, Null, Null, Null, Bytes("1"), Bytes("1.00"), Double(100.0), Double(100.0), Bytes("")] | no | ran, 1 rows, last [Int(1), Bytes("SIMPLE"), Bytes("victim"), Bytes("ALL"), Null, Null, Null, Null, Bytes("1"), Bytes("1.00"), Double(100.0), Double(100.0), Bytes("")] |
| table | `TABLE probe.victim` | refused: 1064 You have an error in your SQL syntax; check the manual that corresponds to your MariaDB server version for the right syntax to use near 'TABLE probe.victim' at line 1 |  | no |  |
| values | `VALUES ROW(1)` | refused: 1064 You have an error in your SQL syntax; check the manual that corresponds to your MariaDB server version for the right syntax to use near 'ROW(1)' at line 1 |  | no |  |
| with | `WITH v AS (SELECT x FROM probe.victim) SELECT x FROM v` | 1 columns | ran, 1 rows, last [Int(1)] | no | ran, 1 rows, last [Int(1)] |
| help | `HELP 'select'` | 3 columns | ran, 1 rows, last [Bytes("SELECT"), Bytes("Syntax\\n-.."), Bytes("")] | no | ran, 1 rows, last [Bytes("SELECT"), Bytes("Syntax\\n-.."), Bytes("")] |
| shutdown | `SHUTDOWN` | 0 columns | failed: 1227 Access denied; you need (at least one of) the SHUTDOWN privilege(s) for this operation | no | failed: 1227 Access denied; you need (at least one of) the SHUTDOWN privilege(s) for this operation |

text protocol, `SELECT 1 AS one; DROP TABLE probe.victim` as writer: set of 1 rows; state [Bytes("1"), Bytes("0"), Bytes("0"), Bytes("2026-11-.."), Bytes("2026-11-.."), Bytes("2"), Bytes("151")]
