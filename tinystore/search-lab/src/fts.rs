use crate::analyzer::Analyzer;
use rusqlite::{Connection, ffi};
use std::ffi::{c_char, c_void};
unsafe extern "C" fn create(
    context: *mut c_void,
    _args: *mut *const c_char,
    _n: i32,
    out: *mut *mut ffi::Fts5Tokenizer,
) -> i32 {
    match std::panic::catch_unwind(|| unsafe { (&*context.cast::<Analyzer>()).clone() }) {
        Ok(a) => {
            unsafe {
                *out = Box::into_raw(Box::new(a)).cast();
            }
            ffi::SQLITE_OK
        }
        Err(_) => ffi::SQLITE_ERROR,
    }
}
unsafe extern "C" fn destroy(a: *mut ffi::Fts5Tokenizer) {
    unsafe {
        drop(Box::from_raw(a.cast::<Analyzer>()));
    }
}
unsafe extern "C" fn context_destroy(a: *mut c_void) {
    unsafe {
        drop(Box::from_raw(a.cast::<Analyzer>()));
    }
}
unsafe extern "C" fn tokenize(
    a: *mut ffi::Fts5Tokenizer,
    context: *mut c_void,
    _flags: i32,
    text: *const c_char,
    len: i32,
    emit: Option<unsafe extern "C" fn(*mut c_void, i32, *const c_char, i32, i32, i32) -> i32>,
) -> i32 {
    // No Rust unwind crosses SQLite's C ABI. Byte offsets always refer to the
    // original UTF-8 input, even when normalization changes token byte length.
    std::panic::catch_unwind(|| {
        let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), len as usize) };
        let text = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return ffi::SQLITE_ERROR,
        };
        let a = unsafe { &*a.cast::<Analyzer>() };
        let mut prev = None;
        for t in a.tokens(text) {
            let flags = if prev == Some(t.position) { 1 } else { 0 };
            prev = Some(t.position);
            let rc = unsafe {
                emit.unwrap()(
                    context,
                    flags,
                    t.text.as_ptr().cast(),
                    t.text.len() as i32,
                    t.start as i32,
                    t.end as i32,
                )
            };
            if rc != ffi::SQLITE_OK {
                return rc;
            }
        }
        ffi::SQLITE_OK
    })
    .unwrap_or(ffi::SQLITE_ERROR)
}
pub fn register(c: &Connection, mode: &str) {
    if mode == "unicode" {
        return;
    }
    unsafe {
        let db = c.handle();
        let mut stmt = std::ptr::null_mut();
        let mut api: *mut ffi::fts5_api = std::ptr::null_mut();
        assert_eq!(
            ffi::sqlite3_prepare_v2(
                db,
                c"select fts5(?1)".as_ptr(),
                -1,
                &mut stmt,
                std::ptr::null_mut()
            ),
            ffi::SQLITE_OK
        );
        assert_eq!(
            ffi::sqlite3_bind_pointer(
                stmt,
                1,
                (&mut api as *mut *mut ffi::fts5_api).cast(),
                c"fts5_api_ptr".as_ptr(),
                None
            ),
            ffi::SQLITE_OK
        );
        assert_eq!(ffi::sqlite3_step(stmt), ffi::SQLITE_ROW);
        ffi::sqlite3_finalize(stmt);
        assert!(!api.is_null());
        let mut v = ffi::fts5_tokenizer {
            xCreate: Some(create),
            xDelete: Some(destroy),
            xTokenize: Some(tokenize),
        };
        let context = Box::into_raw(Box::new(Analyzer::new(mode))).cast();
        assert_eq!(
            (*api).xCreateTokenizer.unwrap()(
                api,
                c"tiny_auto_v1".as_ptr(),
                context,
                &mut v,
                Some(context_destroy)
            ),
            ffi::SQLITE_OK
        );
    }
}
pub fn query_string(q: &str, mode: &str) -> String {
    // Literal OR-of-word queries. No user input is interpreted as FTS syntax.
    let words: Vec<String> = if mode == "unicode" {
        q.split_whitespace().map(str::to_owned).collect()
    } else {
        let a = Analyzer::new("icu");
        a.tokens(q)
            .into_iter()
            .map(|t| q[t.start..t.end].to_owned())
            .collect()
    };
    words
        .into_iter()
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}
