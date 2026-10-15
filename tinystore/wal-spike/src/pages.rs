use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::{Result, require};

pub(super) const MAX_IMAGE: usize = 16 * 1024 * 1024;
pub(super) type Pages = BTreeMap<u32, Vec<u8>>;

fn initialize() -> Result<()> {
    static READY: AtomicBool = AtomicBool::new(false);
    static START: Mutex<()> = Mutex::new(());
    if READY.load(Ordering::Acquire) {
        return Ok(());
    }
    let _guard = START
        .lock()
        .map_err(|_| std::io::Error::other("SQLite startup lock poisoned"))?;
    if !READY.load(Ordering::Acquire) {
        // Native rusqlite must not race TinyStore installing its SQLite mutexes.
        let dir = tempfile::tempdir()?;
        let store = tinystore::Store::open(
            dir.path(),
            tinystore::Options {
                background: false,
                ..Default::default()
            },
        )?;
        store.database("bootstrap").open()?;
        store.close()?;
        READY.store(true, Ordering::Release);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub(super) struct Image {
    pub page_size: u32,
    pub pages: Pages,
}

pub(super) fn reader(path: &Path) -> Result<Connection> {
    initialize()?;
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(Duration::from_secs(5))?;
    db.pragma_update(None, "query_only", true)?;
    Ok(db)
}

impl Image {
    pub(super) fn read(db: &mut Connection) -> Result<Self> {
        let tx = db.transaction()?;
        let image = Self::in_snapshot(&tx)?;
        tx.commit()?;
        Ok(image)
    }

    fn in_snapshot(db: &Connection) -> Result<Self> {
        let page_size: u32 = db.pragma_query_value(None, "page_size", |row| row.get(0))?;
        let count: u32 = db.pragma_query_value(None, "page_count", |row| row.get(0))?;
        require(
            count > 0 && u64::from(count) * u64::from(page_size) <= MAX_IMAGE as u64,
            "snapshot exceeds the experiment's 16 MiB limit or is empty",
        )?;
        let mut statement = db.prepare("select pgno, data from sqlite_dbpage order by pgno")?;
        let pages = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Pages>>()?;
        require(pages.len() == count as usize, "snapshot is missing pages")?;
        Ok(Self { page_size, pages })
    }

    pub(super) fn apply(&self, path: &Path, count: u32) -> Result<()> {
        initialize()?;
        if self.pages.is_empty() {
            let current: u32 =
                reader(path)?.pragma_query_value(None, "page_count", |row| row.get(0))?;
            return require(current == count, "empty epoch marker changes file size");
        }
        let mut db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.pragma_update(None, "page_size", self.page_size)?;
        db.pragma_update(None, "journal_mode", "wal")?;
        db.pragma_update(None, "synchronous", "full")?;
        // During a schema replacement SQLite may parse roots not installed yet.
        // This connection applies trusted pages only and is never a query handle.
        db.pragma_update(None, "writable_schema", true)?;
        let size: u32 = db.pragma_query_value(None, "page_size", |row| row.get(0))?;
        require(
            size == self.page_size,
            "page size differs: a new snapshot directory is required",
        )?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (sql, payload) = self.insert_statement(count);
        tx.execute(&sql, [payload])?;
        tx.commit()?;
        // dbpage changes the schema and file length behind the b-tree cache.
        // Close this handle; verification and queries use freshly opened readers.
        drop(db);
        Ok(())
    }

    fn insert_statement(&self, count: u32) -> (String, Vec<u8>) {
        let mut rows = Vec::new();
        let mut bytes = Vec::new();
        // One statement avoids schema re-preparation halfway through replacing
        // sqlite_schema's pages. One blob also avoids SQLite's parameter limit.
        for (number, page) in self.pages.iter().rev() {
            rows.push(format!(
                "({number}, substr(?1, {}, {}))",
                bytes.len() + 1,
                page.len()
            ));
            bytes.extend_from_slice(page);
        }
        // Truncation must come last: another page insert cancels it in dbpage.
        rows.push(format!("({}, null)", count + 1));
        (
            format!(
                "insert into sqlite_dbpage(pgno, data) values {}",
                rows.join(",")
            ),
            bytes,
        )
    }
}
