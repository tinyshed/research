//! Same SQL and owned output through rusqlite and an independently owned C stmt.
use crate::engine::Result;
use rusqlite::{Connection, StatementStatus, ffi};
use serde_json::json;
use std::{ffi::CStr, marker::PhantomData, rc::Rc, time::Instant};

const SQL: &CStr = c"SELECT id,body FROM payloads ORDER BY id LIMIT CAST(? AS INTEGER)";
type Rows = Vec<(i64, Vec<u8>)>;

struct RawStatement<'conn> {
    statement: *mut ffi::sqlite3_stmt,
    // Own only this statement, never a pointer from rusqlite's statement cache.
    // The connection must outlive it, and a NO_MUTEX stmt cannot cross threads.
    _owner: PhantomData<(&'conn Connection, Rc<()>)>,
}
fn checked(rc: i32) -> rusqlite::Result<()> {
    if rc == ffi::SQLITE_OK {
        Ok(())
    } else {
        Err(rusqlite::Error::SqliteFailure(ffi::Error::new(rc), None))
    }
}
impl<'conn> RawStatement<'conn> {
    fn new(connection: &'conn Connection) -> Result<Self> {
        let mut statement = std::ptr::null_mut();
        // SQL is static and NUL terminated. The statement is exclusively owned
        // here; prepare-v3/PERSISTENT matches prepare_cached outside the timer.
        let rc = unsafe {
            ffi::sqlite3_prepare_v3(
                connection.handle(),
                SQL.as_ptr(),
                -1,
                ffi::SQLITE_PREPARE_PERSISTENT as u32,
                &mut statement,
                std::ptr::null_mut(),
            )
        };
        if rc != ffi::SQLITE_OK {
            if !statement.is_null() {
                unsafe {
                    ffi::sqlite3_finalize(statement);
                }
            }
            checked(rc)?;
        }
        if statement.is_null() {
            return Err("prepare returned no statement".into());
        }
        Ok(Self {
            statement,
            _owner: PhantomData,
        })
    }
    fn read(&mut self, limit: i64) -> Result<Rows> {
        // Reset on every success/error. No raw SQLite memory escapes the step.
        let result = self.read_inner(limit);
        let reset = unsafe { ffi::sqlite3_reset(self.statement) };
        match result {
            Err(error) => Err(error),
            Ok(rows) => {
                checked(reset)?;
                Ok(rows)
            }
        }
    }
    fn read_inner(&mut self, limit: i64) -> Result<Rows> {
        checked(unsafe { ffi::sqlite3_bind_int64(self.statement, 1, limit) })?;
        let mut out = Vec::new();
        loop {
            let rc = unsafe { ffi::sqlite3_step(self.statement) };
            if rc == ffi::SQLITE_DONE {
                break;
            }
            if rc != ffi::SQLITE_ROW {
                checked(rc)?;
            }
            // Check types, indices and allocation failure rather than treating
            // NULL as an empty BLOB. Matches the safe typed conversion path.
            let (id_type, blob_type) = unsafe {
                (
                    ffi::sqlite3_column_type(self.statement, 0),
                    ffi::sqlite3_column_type(self.statement, 1),
                )
            };
            if id_type != ffi::SQLITE_INTEGER || blob_type != ffi::SQLITE_BLOB {
                return Err("raw probe column type".into());
            }
            let id = unsafe { ffi::sqlite3_column_int64(self.statement, 0) };
            let pointer = unsafe { ffi::sqlite3_column_blob(self.statement, 1) };
            let length = unsafe { ffi::sqlite3_column_bytes(self.statement, 1) };
            if length < 0 || (length > 0 && pointer.is_null()) {
                return Err("raw probe BLOB allocation".into());
            }
            let bytes = if length == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length as usize) }
            };
            out.push((id, bytes.to_vec()));
        }
        Ok(out)
    }
    fn status(&self) -> (i32, i32) {
        unsafe {
            (
                ffi::sqlite3_stmt_status(self.statement, ffi::SQLITE_STMTSTATUS_VM_STEP, 0),
                ffi::sqlite3_stmt_status(self.statement, ffi::SQLITE_STMTSTATUS_MEMUSED, 0),
            )
        }
    }
}
impl Drop for RawStatement<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::sqlite3_finalize(self.statement);
        }
    }
}

fn safe_read(statement: &mut rusqlite::Statement<'_>, limit: i64) -> Result<Rows> {
    let mut rows = statement.query([limit])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push((row.get(0)?, row.get(1)?));
    }
    Ok(out)
}

pub fn run(
    connection: &Connection,
    case: &str,
    iterations: u64,
    warm: u64,
) -> Result<serde_json::Value> {
    let (variant, limit) = case
        .split_once('_')
        .ok_or("probe requires safe_N or raw_N")?;
    let limit: i64 = limit.parse()?;
    if !(1..=4096).contains(&limit) || !["safe", "raw"].contains(&variant) {
        return Err("probe arguments".into());
    }
    let mut safe = connection.prepare_cached(SQL.to_str()?)?;
    let mut raw = RawStatement::new(connection)?;
    let wanted = safe_read(&mut safe, limit)?;
    assert_eq!(raw.read(limit)?, wanted);
    let read = |safe: &mut rusqlite::Statement<'_>, raw: &mut RawStatement<'_>| {
        if variant == "safe" {
            safe_read(safe, limit)
        } else {
            raw.read(limit)
        }
    };
    for _ in 0..warm {
        std::hint::black_box(read(&mut safe, &mut raw)?);
    }
    let start = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(read(&mut safe, &mut raw)?);
    }
    let elapsed = start.elapsed();
    let (steps, memory) = if variant == "safe" {
        (
            safe.get_status(StatementStatus::VmStep),
            safe.get_status(StatementStatus::MemUsed),
        )
    } else {
        raw.status()
    };
    Ok(json!({"case":case,"iterations":iterations,"warm":warm,
        "ns_per_op":elapsed.as_nanos() as f64 / iterations as f64,
        "rows":wanted.len(),"bytes":wanted.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
        "vm_steps":steps,"statement_bytes":memory}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_statement_resets_after_error_and_preserves_owned_bytes() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE payloads(id INTEGER PRIMARY KEY,body BLOB); INSERT INTO payloads VALUES(1,x'000102'),(2,x'')").unwrap();
        let mut raw = RawStatement::new(&connection).unwrap();
        let first = raw.read(2).unwrap();
        connection
            .execute_batch("UPDATE payloads SET body=NULL WHERE id=1")
            .unwrap();
        assert!(raw.read(2).is_err());
        connection
            .execute_batch("UPDATE payloads SET body=x'ff' WHERE id=1")
            .unwrap();
        assert_eq!(raw.read(1).unwrap(), [(1, vec![255])]);
        drop(raw);
        drop(connection);
        assert_eq!(first, [(1, vec![0, 1, 2]), (2, vec![])]);
    }
}
