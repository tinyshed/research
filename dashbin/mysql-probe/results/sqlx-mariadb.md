# sqlx-mysql 0.9 on mariadb

- connect and authenticate, its own settings off: (1.0376ms, 1.1118ms)
- connect and authenticate, its own settings on: (1.4415ms, 1.5485ms)
- what its own settings make of a session: sql_mode Some("PIPES_AS_CONCAT,IGNORE_SPACE,STRICT_TRANS_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_AUTO_CREATE_USER,NO_ENGINE_SUBSTITUTION"), time_zone Some("+00:00"), `'a' || 'b'` = Some("ab")
- a read whole, prepared: (3.8804ms, 4.0731ms)
- d65 as &str: Err("error occurred while decoding column 0: mismatched types; Rust type `&str` (as SQL type `VARCHAR`) is not compatible with SQL type `DECIMAL`"); its bytes unchecked: Ok("[39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 2E, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39]")
- u64 as &str: Err("error occurred while decoding column 1: mismatched types; Rust type `&str` (as SQL type `VARCHAR`) is not compatible with SQL type `BIGINT UNSIGNED`"); its bytes unchecked: Ok("[FF, FF, FF, FF, FF, FF, FF, FF]")
- wall6 as &str: Err("error occurred while decoding column 2: mismatched types; Rust type `&str` (as SQL type `VARCHAR`) is not compatible with SQL type `DATETIME`"); its bytes unchecked: Ok("[0B, EA, 07, 0B, 01, 00, 00, 00, 01, 00, 00, 00]")
- instant6 as &str: Err("error occurred while decoding column 3: mismatched types; Rust type `&str` (as SQL type `VARCHAR`) is not compatible with SQL type `TIMESTAMP`"); its bytes unchecked: Ok("[0B, EA, 07, 0B, 01, 00, 00, 00, 20, A1, 07, 00]")
- doc as &str: Ok("{\"b\": 1, \"a\": [1.5, null]}"); its bytes unchecked: Ok("[7B, 22, 62, 22, 3A, 20, 31, 2C, 20, 22, 61, 22, 3A, 20, 5B, 31, 2E, 35, 2C, 20, 6E, 75, 6C, 6C, 5D, 7D]")
