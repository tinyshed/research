# types on MariaDb

| column | type (text) | type (binary) | row | text protocol | binary protocol |
|---|---|---|---|---|---|
| id | LONG binary decimals=0 | LONG binary decimals=0 | 1 | Bytes "1" | Int 1 |
| id | LONG binary decimals=0 | LONG binary decimals=0 | 2 | Bytes "2" | Int 2 |
| d65 | NEWDECIMAL binary decimals=30 | NEWDECIMAL binary decimals=30 | 1 | Bytes "99999999999999999999999999999999999.999999999999999999999999999999" | Bytes "99999999999999999999999999999999999.999999999999999999999999999999" |
| d65 | NEWDECIMAL binary decimals=30 | NEWDECIMAL binary decimals=30 | 2 | Bytes "-0.000000000000000000000000000001" | Bytes "-0.000000000000000000000000000001" |
| d12 | NEWDECIMAL binary decimals=2 | NEWDECIMAL binary decimals=2 | 1 | Bytes "9999999999.99" | Bytes "9999999999.99" |
| d12 | NEWDECIMAL binary decimals=2 | NEWDECIMAL binary decimals=2 | 2 | Bytes "-0.01" | Bytes "-0.01" |
| u64 | LONGLONG unsigned binary decimals=0 | LONGLONG unsigned binary decimals=0 | 1 | Bytes "18446744073709551615" | UInt 18446744073709551615 |
| u64 | LONGLONG unsigned binary decimals=0 | LONGLONG unsigned binary decimals=0 | 2 | Bytes "0" | Int 0 |
| i64 | LONGLONG binary decimals=0 | LONGLONG binary decimals=0 | 1 | Bytes "-9223372036854775808" | Int -9223372036854775808 |
| i64 | LONGLONG binary decimals=0 | LONGLONG binary decimals=0 | 2 | Bytes "9223372036854775807" | Int 9223372036854775807 |
| i8 | TINY binary decimals=0 | TINY binary decimals=0 | 1 | Bytes "-128" | Int -128 |
| i8 | TINY binary decimals=0 | TINY binary decimals=0 | 2 | Bytes "127" | Int 127 |
| flag | TINY binary decimals=0 | TINY binary decimals=0 | 1 | Bytes "1" | Int 1 |
| flag | TINY binary decimals=0 | TINY binary decimals=0 | 2 | Bytes "0" | Int 0 |
| f32 | FLOAT binary decimals=31 | FLOAT binary decimals=31 | 1 | Bytes "0.1" | Float 0.1 |
| f32 | FLOAT binary decimals=31 | FLOAT binary decimals=31 | 2 | Bytes "-3.4e38" | Float -340000000000000000000000000000000000000 |
| f64 | DOUBLE binary decimals=31 | DOUBLE binary decimals=31 | 1 | Bytes "0.1" | Double 0.1 |
| f64 | DOUBLE binary decimals=31 | DOUBLE binary decimals=31 | 2 | Bytes "1e308" | Double 100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 |
| day | DATE binary decimals=0 | DATE binary decimals=0 | 1 | Bytes "2026-11-01" | Date 2026-11-1 0:0:0.0 |
| day | DATE binary decimals=0 | DATE binary decimals=0 | 2 | Bytes "1000-01-01" | Date 1000-1-1 0:0:0.0 |
| wall0 | DATETIME binary decimals=0 | DATETIME binary decimals=0 | 1 | Bytes "9999-12-31 23:59:59" | Date 9999-12-31 23:59:59.0 |
| wall0 | DATETIME binary decimals=0 | DATETIME binary decimals=0 | 2 | Bytes "1000-01-01 00:00:00" | Date 1000-1-1 0:0:0.0 |
| wall3 | DATETIME binary decimals=3 | DATETIME binary decimals=3 | 1 | Bytes "2026-11-01 00:00:00.120" | Date 2026-11-1 0:0:0.120000 |
| wall3 | DATETIME binary decimals=3 | DATETIME binary decimals=3 | 2 | Bytes "1000-01-01 00:00:00.000" | Date 1000-1-1 0:0:0.0 |
| wall6 | DATETIME binary decimals=6 | DATETIME binary decimals=6 | 1 | Bytes "2026-11-01 00:00:00.000001" | Date 2026-11-1 0:0:0.1 |
| wall6 | DATETIME binary decimals=6 | DATETIME binary decimals=6 | 2 | Bytes "1970-01-01 00:00:00.000000" | Date 1970-1-1 0:0:0.0 |
| instant6 | TIMESTAMP unsigned binary decimals=6 | TIMESTAMP unsigned binary decimals=6 | 1 | Bytes "2026-11-01 00:00:00.500000" | Date 2026-11-1 0:0:0.500000 |
| instant6 | TIMESTAMP unsigned binary decimals=6 | TIMESTAMP unsigned binary decimals=6 | 2 | Bytes "1970-01-01 00:00:01.000000" | Date 1970-1-1 0:0:1.0 |
| clock | TIME binary decimals=6 | TIME binary decimals=6 | 1 | Bytes "-838:59:59.000000" | Time neg=true 34d 22:59:59.0 |
| clock | TIME binary decimals=6 | TIME binary decimals=6 | 2 | Bytes "838:59:59.000000" | Time neg=false 34d 22:59:59.0 |
| yr | YEAR unsigned binary decimals=0 | YEAR unsigned binary decimals=0 | 1 | Bytes "2155" | Int 2155 |
| yr | YEAR unsigned binary decimals=0 | YEAR unsigned binary decimals=0 | 2 | Bytes "1901" | Int 1901 |
| word | VAR_STRING decimals=0 | VAR_STRING decimals=0 | 1 | Bytes "кот 🐈 \"q\" \\ '" | Bytes "кот 🐈 \"q\" \\ '" |
| word | VAR_STRING decimals=0 | VAR_STRING decimals=0 | 2 | Bytes "" | Bytes "" |
| latin | VAR_STRING decimals=0 | VAR_STRING decimals=0 | 1 | Bytes "café" | Bytes "café" |
| latin | VAR_STRING decimals=0 | VAR_STRING decimals=0 | 2 | Bytes "" | Bytes "" |
| bytes | VAR_STRING binary decimals=0 | VAR_STRING binary decimals=0 | 1 | Bytes 0x00FF10 | Bytes 0x00FF10 |
| bytes | VAR_STRING binary decimals=0 | VAR_STRING binary decimals=0 | 2 | Bytes "" | Bytes "" |
| bits | BIT unsigned binary decimals=0 | BIT unsigned binary decimals=0 | 1 | Bytes 0x02AA | Bytes 0x02AA |
| bits | BIT unsigned binary decimals=0 | BIT unsigned binary decimals=0 | 2 | Bytes 0x0000 | Bytes 0x0000 |
| doc | BLOB decimals=0 | BLOB decimals=0 | 1 | Bytes "{\"b\": 1, \"a\": [1.5, null]}" | Bytes "{\"b\": 1, \"a\": [1.5, null]}" |
| doc | BLOB decimals=0 | BLOB decimals=0 | 2 | Bytes "null" | Bytes "null" |
| state | STRING decimals=0 | STRING decimals=0 | 1 | Bytes "paid" | Bytes "paid" |
| state | STRING decimals=0 | STRING decimals=0 | 2 | Bytes "open" | Bytes "open" |
| tags | STRING decimals=0 | STRING decimals=0 | 1 | Bytes "a,c" | Bytes "a,c" |
| tags | STRING decimals=0 | STRING decimals=0 | 2 | Bytes "" | Bytes "" |
| spot | GEOMETRY binary decimals=0 | GEOMETRY binary decimals=0 | 1 | Bytes 0x000000000101000000000000000000F03F0000000000000040 | Bytes 0x000000000101000000000000000000F03F0000000000000040 |
| spot | GEOMETRY binary decimals=0 | GEOMETRY binary decimals=0 | 2 | NULL | NULL |

row 3 through the binary protocol: 24 values, all NULL but id: true

computed columns through the binary protocol:

- created_at: TIMESTAMP unsigned binary decimals=6 → Date 2026-10-1 0:0:15.0
- later: DATETIME binary decimals=6 → Date 2026-10-1 1:0:15.0
- day: DATE binary decimals=0 → Date 2026-10-1 0:0:0.0
- epoch: NEWDECIMAL binary decimals=6 → Bytes "1790812815.000000"
- cast_param: DATETIME binary decimals=6 → Date 2026-11-1 0:0:0.0
- raw_param: VAR_STRING decimals=39 → Bytes "2026-11-01 00:00:00"
- total: NEWDECIMAL binary decimals=2 → Bytes "0.37"
- n: LONGLONG binary decimals=0 → Int 1
- mean: NEWDECIMAL binary decimals=6 → Bytes "0.370000"
