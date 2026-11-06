# parameters on MariaDb

exact must say 1, near must say 0; both 1 means the comparison went through DOUBLE.

| value | binding | exact | near |
|---|---|---|---|
| DECIMAL(65,30) | (a) `SET @p = ?`, text | Bytes "1" | Bytes "0" |
| DECIMAL(65,30) | (a) `SET @p = CAST(? AS DECIMAL(65,30))` | Bytes "1" | Bytes "0" |
| DECIMAL(65,30) | (b) `= ?`, text | Int 1 | Int 0 |
| DECIMAL(65,30) | (b) `= CAST(? AS DECIMAL(65,30))`, text | Int 1 | Int 0 |
| BIGINT UNSIGNED | (a) `SET @p = ?`, text | Bytes "1" | Bytes "0" |
| BIGINT UNSIGNED | (a) `SET @p = CAST(? AS UNSIGNED)` | Bytes "1" | Bytes "0" |
| BIGINT UNSIGNED | (b) `= ?`, text | Int 1 | Int 0 |
| BIGINT UNSIGNED | (b) `= CAST(? AS UNSIGNED)`, text | Int 1 | Int 0 |
| BIGINT UNSIGNED | (b) `= ?`, UInt | Int 1 | Int 0 |

what the value itself comes back as:

- (a) `SET @p = CAST(? AS DECIMAL(65,30))`, `SELECT @p`: Bytes "99999999999999999999999999999999999.999999999999999999999999999999"
- (a) `SET @p = CAST(? AS DATETIME(6))`, `SELECT @p`: Bytes "2026-10-02 00:00:00.000000"
- (a) the same, `SELECT @p + INTERVAL 1 DAY`: Bytes "2026-10-03 00:00:00.000000"
- (b) `SELECT CAST(? AS DECIMAL(65,30))`: Bytes "99999999999999999999999999999999999.999999999999999999999999999999"
- (b) `SELECT CAST(? AS DATETIME(6)) + INTERVAL 1 DAY`: Date 2026-10-3 0:0:0.0
- (b) `SELECT ?` with a string: Bytes "12.50"
- (b) `SELECT CAST(? AS DECIMAL(4,2))` with '12.345': Bytes "12.35"
- (b) `SELECT CAST(? AS DECIMAL(4,2))` with '123.45': Bytes "99.99"
- (b) `SELECT CAST(? AS DECIMAL(65,30))` with 'abc': Bytes "0.000000000000000000000000000000"
- (b) `SELECT CAST(? AS SIGNED)` with NULL: NULL

## a day of `big`, 5760 rows, through each binding, and whether the index is used

| binding | count | plan |
|---|---|---|
| (a) `SET @f = ?`, text | Bytes "5760" | type=range key=created_at rows=5760 |
| (a) `SET @f = CAST(? AS DATETIME(6))` | Bytes "5760" | type=range key=created_at rows=5760 |
| (b) `?`, text | Int 5760 | type=range key=created_at rows=5760 |
| (b) `CAST(? AS DATETIME(6))` | Int 5760 | type=range key=created_at rows=5760 |
| (b) `?`, a DATETIME value | Int 5760 | |
