use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

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
}
