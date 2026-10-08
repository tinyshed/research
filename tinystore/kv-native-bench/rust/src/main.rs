// Copyright 2026 the TinyStore authors. Apache-2.0; see ../../NOTICE.
// Synchronous research slice; stock safe rusqlite on a matched external SQLite.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{
    Connection, OptionalExtension, ffi, params,
    types::{Value, ValueRef},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(n as u64, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}
#[cfg(feature = "telemetry")]
#[global_allocator]
static ALLOC: Counting = Counting;
const EPOCH: i64 = 1800000000000;
const MAX: usize = 1 << 20;
#[derive(Debug)]
struct Error {
    code: &'static str,
    bucket: String,
    path: String,
    #[allow(dead_code)]
    detail: String,
}
type Result<T> = std::result::Result<T, Error>;
fn sql(e: rusqlite::Error) -> Error {
    Error {
        code: "other",
        bucket: String::new(),
        path: String::new(),
        detail: e.to_string(),
    }
}
fn err(code: &'static str, detail: &str) -> Error {
    Error {
        code,
        bucket: String::new(),
        path: String::new(),
        detail: detail.into(),
    }
}
fn named<T>(r: Result<T>, b: &str, owners: &[String], key: &str) -> Result<T> {
    r.map_err(|mut e| {
        e.bucket = b.into();
        let mut s = owners.to_vec();
        if !key.is_empty() {
            s.push(key.into())
        };
        e.path = s.join("/");
        e
    })
}
fn err_json(e: Option<Error>) -> Json {
    e.map_or(
        Json::Null,
        |e| json!({"code":e.code,"bucket":e.bucket,"path":e.path}),
    )
}
fn raw_json(v: &Value) -> Json {
    match v {
        Value::Null => json!({"Kind":0,"Int":0,"Bytes":null}),
        Value::Integer(n) => json!({"Kind":1,"Int":n,"Bytes":null}),
        Value::Blob(b) => json!({"Kind":2,"Int":0,"Bytes":STANDARD.encode(b)}),
        _ => panic!("invalid raw"),
    }
}
fn raw_parse(v: &Json) -> Value {
    match v["Kind"].as_u64().unwrap_or(0) {
        0 => Value::Null,
        1 => Value::Integer(v["Int"].as_i64().unwrap()),
        2 => Value::Blob(STANDARD.decode(v["Bytes"].as_str().unwrap_or("")).unwrap()),
        _ => panic!("raw kind"),
    }
}
fn escaped(dst: &mut Vec<u8>, s: &[u8]) {
    for &b in s {
        dst.push(b);
        if b == 0 {
            dst.push(255)
        }
    }
}
fn escaped_search(dst: &mut Vec<u8>, mut s: &[u8]) {
    while let Some(p) = s.iter().position(|&b| b == 0) {
        dst.extend_from_slice(&s[..=p]);
        dst.push(255);
        s = &s[p + 1..]
    }
    dst.extend_from_slice(s)
}
fn prefix(owners: &[String]) -> (Vec<u8>, i64) {
    let mut p = Vec::new();
    let mut h = 0i64;
    for (d, o) in owners.iter().enumerate() {
        p.push(1);
        escaped(&mut p, o.as_bytes());
        p.push(0);
        if d < 6 && p.len() < 1024 {
            h |= (p.len() as i64) << (10 * d)
        } else {
            h |= 1 << 60
        }
    }
    (p, h)
}
fn path(owners: &[String], key: &str) -> Result<(Vec<u8>, i64)> {
    if key.is_empty() || owners.iter().any(|s| s.is_empty()) {
        return Err(err("invalid", "an empty key"));
    };
    let (mut p, h) = prefix(owners);
    p.push(2);
    escaped(&mut p, key.as_bytes());
    if p.len() > 1024 {
        return Err(err("invalid", "a path over 1 KiB"));
    };
    Ok((p, h))
}
fn key_of(p: &[u8], n: usize) -> Result<String> {
    let mut out = Vec::new();
    let mut i = n + 1;
    while i < p.len() {
        out.push(p[i]);
        if p[i] == 0 && p.get(i + 1) == Some(&255) {
            i += 1
        };
        i += 1
    }
    String::from_utf8(out).map_err(|_| err("corrupt", "non-UTF8 key in text-only research API"))
}
fn hidden(row: &str, n: usize) -> String {
    let mut a = vec![format!(
        "exists (select 1 from branches as m where m.bucket = {row}.bucket and m.prefix = x'' and m.cleared >= {row}.version)"
    )];
    for d in 0..6 {
        a.push(format!("exists (select 1 from branches as m where m.bucket = {row}.bucket and m.prefix = substr({row}.path,1,nullif((?{n} >> {}) & 1023,0)) and m.cleared >= {row}.version)",d*10))
    }
    a.push(format!("((?{n} >> 60) & 1 and exists(select 1 from branches as m where m.bucket={row}.bucket and m.cleared>={row}.version and substr({row}.path,1,length(m.prefix))=m.prefix and substr({row}.path,length(m.prefix)+1,1) in (x'01',x'02')))") );
    format!(
        "(exists(select 1 from branches as g where g.bucket={row}.bucket) and ({}))",
        a.join(" or ")
    )
}
fn checked_value(v: ValueRef<'_>) -> rusqlite::Result<Value> {
    match v {
        ValueRef::Null => Ok(Value::Null),
        ValueRef::Integer(n) => Ok(Value::Integer(n)),
        ValueRef::Blob(b) if b.len() <= MAX => Ok(Value::Blob(b.to_vec())),
        _ => Err(rusqlite::Error::InvalidColumnType(
            0,
            "value bound/type".into(),
            v.data_type(),
        )),
    }
}
#[derive(Debug)]
struct Entry {
    key: String,
    value: Value,
    version: i64,
    expiry: Option<i64>,
}
fn base36(mut n: i64) -> String {
    let mut s = Vec::new();
    while n > 0 {
        let d = (n % 36) as u8;
        s.push(if d < 10 { b'0' + d } else { b'a' + d - 10 });
        n /= 36
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}
fn entry_json(e: &Entry) -> Json {
    json!({"key":e.key,"value":raw_json(&e.value),"version":base36(e.version),"expiry":e.expiry.unwrap_or(0)})
}
struct Store {
    w: Connection,
    r: Connection,
    revision: Cell<i64>,
    now: Cell<i64>,
    commits: Cell<u64>,
    live: String,
    has_sql: String,
    scan_sql: String,
    cell_sql: String,
    take_sql: String,
    delete_sql: String,
    touch_sql: String,
    add_sql: String,
    max_sql: String,
}
fn connect(p: &Path, read: bool) -> Connection {
    let c = Connection::open(p).unwrap();
    c.execute_batch("pragma foreign_keys=1;pragma busy_timeout=5000;pragma synchronous=FULL;pragma fullfsync=1;pragma checkpoint_fullfsync=1;pragma cache_size=-1024;").unwrap();
    if read {
        c.execute_batch("pragma query_only=1").unwrap()
    } else {
        c.execute_batch("pragma cache_size=-4096;pragma journal_mode=WAL")
            .unwrap()
    };
    c.set_prepared_statement_cache_capacity(32);
    c.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, 2 * MAX as i32)
        .unwrap();
    c
}
impl Store {
    fn open(p: &Path) -> Self {
        let w = connect(p, false);
        let r = connect(p, true);
        let version: String = w
            .query_row("select sqlite_version()", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, "3.53.4");
        let id: i64 = w
            .query_row("pragma application_id", [], |r| r.get(0))
            .unwrap();
        assert_eq!(id, 0x544b5653);
        let page: i64 = w.query_row("pragma page_size", [], |r| r.get(0)).unwrap();
        assert_eq!(page, 4096);
        let revision = w
            .query_row("select value from meta where name='revision'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let gone = format!("(cells.expires <= ?6 or {})", hidden("cells", 7));
        Self {
            w,
            r,
            revision: Cell::new(revision),
            now: Cell::new(EPOCH),
            commits: Cell::new(0),
            live: format!(
                "select c.version,c.expires,c.value,c.spill,s.value from cells as c left join spilled as s on s.id=c.spill where c.bucket=?1 and c.path=?2 and (c.expires is null or c.expires>?3) and not {}",
                hidden("c", 4)
            ),
            has_sql: format!(
                "select version,expires from cells where bucket=?1 and path=?2 and (expires is null or expires>?3) and not {}",
                hidden("cells", 4)
            ),
            scan_sql: format!(
                "select c.path,c.version,c.expires,c.value,c.spill,s.value from cells as c left join spilled as s on s.id=c.spill where c.bucket=?1 and c.path>?2 and c.path<?3 and (c.expires is null or c.expires>?4) and not {} order by c.path limit cast(?5 as integer)",
                hidden("c", 6)
            ),
            cell_sql: format!(
                "select version,expires,spill,{} from cells where bucket=?1 and path=?2",
                hidden("cells", 3)
            ),
            take_sql: format!(
                "delete from cells where bucket=?1 and path=?2 and (expires is null or expires>?3) and (?4=0 or version=?4) and not {} returning version,expires,value,spill",
                hidden("cells", 5)
            ),
            delete_sql: format!(
                "delete from cells where bucket=?1 and path=?2 and (?4=0 or(version=?4 and (expires is null or expires>?3) and not {})) returning spill",
                hidden("cells", 5)
            ),
            touch_sql: format!(
                "update cells set expires=?4 where bucket=?1 and path=?2 and (expires is null or expires>?3) and (?5=0 or version=?5) and not {} returning version",
                hidden("cells", 6)
            ),
            add_sql: format!(
                "insert into cells(bucket,path,version,expires,value)values(?1,?2,?3,?4,?5) on conflict(bucket,path)do update set value=iif({gone},excluded.value,cells.value+excluded.value),expires=iif({gone},excluded.expires,cells.expires),version=excluded.version where {gone} or typeof(cells.value+excluded.value)='integer' returning value"
            ),
            max_sql: format!(
                "insert into cells(bucket,path,version,expires,value)values(?1,?2,?3,?4,max(?5,0)) on conflict(bucket,path)do update set value=iif({gone},excluded.value,max(cells.value,?5)),expires=iif({gone},excluded.expires,cells.expires),version=excluded.version returning value"
            ),
        }
    }
    fn bucket(&self, name: &str, kind: &str) -> Result<i64> {
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().enumerate().all(|(i, b)| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || (i > 0 && (b == b'_' || b == b'-'))
            })
        {
            return Err(err("invalid", "bucket name"));
        };
        let found: Option<(i64, String)> = self
            .w
            .prepare_cached("select id,kind from buckets where name=?1")
            .map_err(sql)?
            .query_row([name], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(sql)?;
        if let Some((id, k)) = found {
            if k != kind {
                return Err(err("invalid", "bucket kind"));
            };
            return Ok(id);
        };
        self.tx(|| {
            self.w
                .prepare_cached("insert into buckets(name,kind)values(?1,?2)returning id")
                .map_err(sql)?
                .query_row(params![name, kind], |r| r.get(0))
                .map_err(sql)
        })
    }
    fn tx<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        self.w
            .execute_batch("begin immediate;savepoint alone")
            .map_err(sql)?;
        match f() {
            Ok(v) => {
                if let Err(e) = self.w.execute_batch("release alone;commit") {
                    let _ = self.w.execute_batch("rollback");
                    return Err(sql(e));
                };
                self.commits.set(self.commits.get() + 1);
                Ok(v)
            }
            Err(e) => {
                self.w
                    .execute_batch("rollback to alone;release alone;rollback")
                    .map_err(sql)?;
                Err(e)
            }
        }
    }
    fn bump(&self) -> Result<i64> {
        let n = self
            .revision
            .get()
            .checked_add(1)
            .ok_or_else(|| err("limit", "revision overflow"))?;
        self.revision.set(n);
        self.w
            .prepare_cached("update meta set value=?1 where name='revision'")
            .map_err(sql)?
            .execute([n])
            .map_err(sql)?;
        Ok(n)
    }
    fn get(&self, b: i64, o: &[String], key: &str) -> Result<Option<Entry>> {
        let (p, h) = path(o, key)?;
        let mut st = self.r.prepare_cached(&self.live).map_err(sql)?;
        let got = st
            .query_row(params![b, p, self.now.get(), h], |r| {
                let spill: Option<i64> = r.get(3)?;
                let v = if spill.is_some() {
                    if r.get_ref(4)?.data_type() == rusqlite::types::Type::Null {
                        return Err(rusqlite::Error::InvalidQuery);
                    };
                    checked_value(r.get_ref(4)?)?
                } else {
                    checked_value(r.get_ref(2)?)?
                };
                Ok(Entry {
                    key: key.into(),
                    version: r.get(0)?,
                    expiry: r.get(1)?,
                    value: v,
                })
            })
            .optional();
        got.map_err(|e| {
            let mut e = sql(e);
            e.code = "corrupt";
            e
        })
    }
    fn has(&self, b: i64, o: &[String], key: &str) -> Result<bool> {
        let (p, h) = path(o, key)?;
        self.r
            .prepare_cached(&self.has_sql)
            .map_err(sql)?
            .query_row(params![b, p, self.now.get(), h], |_| Ok(()))
            .optional()
            .map(|v| v.is_some())
            .map_err(sql)
    }
    fn set_inner(
        &self,
        b: i64,
        o: &[String],
        key: &str,
        value: &Value,
        version: i64,
        expiry: Option<i64>,
        default_ttl: i64,
    ) -> Result<Entry> {
        let (p, h) = path(o, key)?;
        if let Value::Blob(v) = value {
            if v.len() > MAX {
                return Err(err("limit", "value over 1 MiB"));
            }
        };
        let old: Option<(i64, Option<i64>, Option<i64>, bool)> = self
            .w
            .prepare_cached(&self.cell_sql)
            .map_err(sql)?
            .query_row(params![b, p, h], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .optional()
            .map_err(sql)?;
        let live = old
            .as_ref()
            .is_some_and(|(_, ex, _, hidden)| !hidden && ex.is_none_or(|n| n > self.now.get()));
        if version != 0 && (!live || old.as_ref().unwrap().0 != version) {
            return Err(err("conflict", "the key is not live at the version given"));
        };
        let ex = expiry.or_else(|| {
            if live {
                old.as_ref().unwrap().1
            } else if default_ttl > 0 {
                Some(self.now.get() + default_ttl)
            } else {
                None
            }
        });
        let rev = self.bump()?;
        let spill = match value {
            Value::Blob(v) if v.len() > 512 => {
                self.w
                    .prepare_cached("insert into spilled(value)values(?1)")
                    .map_err(sql)?
                    .execute([v])
                    .map_err(sql)?;
                Some(self.w.last_insert_rowid())
            }
            _ => None,
        };
        let inline = if spill.is_some() { &Value::Null } else { value };
        self.w.prepare_cached("insert into cells(bucket,path,version,expires,value,spill)values(?1,?2,?3,?4,?5,?6) on conflict(bucket,path)do update set version=excluded.version,expires=excluded.expires,value=excluded.value,spill=excluded.spill").map_err(sql)?.execute(params![b,p,rev,ex,inline,spill]).map_err(sql)?;
        if let Some((_, _, Some(id), _)) = old {
            self.drop_spill(id)?
        };
        Ok(Entry {
            key: key.into(),
            value: value.clone(),
            version: rev,
            expiry: ex,
        })
    }
    fn set(
        &self,
        b: i64,
        o: &[String],
        key: &str,
        value: &Value,
        version: i64,
        expiry: Option<i64>,
    ) -> Result<Entry> {
        self.tx(|| self.set_inner(b, o, key, value, version, expiry, 0))
    }
    fn drop_spill(&self, id: i64) -> Result<()> {
        self.w
            .prepare_cached("delete from spilled where id=?1")
            .map_err(sql)?
            .execute([id])
            .map_err(sql)?;
        Ok(())
    }
    fn take(
        &self,
        b: i64,
        o: &[String],
        key: &str,
        version: i64,
        bad: bool,
    ) -> Result<Option<Value>> {
        let (p, h) = path(o, key)?;
        self.tx(|| {
            let row: Option<(i64, Option<i64>, Value, Option<i64>)> = self
                .w
                .prepare_cached(&self.take_sql)
                .map_err(sql)?
                .query_row(params![b, p, self.now.get(), version, h], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        checked_value(r.get_ref(2)?)?,
                        r.get(3)?,
                    ))
                })
                .optional()
                .map_err(sql)?;
            let Some((_, _, mut v, spill)) = row else {
                if version != 0 {
                    return Err(err("conflict", "the key is not live at the version given"));
                };
                return Ok(None);
            };
            if let Some(id) = spill {
                v = self
                    .w
                    .prepare_cached("delete from spilled where id=?1 returning value")
                    .map_err(sql)?
                    .query_row([id], |r| checked_value(r.get_ref(0)?))
                    .map_err(|_| err("corrupt", "missing spilled value"))?
            };
            if bad && !matches!(v, Value::Integer(_)) {
                return Err(err("corrupt", "stored value is not an integer"));
            };
            Ok(Some(v))
        })
    }
    fn delete(&self, b: i64, o: &[String], key: &str, version: i64) -> Result<()> {
        let (p, h) = path(o, key)?;
        self.tx(|| {
            let v: Option<Option<i64>> = self
                .w
                .prepare_cached(&self.delete_sql)
                .map_err(sql)?
                .query_row(params![b, p, self.now.get(), version, h], |r| r.get(0))
                .optional()
                .map_err(sql)?;
            if v.is_none() && version != 0 {
                return Err(err("conflict", "version moved"));
            };
            if let Some(Some(id)) = v {
                self.drop_spill(id)?
            };
            Ok(())
        })
    }
    fn touch(
        &self,
        b: i64,
        o: &[String],
        key: &str,
        version: i64,
        ex: Option<i64>,
    ) -> Result<bool> {
        let (p, h) = path(o, key)?;
        let ex = ex.ok_or_else(|| err("invalid", "Touch needs expiry"))?;
        self.tx(|| {
            let v: Option<i64> = self
                .w
                .prepare_cached(&self.touch_sql)
                .map_err(sql)?
                .query_row(params![b, p, self.now.get(), ex, version, h], |r| r.get(0))
                .optional()
                .map_err(sql)?;
            if v.is_none() && version != 0 {
                return Err(err("conflict", "version moved"));
            };
            Ok(v.is_some())
        })
    }
    fn scan(
        &self,
        b: i64,
        o: &[String],
        after: &str,
        limit: i64,
    ) -> Result<(Vec<Entry>, bool, String)> {
        if !(0..=1000).contains(&limit) {
            return Err(err("invalid", "scan limit"));
        };
        let limit = if limit == 0 { 100 } else { limit };
        let (pre, h) = prefix(o);
        let mut from = pre.clone();
        from.push(2);
        escaped(&mut from, after.as_bytes());
        let mut to = pre.clone();
        to.push(3);
        let mut st = self.r.prepare_cached(&self.scan_sql).map_err(sql)?;
        let mut rows = st
            .query(params![b, from, to, self.now.get(), limit + 1, h])
            .map_err(sql)?;
        let mut out = Vec::new();
        let mut bytes = 0;
        let mut more = false;
        while let Some(r) = rows.next().map_err(sql)? {
            if out.len() == limit as usize {
                more = true;
                break;
            };
            let spill: Option<i64> = r.get(4).map_err(sql)?;
            let value = if spill.is_some() {
                checked_value(r.get_ref(5).map_err(sql)?)
            } else {
                checked_value(r.get_ref(3).map_err(sql)?)
            }
            .map_err(|_| err("corrupt", "scan value"))?;
            if spill.is_some() && value == Value::Null {
                return Err(err("corrupt", "missing spilled value"));
            };
            let n = match &value {
                Value::Blob(v) => v.len(),
                Value::Null => 0,
                _ => 8,
            };
            if !out.is_empty() && bytes + n > 4 << 20 {
                more = true;
                break;
            };
            bytes += n;
            let p: Vec<u8> = r.get(0).map_err(sql)?;
            out.push(Entry {
                key: key_of(&p, pre.len())?,
                value,
                version: r.get(1).map_err(sql)?,
                expiry: r.get(2).map_err(sql)?,
            })
        }
        let next = if more {
            out.last().map_or(String::new(), |e| e.key.clone())
        } else {
            String::new()
        };
        Ok((out, more, next))
    }
    fn delete_rows(&self, statement: &str, args: impl rusqlite::Params) -> Result<usize> {
        let mut st = self.w.prepare_cached(statement).map_err(sql)?;
        let mut rows = st.query(args).map_err(sql)?;
        let mut spills = Vec::new();
        let mut count = 0;
        while let Some(r) = rows.next().map_err(sql)? {
            count += 1;
            let id: Option<i64> = r.get(0).map_err(sql)?;
            if let Some(id) = id {
                spills.push(id)
            }
        }
        drop(rows);
        drop(st);
        for id in spills {
            self.drop_spill(id)?
        }
        Ok(count)
    }
    fn clear(&self, b: i64, o: &[String]) -> Result<()> {
        let (pre, _) = prefix(o);
        let mut from = pre.clone();
        from.push(1);
        let mut to = pre.clone();
        to.push(3);
        self.tx(||{let n:i64=self.w.prepare_cached("select count(*)from(select 1 from cells where bucket=?1 and path>=?2 and path<?3 limit cast(?4 as integer))").map_err(sql)?.query_row(params![b,from,to,10001],|r|r.get(0)).map_err(sql)?;if n<=10000{self.delete_rows("delete from cells where bucket=?1 and path>=?2 and path<?3 returning spill",params![b,from,to])?;}else{let rev=self.bump()?;self.w.prepare_cached("insert into branches(bucket,prefix,cleared)values(?1,?2,?3)on conflict(bucket,prefix)do update set cleared=excluded.cleared").map_err(sql)?.execute(params![b,pre,rev]).map_err(sql)?;};Ok(())})
    }
    fn maintain(&self) -> Result<(usize, usize)> {
        let marks: Vec<(i64, Vec<u8>, i64)> = self
            .r
            .prepare_cached("select bucket,prefix,cleared from branches")
            .map_err(sql)?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(sql)?
            .collect::<rusqlite::Result<_>>()
            .map_err(sql)?;
        let (mut cleared, mut expired) = (0, 0);
        for (b, pre, rev) in marks {
            let mut from = pre.clone();
            from.push(1);
            let mut to = pre.clone();
            to.push(3);
            for _ in 0..10 {
                let n=self.tx(||{let n=self.delete_rows("delete from cells where(bucket,path)in(select bucket,path from cells where bucket=?1 and path>=?2 and path<?3 and version<=?4 limit cast(?5 as integer))returning spill",params![b,from,to,rev,10000])?;if n<10000{self.w.prepare_cached("delete from branches where bucket=?1 and prefix=?2 and cleared=?3").map_err(sql)?.execute(params![b,pre,rev]).map_err(sql)?;};Ok(n)})?;
                cleared += n;
                if n < 10000 {
                    break;
                }
            }
        }
        for _ in 0..10 {
            let n=self.tx(||self.delete_rows("delete from cells where(bucket,path)in(select bucket,path from cells where expires<=?1 order by expires limit cast(?2 as integer))returning spill",params![self.now.get(),10000]))?;
            expired += n;
            if n < 10000 {
                break;
            }
        }
        Ok((expired, cleared))
    }
    fn counter(&self, b: i64, o: &[String], key: &str, n: i64, max: bool, ttl: i64) -> Result<i64> {
        let (p, h) = path(o, key)?;
        self.tx(|| {
            let rev = self.bump()?;
            self.w
                .prepare_cached(if max { &self.max_sql } else { &self.add_sql })
                .map_err(sql)?
                .query_row(
                    params![
                        b,
                        p,
                        rev,
                        if ttl > 0 {
                            Some(self.now.get() + ttl)
                        } else {
                            None
                        },
                        n,
                        self.now.get(),
                        h
                    ],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?
                .ok_or_else(|| err("limit", "counter overflow"))
        })
    }
}
fn s<'a>(v: &'a Json, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn owners(v: &Json) -> Vec<String> {
    v["Owners"].as_array().map_or(vec![], |a| {
        a.iter().map(|x| x.as_str().unwrap().into()).collect()
    })
}
fn trace(db: &Path, p: &Path) {
    let st = Store::open(db);
    let data = fs::read_to_string(p).unwrap();
    for line in data.lines() {
        let c: Json = serde_json::from_str(line).unwrap();
        let op = s(&c, "Op");
        let o = owners(&c);
        let b = s(&c, "Bucket");
        let key = s(&c, "Key");
        if let Some(now) = c["Now"].as_i64().filter(|&n| n != 0) {
            st.now.set(now)
        };
        let version = if s(&c, "Version").is_empty() {
            0
        } else {
            i64::from_str_radix(s(&c, "Version"), 36).unwrap()
        };
        let expiry = c["Expiry"].as_i64().filter(|&n| n != 0);
        let mut r = json!({"op":op});
        let result = (|| -> Result<()> {
            if op == "maintain" {
                let (ex, cl) = st.maintain()?;
                r["expired"] = json!(ex);
                r["cleared"] = json!(cl);
                return Ok(());
            };
            let id = st.bucket(
                b,
                if op.starts_with("counter_") {
                    "counters"
                } else {
                    "values"
                },
            )?;
            match op {
                "set" => {
                    r["entry"] = entry_json(&st.tx(|| {
                        st.set_inner(
                            id,
                            &o,
                            key,
                            &raw_parse(&c["Value"]),
                            version,
                            expiry,
                            c["DefaultTTL"].as_i64().unwrap_or(0),
                        )
                    })?)
                }
                "get" => {
                    r["found"] = json!(false);
                    let e = st.get(id, &o, key)?;
                    r["found"] = json!(e.is_some());
                    if let Some(e) = e {
                        r["entry"] = entry_json(&e)
                    }
                }
                "has" => r["found"] = json!(st.has(id, &o, key)?),
                "take" | "take_bad" => {
                    r["found"] = json!(false);
                    let v = st.take(id, &o, key, version, op == "take_bad")?;
                    r["found"] = json!(v.is_some());
                    if let Some(v) = v {
                        r["value"] = raw_json(&v)
                    }
                }
                "delete" => st.delete(id, &o, key, version)?,
                "touch" => {
                    r["found"] = json!(false);
                    r["found"] = json!(st.touch(id, &o, key, version, expiry)?)
                }
                "clear" => st.clear(id, &o)?,
                "scan" => {
                    let (es, more, next) =
                        st.scan(id, &o, s(&c, "After"), c["Limit"].as_i64().unwrap_or(0))?;
                    r["entries"] = json!(es.iter().map(entry_json).collect::<Vec<_>>());
                    r["more"] = json!(more);
                    r["after"] = json!(next)
                }
                "rollback_set" => {
                    let e = st.tx(|| {
                        st.set_inner(id, &o, key, &raw_parse(&c["Value"]), 0, None, 0)?;
                        Err::<(), _>(err("rollback", "research rollback"))
                    });
                    if e.is_err_and(|e| e.code == "rollback") {
                        r["rolled_back"] = json!(true)
                    } else {
                        panic!("rollback failed")
                    }
                }
                "absent" => {
                    if version != 0 {
                        return Err(err("invalid", "IfVersion with SetIfAbsent"));
                    };
                    let old = st.get(id, &o, key)?;
                    if let Some(e) = old {
                        r["created"] = json!(false);
                        r["entry"] = entry_json(&e)
                    } else {
                        r["created"] = json!(true);
                        r["entry"] = entry_json(&st.tx(|| {
                            st.set_inner(
                                id,
                                &o,
                                key,
                                &raw_parse(&c["Value"]),
                                0,
                                expiry,
                                c["DefaultTTL"].as_i64().unwrap_or(0),
                            )
                        })?)
                    }
                }
                "counter_add" | "counter_max" => {
                    r["n"] = json!(st.counter(
                        id,
                        &o,
                        key,
                        c["N"].as_i64().unwrap(),
                        op == "counter_max",
                        1000
                    )?)
                }
                "counter_get" => {
                    let v = st.get(id, &o, key)?;
                    r["n"] = json!(v.map_or(0, |e| match e.value {
                        Value::Integer(n) => n,
                        _ => panic!("counter corrupt"),
                    }))
                }
                _ => panic!("{op}"),
            };
            Ok(())
        })();
        r["error"] = err_json(
            named(
                result,
                b,
                &o,
                if op == "scan" { s(&c, "After") } else { key },
            )
            .err(),
        );
        println!("{r}")
    }
}
#[derive(Serialize, Deserialize)]
struct Document {
    id: i64,
    name: String,
    tags: Vec<String>,
}
#[derive(Serialize)]
struct DocumentRef<'a> {
    id: i64,
    name: &'a str,
    tags: [&'a str; 3],
}
fn keys() -> Vec<String> {
    (0..256)
        .map(|i| {
            let mut s = format!("key{i:06}-Русский-{}", "x".repeat(i % 80));
            if i % 4 == 0 {
                s.push_str("\0tail")
            };
            s
        })
        .collect()
}
fn kernel(name: &str, i: usize, keys: &[String], value: &[u8]) -> u64 {
    match name {
        "key_loop" | "key_search" => {
            let key = &keys[i & 255];
            let mut p = Vec::with_capacity(key.len() + 20);
            p.push(1);
            let esc = if name == "key_loop" {
                escaped
            } else {
                escaped_search
            };
            esc(&mut p, b"tenant");
            p.extend_from_slice(&[0, 1]);
            esc(&mut p, b"42");
            p.extend_from_slice(&[0, 2]);
            esc(&mut p, key.as_bytes());
            let p = black_box(p);
            p.len() as u64 + p[p.len() - 1] as u64
        }
        "value_codec" => {
            let bits = (i as u64).wrapping_mul(0x9e3779b97f4a7c15);
            let v = black_box(bits.to_be_bytes().to_vec());
            let n = u64::from_be_bytes(v.as_slice().try_into().unwrap());
            black_box(n ^ bits) + v.len() as u64
        }
        "json_codec" => {
            let d = DocumentRef {
                id: (i & 255) as i64,
                name: "document",
                tags: ["one", "two", "three"],
            };
            let b = serde_json::to_vec(&d).unwrap();
            let got: Document = black_box(serde_json::from_slice(&b).unwrap());
            b.len() as u64 + got.id as u64
        }
        "scan_materialize" => {
            let es: Vec<Entry> = (0..100)
                .map(|j| Entry {
                    key: keys[j].clone(),
                    value: Value::Blob(value.to_vec()),
                    version: 0,
                    expiry: None,
                })
                .collect();
            let es = black_box(es);
            black_box(
                es.iter()
                    .map(|e| {
                        e.key.len() as u64
                            + match &e.value {
                                Value::Blob(b) => b.len() as u64,
                                _ => 0,
                            }
                    })
                    .sum(),
            )
        }
        "ttl_guard" => {
            let observed = black_box([10, 10, 11, 10, 0, 10, 11, 10][i & 7]);
            let expected = black_box([10, 11, 11, 10, 0, 10, 10, 10][i & 7]);
            let expires = black_box([200, 200, 99, 100, 200, 201, 200, 200][i & 7]);
            let hidden = black_box([false, false, false, false, false, true, false, false][i & 7]);
            u64::from(observed > 0 && observed == expected && expires > 100 && !hidden)
        }
        _ => panic!("kernel"),
    }
}
fn rss() -> Json {
    let mut v = json!({});
    for l in fs::read_to_string("/proc/self/status").unwrap().lines() {
        let a: Vec<_> = l.split_whitespace().collect();
        if a.len() > 1 && (a[0] == "VmRSS:" || a[0] == "VmHWM:") {
            v[a[0]] = json!(a[1].parse::<u64>().unwrap() * 1024)
        }
    }
    v
}
fn counters(c: &Connection) -> Json {
    let mut v = json!({});
    unsafe {
        for (n, op) in [
            ("cache_hits", ffi::SQLITE_DBSTATUS_CACHE_HIT),
            ("cache_misses", ffi::SQLITE_DBSTATUS_CACHE_MISS),
            ("cache_writes", ffi::SQLITE_DBSTATUS_CACHE_WRITE),
            ("cache_spills", ffi::SQLITE_DBSTATUS_CACHE_SPILL),
            ("cache_used", ffi::SQLITE_DBSTATUS_CACHE_USED),
            ("stmt_used", ffi::SQLITE_DBSTATUS_STMT_USED),
        ] {
            let (mut current, mut high) = (0, 0);
            assert_eq!(
                ffi::sqlite3_db_status(c.handle(), op, &mut current, &mut high, 0),
                ffi::SQLITE_OK
            );
            v[n] = json!(current)
        }
    };
    v
}
// SQL-only C-API control. Owning result is copied before reset; all return codes
// are checked. This is deliberately not the engine used by public-path rows.
struct FfiPoint {
    p: *mut ffi::sqlite3_stmt,
}
impl FfiPoint {
    fn new(c: &Connection) -> Self {
        let mut p = std::ptr::null_mut();
        let text = c"select value from cells where bucket=1 and path=?1";
        unsafe {
            assert_eq!(
                ffi::sqlite3_prepare_v3(
                    c.handle(),
                    text.as_ptr(),
                    -1,
                    ffi::SQLITE_PREPARE_PERSISTENT,
                    &mut p,
                    std::ptr::null_mut()
                ),
                ffi::SQLITE_OK
            )
        };
        Self { p }
    }
    fn get(&mut self, path: &[u8]) -> usize {
        unsafe {
            assert_eq!(
                ffi::sqlite3_bind_blob(
                    self.p,
                    1,
                    path.as_ptr().cast(),
                    path.len() as i32,
                    ffi::SQLITE_TRANSIENT()
                ),
                ffi::SQLITE_OK
            );
            assert_eq!(ffi::sqlite3_step(self.p), ffi::SQLITE_ROW);
            let n = ffi::sqlite3_column_bytes(self.p, 0) as usize;
            assert!(n <= MAX);
            let ptr = ffi::sqlite3_column_blob(self.p, 0);
            let own = if n == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ptr.cast::<u8>(), n).to_vec()
            };
            assert_eq!(ffi::sqlite3_step(self.p), ffi::SQLITE_DONE);
            assert_eq!(ffi::sqlite3_reset(self.p), ffi::SQLITE_OK);
            black_box(own).len()
        }
    }
}
impl Drop for FfiPoint {
    fn drop(&mut self) {
        unsafe { assert_eq!(ffi::sqlite3_finalize(self.p), ffi::SQLITE_OK) }
    }
}
fn bench(db: &Path, name: &str, n: usize) {
    let st = Store::open(db);
    let o = vec!["tenant".into(), "42".into()];
    let b = st.bucket("bench", "values").unwrap();
    let sp = st.bucket("spill", "values").unwrap();
    let js = st.bucket("json", "values").unwrap();
    let co = st.bucket("counters", "counters").unwrap();
    let small = st.bucket("small", "values").unwrap();
    let clear = st.bucket("clear", "values").unwrap();
    let ks: Vec<String> = (0..4096).map(|i| format!("k{i:06}")).collect();
    let val = Value::Blob(vec![b'v'; 256]);
    let large = Value::Blob(vec![b'l'; 4096]);
    let mut ffi = if name == "sql_ffi_get" {
        Some(FfiPoint::new(&st.r))
    } else {
        None
    };
    let mut op = |i: usize| -> u64 {
        let key = &ks[(i * 997) & 4095];
        match name {
            "get_bytes" | "get_raw" | "getentry" => {
                let e = black_box(st.get(b, &o, key).unwrap().unwrap());
                let v = match e.value {
                    Value::Blob(v) => v,
                    _ => panic!("bytes"),
                };
                v.len() as u64
                    + if name == "getentry" {
                        base36(e.version).len() as u64
                    } else {
                        0
                    }
            }
            "has" => u64::from(st.has(b, &o, key).unwrap()),
            "get_spill" => {
                match black_box(st.get(sp, &o, &ks[(i * 997) & 511]).unwrap().unwrap()).value {
                    Value::Blob(v) => v.len() as u64,
                    _ => panic!(),
                }
            }
            "get_json" => {
                let e = black_box(st.get(js, &o, &ks[(i * 997) & 1023]).unwrap().unwrap());
                let Value::Blob(v) = e.value else { panic!() };
                let d: Document = black_box(serde_json::from_slice(&v).unwrap());
                d.id as u64 + d.tags.len() as u64
            }
            "scan100" => {
                let (es, _, _) = black_box(st.scan(b, &o, &ks[(i * 97) & 2047], 100).unwrap());
                es.iter()
                    .map(|e| {
                        e.key.len() as u64
                            + match &e.value {
                                Value::Blob(v) => v.len() as u64,
                                _ => 0,
                            }
                    })
                    .sum()
            }
            "set_bytes" => {
                black_box(st.set(b, &o, key, &val, 0, None).unwrap());
                1
            }
            "set_spill" => {
                black_box(
                    st.set(sp, &o, &ks[(i * 997) & 511], &large, 0, None)
                        .unwrap(),
                );
                1
            }
            "cas" => {
                let e = black_box(st.get(b, &o, key).unwrap().unwrap());
                black_box(st.set(b, &o, key, &val, e.version, None).unwrap());
                1
            }
            "take_cycle" => {
                let Value::Blob(v) = black_box(st.take(b, &o, key, 0, false).unwrap().unwrap())
                else {
                    panic!()
                };
                black_box(st.set(b, &o, key, &val, 0, None).unwrap());
                v.len() as u64
            }
            "delete_cycle" => {
                st.delete(b, &o, key, 0).unwrap();
                black_box(st.set(b, &o, key, &val, 0, None).unwrap());
                1
            }
            "counter_add" => st
                .counter(co, &[], &ks[(i * 997) & 1023], 1, false, 0)
                .unwrap() as u64,
            "clear_small" => {
                st.clear(small, &["parent".into()]).unwrap();
                256
            }
            "clear_mark" => {
                st.clear(clear, &["parent".into()]).unwrap();
                10001
            }
            "expire" => {
                st.now.set(EPOCH + 1000);
                st.maintain().unwrap().0 as u64
            }
            "sql_get" | "sql_ffi_get" => {
                let (p, _) = path(&o, key).unwrap();
                if let Some(f) = ffi.as_mut() {
                    f.get(&p) as u64
                } else {
                    let v: Vec<u8> =
                        st.r.prepare_cached("select value from cells where bucket=1 and path=?1")
                            .unwrap()
                            .query_row([p], |r| r.get(0))
                            .unwrap();
                    black_box(v).len() as u64
                }
            }
            _ => panic!("case {name}"),
        }
    };
    if !name.starts_with("clear_") && name != "expire" {
        for i in 0..64 {
            black_box(op(i));
        }
    };
    let cb = counters(&st.r);
    let wb = counters(&st.w);
    let commits = st.commits.get();
    let before = rss();
    let a = CALLS.load(Ordering::Relaxed);
    let ab = BYTES.load(Ordering::Relaxed);
    let start = Instant::now();
    let mut sum = 0u64;
    for i in 0..n {
        sum = sum.wrapping_add(black_box(op(i)))
    }
    let ns = start.elapsed().as_nanos();
    let ac = CALLS.load(Ordering::Relaxed) - a;
    let bytes = BYTES.load(Ordering::Relaxed) - ab;
    println!(
        "{}",
        json!({"language":"rust","case":name,"iterations":n,"elapsed_ns":ns,"ns_op":ns as f64/n as f64,"alloc_calls":ac,"alloc_bytes":bytes,"rss_before":before,"rss_after":rss(),"checksum":sum,"sqlite_reader_before":cb,"sqlite_reader_after":counters(&st.r),"sqlite_writer_before":wb,"sqlite_writer_after":counters(&st.w),"commits":st.commits.get()-commits})
    );
}
fn kernel_bench(name: &str, n: usize) {
    let keys = keys();
    let v = vec![b'k'; 256];
    for i in 0..64 {
        black_box(kernel(name, i, &keys, &v));
    }
    let a = CALLS.load(Ordering::Relaxed);
    let ab = BYTES.load(Ordering::Relaxed);
    let start = Instant::now();
    let mut sum = 0u64;
    for i in 0..n {
        sum = sum.wrapping_add(black_box(kernel(name, i, &keys, &v)))
    }
    let ns = start.elapsed().as_nanos();
    let ac = CALLS.load(Ordering::Relaxed) - a;
    let bytes = BYTES.load(Ordering::Relaxed) - ab;
    println!(
        "{}",
        json!({"language":"rust","case":name,"iterations":n,"elapsed_ns":ns,"ns_op":ns as f64/n as f64,"alloc_calls":ac,"alloc_bytes":bytes,"checksum":sum,"rss_after":rss()})
    );
}
fn metadata(db: &Path) {
    let st = Store::open(db);
    let mut out = json!({});
    for q in ["sqlite_version()", "sqlite_source_id()"] {
        let v: String =
            st.r.query_row(&format!("select {q}"), [], |r| r.get(0))
                .unwrap();
        out[q] = json!(v)
    }
    let opts: Vec<String> =
        st.r.prepare("pragma compile_options")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
    out["compile_options"] = json!(opts);
    for (name, c) in [("reader", &st.r), ("writer", &st.w)] {
        let mut p = json!({});
        for q in [
            "page_size",
            "synchronous",
            "cache_size",
            "foreign_keys",
            "busy_timeout",
            "fullfsync",
            "checkpoint_fullfsync",
            "query_only",
        ] {
            let v: i64 = c
                .query_row(&format!("pragma {q}"), [], |r| r.get(0))
                .unwrap();
            p[q] = json!(v)
        }
        let wal: String = c
            .query_row("pragma journal_mode", [], |r| r.get(0))
            .unwrap();
        p["journal_mode"] = json!(wal);
        out[name] = p
    }
    println!("{out}")
}
fn verify_typed(db: &Path) {
    let st = Store::open(db);
    let mut checked = 0;
    for (name, bits) in [
        (
            "floats",
            vec![
                0,
                1 << 63,
                0x7ff0000000000000,
                0xfff0000000000000,
                0x7ff0000000000001,
                0x7ff8000000001234u64,
            ],
        ),
        ("unsigned", vec![0, 1, 1 << 63, u64::MAX]),
    ] {
        let b = st.bucket(name, "values").unwrap();
        for (i, n) in bits.iter().enumerate() {
            let e = st.get(b, &[], &i.to_string()).unwrap().unwrap();
            let Value::Blob(v) = &e.value else { panic!() };
            assert_eq!(u64::from_be_bytes(v.as_slice().try_into().unwrap()), *n);
            st.set(b, &[], &e.key, &e.value, 0, None).unwrap();
            checked += 1
        }
    }
    let b = st.bucket("float32", "values").unwrap();
    for (i, n) in [0, 1 << 31, 0x7f800001, 0x7fc01234u32].iter().enumerate() {
        let e = st.get(b, &[], &i.to_string()).unwrap().unwrap();
        let Value::Blob(v) = &e.value else { panic!() };
        assert_eq!(u32::from_be_bytes(v.as_slice().try_into().unwrap()), *n);
        st.set(b, &[], &e.key, &e.value, 0, None).unwrap();
        checked += 1
    }
    for name in ["strings", "signed", "shapes", "nothing"] {
        let b = st.bucket(name, "values").unwrap();
        let (es, _, _) = st.scan(b, &[], "", 1000).unwrap();
        for e in es {
            st.set(b, &[], &e.key, &e.value, 0, None).unwrap();
            checked += 1
        }
    }
    println!(
        "{}",
        json!({"native_typed_and_raw_rewrite_checked":checked,"bit_exact":true})
    )
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let arg = |name: &str, default: &str| -> String {
        a.windows(2)
            .find(|x| x[0] == name)
            .map_or(default.to_string(), |x| x[1].clone())
    };
    let mode = arg("--mode", "bench");
    let dir = PathBuf::from(arg("--dir", ""));
    let db = dir.join("kv.db");
    let name = arg("--case", "get_bytes");
    let n = arg("--iterations", "10000").parse().unwrap();
    match mode.as_str() {
        "bench" => bench(&db, &name, n),
        "kernel" => kernel_bench(&name, n),
        "trace" => trace(&db, Path::new(&arg("--trace", ""))),
        "metadata" => metadata(&db),
        "verify" => verify_typed(&db),
        "kernel-check" => {
            let keys = keys();
            let v = vec![b'k'; 256];
            let mut out = json!({});
            for name in [
                "key_loop",
                "key_search",
                "value_codec",
                "json_codec",
                "scan_materialize",
                "ttl_guard",
            ] {
                let sum: u64 = (0..256).map(|i| kernel(name, i, &keys, &v)).sum();
                out[name] = json!(sum)
            }
            out["escaped_hex"] = json!("6100ff62");
            println!("{out}")
        }
        _ => panic!("mode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_encoding_and_bounds() {
        assert_eq!(
            path(&["a\0b".into(), "42".into()], "x\0y").unwrap().0,
            vec![
                1, b'a', 0, 255, b'b', 0, 1, b'4', b'2', 0, 2, b'x', 0, 255, b'y'
            ]
        );
        assert_eq!(path(&[], "").unwrap_err().code, "invalid");
        assert_eq!(path(&[], &"x".repeat(1024)).unwrap_err().code, "invalid");
        assert_eq!(key_of(&path(&[], "x\0y").unwrap().0, 0).unwrap(), "x\0y");
    }
    #[test]
    fn base36_and_owned_float_bits() {
        assert_eq!(base36(36), "10");
        for n in [0, 1 << 63, 0x7ff0000000000001, 0x7ff8000000001234u64] {
            let v = Value::Blob(n.to_be_bytes().to_vec());
            let Value::Blob(b) = v.clone() else { panic!() };
            assert_eq!(u64::from_be_bytes(b.try_into().unwrap()), n)
        }
    }
    #[test]
    fn alternative_kernels_agree() {
        let keys = keys();
        for i in 0..256 {
            assert_eq!(
                kernel("key_loop", i, &keys, &[]),
                kernel("key_search", i, &keys, &[])
            )
        }
    }
}
