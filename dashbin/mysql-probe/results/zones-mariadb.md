# zones on MariaDb

- `SELECT COUNT(*) FROM mysql.time_zone_name` as reader: failed: 1142 SELECT command denied to user 'reader'@'172.28.0.1' for table `mysql`.`time_zone_name`
- `SELECT CONVERT_TZ('2026-10-25 00:30:00', '+00:00', 'Europe/Berlin')` as reader: Bytes "2026-10-25 02:30:00"
- `SELECT CONVERT_TZ('2026-10-25 01:30:00', '+00:00', 'Europe/Berlin')` as reader: Bytes "2026-10-25 02:30:00"
- `SELECT CONVERT_TZ('2026-11-01 00:00:00', '+00:00', 'Asia/Kolkata')` as reader: Bytes "2026-11-01 05:30:00"
- `SELECT CONVERT_TZ('2026-11-01 00:00:00', '+00:00', 'Nowhere/Zone')` as reader: NULL
- `SELECT @@session.time_zone, @@system_time_zone` as reader: Bytes "+00:00"
- `SELECT COUNT(*) FROM mysql.time_zone_name` as root: Bytes "498"

## one TIMESTAMP and one DATETIME holding the same 15 seconds past 2026-10-01 00:00 UTC, row 1 of big

- session `+00:00`: text ["Bytes \"2026-10-01 00:00:15.000000\"", "Bytes \"2026-10-01 00:00:15.000000\"", "Bytes \"1790812815.000000\""]; binary ["Date 2026-10-1 0:0:15.0", "Date 2026-10-1 0:0:15.0"]; a UTC day bound as text counts Int 5760, its first id Int 5760 (5761 is right)
- session `Europe/Moscow`: text ["Bytes \"2026-10-01 03:00:15.000000\"", "Bytes \"2026-10-01 00:00:15.000000\"", "Bytes \"1790812815.000000\""]; binary ["Date 2026-10-1 3:0:15.0", "Date 2026-10-1 0:0:15.0"]; a UTC day bound as text counts Int 5760, its first id Int 5040 (5761 is right)
