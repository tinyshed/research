//! Isolated SQLite adapter experiments; no production API or dependency fork.
use crate::engine::Result;
use rusqlite::{Connection, ffi};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

static MASK: AtomicUsize = AtomicUsize::new(0);
pub fn configure(mask: usize) {
    MASK.store(mask, Ordering::Relaxed);
}
pub fn borrow_metadata() -> bool {
    MASK.load(Ordering::Relaxed) & 1 != 0
}
pub fn arena() -> bool {
    MASK.load(Ordering::Relaxed) & 2 != 0
}

pub fn configure_connection(connection: &Connection) -> Result<()> {
    let mask = MASK.load(Ordering::Relaxed);
    let setting = match mask & 12 {
        0 => return Ok(()),
        4 => (0, 0),
        8 => (512, 512),
        _ => return Err("conflicting lookaside options".into()),
    };
    // Before preparing any statements. A null buffer asks SQLite to own its
    // allocation, so it cannot outlive a Rust backing buffer. NO_MUTEX ownership
    // remains with the Engine; this does not modify its statement cache.
    let rc = unsafe {
        ffi::sqlite3_db_config(
            connection.handle(),
            ffi::SQLITE_DBCONFIG_LOOKASIDE,
            std::ptr::null_mut::<std::ffi::c_void>(),
            setting.0,
            setting.1,
        )
    };
    if rc != ffi::SQLITE_OK {
        return Err(rusqlite::Error::SqliteFailure(ffi::Error::new(rc), None).into());
    }
    Ok(())
}

pub fn counters(connection: &Connection, reset: bool) -> Result<Value> {
    let mut out = serde_json::Map::new();
    for (name, op) in [
        ("lookaside_used", ffi::SQLITE_DBSTATUS_LOOKASIDE_USED),
        ("lookaside_hit", ffi::SQLITE_DBSTATUS_LOOKASIDE_HIT),
        (
            "lookaside_miss_size",
            ffi::SQLITE_DBSTATUS_LOOKASIDE_MISS_SIZE,
        ),
        (
            "lookaside_miss_full",
            ffi::SQLITE_DBSTATUS_LOOKASIDE_MISS_FULL,
        ),
        ("cache_used", ffi::SQLITE_DBSTATUS_CACHE_USED),
        ("cache_hit", ffi::SQLITE_DBSTATUS_CACHE_HIT),
        ("cache_miss", ffi::SQLITE_DBSTATUS_CACHE_MISS),
        ("cache_write", ffi::SQLITE_DBSTATUS_CACHE_WRITE),
        ("statement_used", ffi::SQLITE_DBSTATUS_STMT_USED),
        ("schema_used", ffi::SQLITE_DBSTATUS_SCHEMA_USED),
    ] {
        let (mut current, mut high) = (0, 0);
        let rc = unsafe {
            ffi::sqlite3_db_status(
                connection.handle(),
                op,
                &mut current,
                &mut high,
                i32::from(reset),
            )
        };
        if rc != ffi::SQLITE_OK {
            return Err(format!("db_status {name}: {rc}").into());
        }
        out.insert(name.into(), json!({"current":current,"high":high}));
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_counters_do_not_change_rows() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("create table t(b blob); insert into t values(x'000102');")
            .unwrap();
        counters(&connection, true).unwrap();
        let value: Vec<u8> = connection
            .query_row("select b from t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, [0, 1, 2]);
        let status = counters(&connection, false).unwrap();
        assert!(status["cache_used"]["current"].as_i64().unwrap() > 0);
    }
}
