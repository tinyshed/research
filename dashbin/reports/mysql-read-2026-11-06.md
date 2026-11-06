# What a read-only MySQL read can promise, and through which driver

**6 November 2026.** Slice 6 is MySQL as Dashbin's second plugin. Before a
line of it is written, a second review asked for measurements of what
PostgreSQL's plugin promises and MySQL may not: a row cap the server keeps, a
read-only transaction that is a sandbox, parameters bound without losing
precision, a stop that stops the server, zones, the driver's weight. Measured
with [mysql-probe](../mysql-probe/) on MySQL 8.4.13 and MariaDB 11.8.9, both
from their official images in `compose.yaml`, on Windows 11 through Docker
Desktop's loopback; `results/` holds every run's output as it printed.

Each section says what the servers' documentation promises, what the probe
saw, and what is left open. The last section is the recommendation.

## The driver

| Property                                     | mysql_async 0.37.1, `minimal-rust`                            | sqlx-mysql 0.9.0, `rsa`; sqlx-core `_rt-tokio`, `_tls-none`          |
|----------------------------------------------|---------------------------------------------------------------|----------------------------------------------------------------------|
| Dashbin's static binary, the driver used     | **+2 011 328 bytes**, 9 998 544                               | **+430 496 bytes**, 8 417 712                                        |
| A value of any column, binary protocol       | decoded into `Value`: `Int`, `UInt`, `Double`, `Date`, …      | text types only; every other type its packed bytes, or a feature     |
| A column's type                              | its code, flags and digits of a second                        | its name: `BIGINT UNSIGNED`, `DATETIME`, `VARBINARY`                 |
| `caching_sha2_password`, empty cache, no TLS | signs in: its own RSA, in mysql_common                        | refused until its `rsa` feature, the `rsa` crate                     |
| Its own session settings                     | none                                                          | `PIPES_AS_CONCAT`, `IGNORE_SPACE` unless off: `'a' \|\| 'b'` is `ab` |
| A statement written at run time              | `&str`                                                        | wrapped in `AssertSqlSafe` first                                     |
| The connection's id, for a kill              | `Conn::id()`                                                  | `SELECT CONNECTION_ID()`, a command more                             |
| Connect and sign in, median of 100           | 1.58 ms; 1.10 ms told `max_allowed_packet` and `wait_timeout` | 1.06 ms                                                              |
| A read whole, median of 100                  | 4.73 ms                                                       | 3.92 ms                                                              |
| Dropped with a result pending                | drains it in a spawned task: the server sends on for 10 s     | closes the socket: the server stops sending in 0.2–0.6 s             |
| Peak memory, connected; 10 001 rows streamed | 3 428 kB; 3 480 kB                                            | 1 220 kB; 1 256 kB                                                   |

The binaries are dashbin at `3b83c5c` with each driver making a read as a
plugin would: connect, prepare, read every value, kill, built in
`dashbin-musl:1.99.1` as `just size` builds (`weigh/`); dashbin alone is
7 987 216 bytes, as CI measures it. cargo-bloat puts 571 KiB of
mysql_async's code in `regex` and its two crates, which mysql_common uses to
read the server's version at every sign-in, and regex's Unicode tables are
among its data; mysql_async and mysql_common themselves are 253 KiB.
sqlx-mysql is 129 KiB of code, sqlx-core 38 KiB, `rsa` and `crypto-bigint`
88 KiB.

mysql_async asks the server `SELECT @@max_allowed_packet, @@wait_timeout` after
signing in unless both are given; given, its connection takes what sqlx's
does. The read whole is a connection, the session, a prepared statement, a
rollback and goodbye, on MySQL from the Windows host through Docker's port;
MariaDB's are within 0.3 ms, and inside Docker's network the same read takes
1.9 ms. The memory is each probe's peak on Linux (`results/memory-linux.md`);
mysql_async's 2 MB more is there before the first row.

## Parameters: (a) a user variable set apart, (b) a placeholder

(a) prepares `SET @p = …` with the value apart and sends the statement as
text, so `@from` stays MySQL's own variable and no lexer rewrites it. (b)
writes `?` where `@from` stood and executes the statement prepared, the value
apart. Each was asked whether a stored value equals itself and its neighbour
one unit in the last place: right is 1 and 0, and 1 and 1 means the
comparison went through `DOUBLE`.

| Value             | Binding                                  | MySQL 8.4 | MariaDB 11.8 |
|-------------------|------------------------------------------|-----------|--------------|
| `DECIMAL(65,30)`  | (a) `SET @p = ?`, text                   | **1 1**   | 1 0          |
| `DECIMAL(65,30)`  | (a) `SET @p = CAST(? AS DECIMAL(65,30))` | 1 0       | 1 0          |
| `DECIMAL(65,30)`  | (b) `= ?`, text                          | 1 0       | 1 0          |
| `DECIMAL(65,30)`  | (b) `= CAST(? AS DECIMAL(65,30))`        | 1 0       | 1 0          |
| `BIGINT UNSIGNED` | (a) `SET @p = ?`, text                   | **1 1**   | 1 0          |
| `BIGINT UNSIGNED` | (a) `SET @p = CAST(? AS UNSIGNED)`       | **0 0**   | 1 0          |
| `BIGINT UNSIGNED` | (b) `= ?`, text                          | 1 0       | 1 0          |
| `BIGINT UNSIGNED` | (b) `= CAST(? AS UNSIGNED)`              | **0 0**   | 1 0          |
| `BIGINT UNSIGNED` | (b) `= ?`, the value as `UInt`           | 1 0       | 1 0          |

- **Measured**: (a) with a text value loses precision on MySQL, as its manual
  says of a comparison between a string and a number; (b) with a text value is
  exact on both, the server typing the placeholder from what it is compared
  with. `CAST('18446744073709551615' AS UNSIGNED)` on MySQL equals neither
  value; Dashbin's `int` is an `i64`, so `CAST(? AS SIGNED)` is the cast a
  plugin needs, and this stays unexplained.
- **Documented, and seen**: a user variable holds no temporal type. (a)'s
  `SET @p = CAST(? AS DATETIME(6))` comes back as the text
  `2026-10-02 00:00:00.000000`; (b)'s `CAST(? AS DATETIME(6))` comes back a
  `DATETIME`, and `+ INTERVAL 1 DAY` keeps it one.
- A day of `big` counts 5 760 rows through every binding, and every plan uses
  the index on `created_at`: no binding loses it.
- (a) is one command more than (b): 5.56 ms against 4.73 ms a read on MySQL,
  4.92 against 4.45 on MariaDB.
- **Measured**: `CAST(? AS DECIMAL(4,2))` rounds `12.345` to `12.35` and
  clamps `123.45` to `99.99` without an error, so a decimal's precision and
  scale come from its own digits.

## Values: text protocol and binary

Every column of a table of edges was read through both protocols, on both
servers (`results/types-*.md`): `DECIMAL(65,30)` at both ends, `BIGINT
UNSIGNED` at its maximum, `FLOAT` and `DOUBLE` at theirs, `DATE`, `DATETIME`
at 0, 3 and 6 digits, `TIMESTAMP(6)`, `TIME` at ±838 hours, `YEAR`, utf8mb4
and latin1 text, `VARBINARY`, `BIT(10)`, `JSON`, `ENUM`, `SET`, `POINT`, and a
row of nulls.

- **Measured**: the two protocols give the same values for every column on
  both servers. The binary one gives integers, floats, dates and times typed,
  with each column's digits of a second in its definition; `DECIMAL`, `JSON`,
  `ENUM`, `SET`, `BIT` and geometry come as bytes in both.
- MySQL's type set is closed and every type has a binary form, so the reason
  PostgreSQL's plugin asks for text, a type with no decoder, does not arise.
- MariaDB sends `JSON` as `BLOB`: without MariaDB's extended metadata a
  column does not say it is JSON. Its `UUID` and `INET6` were not tried.
- A `FLOAT` read as `f32` is `0.1`; widened to `f64` it is
  `0.10000000149011612`, so it goes through its shortest text.
- `0000-00-00`, which `NO_ZERO_DATE` keeps out by default and old data holds,
  was not tried.

## One statement, read-only

The probe ran 57 statements that change something, or try to leave the
transaction, and a plain `SELECT` beside them, as a read would run them: a
session with `transaction_read_only = ON`, `START TRANSACTION READ ONLY`, the
statement prepared and executed, `ROLLBACK`. Each ran as a user with every
right on the database and as a user with `SELECT` alone
(`results/attacks-*.md`).

- **One statement is the server's**: the prepared-statement protocol refuses
  two on both servers, and `/*! … */` is read as code inside it.
- **Measured**: the read-only session and transaction refused every change to
  data and schema on both: no implicit commit escaped, since the session is
  read-only too; `RETURNING`, a function that writes, `FOR UPDATE`, accounts,
  sequences, and MariaDB's `SET STATEMENT` and MySQL's `SET_VAR` hint over
  `transaction_read_only` were all refused.
- **Measured**: one statement left the transaction: `CALL` of a procedure
  that commits, switches its session to read-write and writes. It needs
  `EXECUTE`; the `SELECT`-only user was refused it on both.
- **Measured**: what else ran for the user with every right had no effect on
  data: `ANALYZE TABLE` rewrote statistics on MariaDB, `GET_LOCK` took a lock
  that ends with the session, `HANDLER … OPEN` opened one on MariaDB.
- **Measured**: a statement's column count at prepare does not say it is a
  read: MySQL says 0 for `EXPLAIN` and `SHOW CREATE TABLE`.

## The row cap

10 000 rows and one more, through mysql_async (`results/cap-*.md`).

| Statement                              | Stop                          | MySQL 8.4                       | MariaDB 11.8                  |
|----------------------------------------|-------------------------------|---------------------------------|-------------------------------|
| `SELECT * FROM big`, 168 000 rows      | `sql_select_limit`            | server stops at 10 001, 12 ms   | server stops at 10 001, 11 ms |
| the same                               | count, `KILL QUERY`           | 7 288 more in flight, 14 ms     | 6 096 more, 13 ms             |
| `… LIMIT 1000000`                      | `sql_select_limit`, then kill | the limit does not apply; 15 ms | the same; 16 ms               |
| `… ORDER BY note`                      | `sql_select_limit`            | 81 ms                           | 78 ms                         |
| the same                               | count, `KILL QUERY`           | 93 ms                           | 80 ms                         |
| a join without end, `LIMIT` of its own | count, `KILL QUERY`           | 76 595 more in flight, 35 ms    | 61 492 more, 24 ms            |
| the same                               | dropped, no kill              | **still sending after 10 s**    | **still sending after 10 s**  |

- **Measured, as GPT said**: `sql_select_limit` stops the server only for a
  statement without a `LIMIT` of its own.
- **Measured**: a count in the driver and `KILL QUERY` from a second
  connection stopped the server every time; the kill took 2–3 ms, and what
  was already in flight is read and dropped.
- **Measured**: dropping the connection instead left the statement running:
  mysql_async's connection, dropped, reads its pending result to the end in a
  task of its own. sqlx's closes its socket, and the server ended the
  sending statement within 0.6 s; one that computes before it sends would
  not notice, as a lost client shows below.
- **Measured**: through sqlx the same count and kill cut 168 000 rows in
  11–14 ms and the join without end in 22–23 ms, on both servers.

## Stopping a statement

`SELECT COUNT(*)` of a join of `big` with itself, which runs for hours
(`results/cancel-*.md`).

- **Measured**: the server's own bound, `max_execution_time` on MySQL and
  `max_statement_time` on MariaDB, ended it at 2.00 s on both.
- **Measured**: the statement can lift that bound itself: MySQL's hint
  `/*+ MAX_EXECUTION_TIME(600000) */` and MariaDB's prefix `SET STATEMENT
  max_statement_time = 600 FOR` both did; only the probe's own kill at 6 s
  stopped them.
- **Measured**: `KILL QUERY` of one's own statement needs no `PROCESS` or
  `CONNECTION_ADMIN` on either server; another user's is refused, 1095.
- **Measured**: a client process killed mid-statement, and a future dropped
  mid-statement, both left it running for the 15 s watched, on both servers.
- **Measured**: `SELECT SLEEP(5)` under a 1 s bound answers a row on MySQL
  and an error on MariaDB; `BENCHMARK` answers a row on both. A bound that
  ends a function is not always an error.

## Zones

- **Measured**: both images load the zone tables, 1 795 names and 498, and
  `CONVERT_TZ` to a name the tables lack answers `NULL` without an error. The
  `SELECT`-only user cannot read `mysql.time_zone_name`, so asking
  `CONVERT_TZ` is how a plugin finds out.
- **Documented**: a server installed from a distribution's package has no
  zone tables until `mysql_tzinfo_to_sql` loads them.
- **Measured**: `CONVERT_TZ` puts both 00:30 and 01:30 UTC of 25 October at
  02:30 in Berlin, as the change from summer time does.
- **Measured**: a `TIMESTAMP` is printed, and sent in binary, in the session's
  zone; a `DATETIME` is not. Under `Europe/Moscow` the same row's `TIMESTAMP`
  read 07:05:45 and its `DATETIME` 04:05:45. With `time_zone = '+00:00'` a
  `TIMESTAMP` is an instant and a `DATETIME` a wall clock, as `timestamptz`
  and `timestamp` are on PostgreSQL. Arithmetic on a `TIMESTAMP` gives a
  `DATETIME`.

## TLS

Not built. With `rustls-tls`, `ring` and `tls12`, mysql_async's binary grew by
1 445 920 bytes more, to 11 444 464; most of that is rustls, which
PostgreSQL's plugin would link as well, so it is the price of TLS more than
of MySQL. mysql_async signs in to MySQL 8.4's `caching_sha2_password` over
plain TCP with an empty cache through its own RSA, sqlx through its `rsa`
feature. RUSTSEC-2023-0071, the Marvin timing attack on the `rsa` crate,
concerns decrypting with a private key; a client only encrypts its password
with the server's public key. Dashbin's CI runs no advisory check.

## After the second review

The recommendation went to a second review, which asked four things more;
measured the same day (`results/followup-*.md`).

- **Measured: the lexer has to know the session's `sql_mode`.** The same text
  `SELECT 'x\' AS a -- '` is one string, `x' AS a -- `, by default and with
  `ANSI_QUOTES`, and a string `x\` and a comment with `NO_BACKSLASH_ESCAPES`;
  `"y"` is a string by default and a name with `ANSI_QUOTES`. `ANSI` is shown
  with `ANSI_QUOTES` spelt out among its parts, on both servers. MariaDB's
  `ORACLE` and `MSSQL` modes carry `ANSI_QUOTES` too and change its grammar
  further; MySQL refuses both.
- **Measured**: a `SET_VAR(sql_mode = …)` hint does not change how its own
  statement is read: the backslash still escaped. A
  `SET_VAR(max_execution_time = 600000)` hint does lift the session's bound of
  1 s: the statement ran until killed at 4 s.
- **Measured: a named lock outlives the transaction, not the connection.**
  `GET_LOCK` in a read was still held after its `ROLLBACK` and free once its
  connection closed, on both servers. A pooled connection would keep it.
- **Documented**: `SELECT … INTO OUTFILE` and `INTO DUMPFILE` write a file on
  the server for a user with `FILE`, which no transaction undoes.
- **Measured: a kill does not linger.** `KILL QUERY` sent after its statement
  ended left the next statement on that connection to run, five times of five
  on both. A kill meant for one statement can still meet another if the
  other was sent before the kill landed.
- **Measured: a killed statement leaves its transaction open.** After
  `KILL QUERY` ended a statement inside `START TRANSACTION READ ONLY`,
  `SET TRANSACTION READ WRITE` was refused as changing a transaction in
  progress, on both.
- **Measured**: `SET NAMES utf8mb4, SESSION transaction_read_only = ON,
  time_zone = '+00:00'` is accepted as one statement on both, and it is not
  needed: sqlx's handshake alone leaves the connection in `utf8mb4`, and text
  in Cyrillic, an emoji and latin1's `café` read whole.
- **Documented**: `caching_sha2_password`'s RSA hides the password on plain
  TCP; every statement and row after it crosses the network in the clear.

## What it decides

The simplest plugin that keeps PostgreSQL's promises on both servers:

1. **sqlx-mysql 0.9**, `default-features = false, features = ["rsa"]`, and
   sqlx-core with `_rt-tokio` and `_tls-none`: 430 496 bytes, where
   mysql_async is 2 011 328, a quarter of the binary; and sqlx closes a
   dropped connection where mysql_async keeps reading it. What sqlx does not
   do is the plugin's: its session settings turned off (`pipes_as_concat`,
   `no_engine_substitution`, `timezone`, `set_names`) so that the session is
   ours alone; each statement wrapped in `AssertSqlSafe`; the connection's id
   and `@@sql_mode` asked in one `SELECT`; and the binary protocol's integers,
   floats, dates and times decoded from their bytes, chosen by the column's
   type name, while every other value comes as the bytes it is. A
   mysql_async whose mysql_common read the version by hand would weigh
   perhaps 0.8 MB, an estimate from cargo-bloat, with a vendored crate to
   keep.
2. **(b): a placeholder, the value apart as text, a cast by its kind**, as
   PostgreSQL's `($1::timestamptz)`: `text` as `?`, `int` as
   `CAST(? AS SIGNED)`, `float` as `CAST(? AS DOUBLE)`, `decimal` as
   `CAST(? AS DECIMAL(p,s))` with `p` and `s` from its own digits, `date` as
   `CAST(? AS DATE)`, `time` as `CAST(? AS DATETIME(6))` in UTC, `bool` as 1
   or 0 `CAST(? AS SIGNED)`. The casts of `DOUBLE` and of a boolean were not
   run; both servers document `CAST(… AS DOUBLE)`.
3. **The binary protocol into a frame, no text between**:

   | MySQL                        | Column                                          |
   |------------------------------|-------------------------------------------------|
   | `TINYINT` … `BIGINT`, `YEAR` | `Int`                                           |
   | `BIGINT UNSIGNED`            | `Int`; `Decimal` once a value passes `i64`      |
   | `FLOAT`, `DOUBLE`            | `Float`, a `FLOAT` through its shortest text    |
   | `DECIMAL`                    | `Decimal`; `Text` past 38 digits, as `numeric`  |
   | `DATE`                       | `Date`                                          |
   | `DATETIME`                   | `Wall`                                          |
   | `TIMESTAMP`                  | `Time`, the session being UTC                   |
   | `TIME`                       | `Text`, as MySQL prints it, `-838:59:59.000000` |
   | text, `ENUM`, `SET`, `JSON`  | `Text`                                          |
   | binary, `BIT`, geometry      | `Text` as hex, `0x02AA`                         |

4. **A session set up, then the statement**: `SELECT CONNECTION_ID(),
   @@sql_mode`; `SET SESSION transaction_read_only = ON, time_zone =
   '+00:00', max_execution_time = …` (`max_statement_time` on MariaDB)
   `, sql_select_limit = <cap + 1>`, after any setting of the user's;
   `START TRANSACTION READ ONLY`; the statement prepared and executed;
   `ROLLBACK`. One statement is then the server's check, not a parser of
   ours. Through sqlx that is the handshake and six commands that wait; no
   fewer is promised.
5. **The cap and every stop are a count and a kill, on a connection that
   is the read's alone**: a connection is opened for one read, never pooled
   and never lent, so its session state, named locks among it, ends with the
   read. The driver reads the cap and one row more, and on that row, on
   Dashbin's own timeout or on a cancel, sends `KILL QUERY <id>` from a
   second connection as the same user and waits for its answer before
   anything else is sent on the first; it reads what was in flight to the
   error and closes the connection without a `ROLLBACK`, since the
   transaction is still open and closing ends it. A read whose future is
   dropped sends its kill from a task of its own. A read that ends whole is
   rolled back and closed. `sql_select_limit` only saves the server the rest
   of a statement without a `LIMIT`.
6. **The plugin's lexer reads as the session's `sql_mode` does**, beside
   PostgreSQL's: strings in `'`, and in `"` unless `ANSI_QUOTES` makes `"` a
   name's; a backslash escapes in a string unless `NO_BACKSLASH_ESCAPES`;
   doubled quotes; names in backticks; comments in `#`, `-- ` and `/* */`. A
   read whose `sql_mode` holds MariaDB's `ORACLE` or `MSSQL` is refused,
   naming it, since they change the grammar further. The lexer reads
   `/*! … */` and `/*M! … */` as code and refuses an `@name` inside one,
   since whether a server runs it depends on its version; it leaves `@@name`
   and an account's `user@host` alone; every other `@name` is Dashbin's, so
   MySQL's own user variables are not reachable in a read; it refuses a `?`.
   A document is checked by the default mode, without a connection; the read
   binds by its session's, and a name that only the session's reading finds
   is refused there.

   It refuses, outside strings and comments: a statement beginning with
   `CALL`; `INTO OUTFILE` and `INTO DUMPFILE`; an optimizer hint naming
   `SET_VAR` or `MAX_EXECUTION_TIME`; `SET STATEMENT … FOR`. Each is a
   promise the probe or the manuals show the server alone would not keep.

   **SRC-06** says what the plugin keeps, whatever the user's rights: a read
   changes no row, table, routine or account, writes no file on the server,
   leaves no lock or session state behind it, and runs no longer than its
   bound. What a read may see, a table or a file `LOAD_FILE` reads, is the
   grant's: the manual asks for a user with `SELECT` alone, without `FILE`,
   `EXECUTE` or `PROCESS`, as SRC-03 does on PostgreSQL. Its tests are the
   refusals, a lock free after its read, and SRC-01's.
7. **A read that reaches `@zone` asks first whether the server knows the
   zone**, and is refused with `mysql_tzinfo_to_sql` named when it does not,
   so a chart is never empty because `CONVERT_TZ` said `NULL`.
8. **TLS is a slice of its own, for both plugins**: PostgreSQL's has none
   yet either. Until then a source is read over plain TCP, which is for
   loopback, a tunnel or a network the operator trusts: the manual and
   `dashbin help` say so, and a hub's log says it once of each instance whose
   host is not loopback. `caching_sha2_password`'s RSA hides the password and
   nothing after it, so a production server across a network waits for TLS.
9. **SRC-05 is the kernel's already**: `query.rs` refuses `postgres.sql` on
   a MySQL instance by the instance's plugin; the plugin's tests add
   `mysql.sql` on a PostgreSQL one.
10. **MySQL 8.4 and MariaDB 11 join `compose.yaml`**, and CI's database job
    reads each.

**Left open**: a hub killed mid-read leaves its statement running until the
server's bound, as on PostgreSQL. MariaDB's `KILL QUERY ID` would name the
statement a kill is for, where MySQL names only its connection; with a
connection that is the read's alone and nothing sent while a kill is on its
way, the connection is enough. Zero dates, MariaDB's `UUID` and `INET6` and
any managed server, RDS, PlanetScale, TiDB, were not tried. Every time above
is Docker's loopback: across a network each command that waits costs its
round trip, and a read is the handshake and six of them.
