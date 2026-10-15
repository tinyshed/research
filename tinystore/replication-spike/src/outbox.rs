//! SDK-side intents, separate from copied files. SQL here is trusted demo input.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use tinystore::sql::{Database, Sql};

use crate::{Result, pages, require};

pub(super) struct Outbox(Connection);

impl Outbox {
    pub(super) fn open(path: &Path) -> Result<Self> {
        let db = pages::writer(path)?;
        db.execute_batch(
            "create table if not exists pending(id text primary key, sql text not null)",
        )?;
        Ok(Self(db))
    }

    pub(super) fn enqueue(&self, id: &str, sql: &str) -> Result<()> {
        let previous = self
            .0
            .query_row("select sql from pending where id=?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        if let Some(previous) = previous {
            return require(previous == sql, "request id was reused for another intent");
        }
        self.0
            .execute("insert into pending values(?1, ?2)", params![id, sql])?;
        Ok(())
    }

    pub(super) fn pending(&self) -> Result<u32> {
        Ok(self
            .0
            .query_row("select count(*) from pending", [], |row| row.get(0))?)
    }

    pub(super) fn drain(&self, mut deliver: impl FnMut(&str, &str) -> Result<()>) -> Result<usize> {
        let mut query = self
            .0
            .prepare("select id, sql from pending order by rowid limit 32")?;
        let requests: Vec<(String, String)> = query
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, sql) in &requests {
            deliver(id, sql)?;
            self.0.execute("delete from pending where id=?1", [id])?;
        }
        Ok(requests.len())
    }

    pub(super) fn apply(db: &Database, id: &str, sql: &str) -> Result<()> {
        db.exec(
            "create table if not exists _spike_requests(id text primary key, sql text not null)",
        )?;
        db.tx::<_, tinystore::Error>(|tx| {
            let previous: Option<String> =
                tx.scalar(Sql::new("select max(sql) from _spike_requests where id=?1").bind(id))?;
            if let Some(previous) = previous {
                if previous != sql {
                    return Err(tinystore::Error::invalid("request id was reused"));
                }
                return Ok(());
            }
            tx.exec(sql)?;
            tx.exec(
                Sql::new("insert into _spike_requests values(?1, ?2)")
                    .bind(id)
                    .bind(sql),
            )?;
            Ok(())
        })?;
        Ok(())
    }
}
