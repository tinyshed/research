use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use super::packet::{Packet, Position};
use super::pages::Image;
use super::{Result, pages, require};

#[derive(Debug)]
pub(crate) struct Head {
    pub path: String,
    pub owned: bool,
    pub position: Position,
    pub page_size: u32,
}

pub(crate) struct Journal(Connection);

impl Journal {
    pub(crate) fn streams(&self) -> Result<Vec<String>> {
        let mut statement = self
            .0
            .prepare("select id from streams where sequence>0 order by id")?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn open(path: &Path) -> Result<Self> {
        let db = pages::writer(path)?;
        db.execute_batch(
            "create table if not exists streams(
                id text primary key, path text not null unique, owned integer not null,
                epoch integer not null, sequence integer not null, checksum blob not null,
                page_size integer not null);
             create table if not exists pages(
                stream text not null, number integer not null, data blob not null,
                primary key(stream, number)) without rowid;
             create table if not exists entries(
                stream text not null, sequence integer not null, packet blob not null,
                applied integer not null, primary key(stream, sequence)) without rowid;",
        )?;
        Ok(Self(db))
    }

    pub(crate) fn head(&self, stream: &str) -> Result<Option<Head>> {
        Ok(self
            .0
            .query_row(
                "select path, owned, epoch, sequence, checksum, page_size from streams where id=?1",
                [stream],
                |row| {
                    let hash: Vec<u8> = row.get(4)?;
                    let checksum = hash.try_into().map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(Head {
                        path: row.get(0)?,
                        owned: row.get(1)?,
                        page_size: row.get(5)?,
                        position: Position {
                            epoch: read_position(row, 2)?,
                            sequence: read_position(row, 3)?,
                            checksum,
                        },
                    })
                },
            )
            .optional()?)
    }

    pub(crate) fn publish(&self, stream: &str, path: &str) -> Result<()> {
        self.0.execute(
            "insert into streams values(?1, ?2, 1, 1, 0, ?3, 0)",
            params![stream, path, [0u8; 32].as_slice()],
        )?;
        Ok(())
    }

    pub(crate) fn image(&self, stream: &str, page_size: u32) -> Result<Image> {
        let mut statement = self
            .0
            .prepare("select number, data from pages where stream=?1 order by number")?;
        let pages = statement
            .query_map([stream], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Image { page_size, pages })
    }

    pub(crate) fn stage(&self, packet: &Packet, bytes: &[u8]) -> Result<()> {
        self.0.execute(
            "insert into entries values(?1, ?2, ?3, 0)",
            params![
                packet.stream,
                i64::try_from(packet.position.sequence)?,
                bytes
            ],
        )?;
        Ok(())
    }

    pub(crate) fn finish(&mut self, packet: &Packet, bytes: &[u8], local: bool) -> Result<()> {
        let tx = self.0.transaction()?;
        if packet.snapshot {
            tx.execute("delete from pages where stream=?1", [&packet.stream])?;
            tx.execute("delete from entries where stream=?1", [&packet.stream])?;
        }
        tx.execute(
            "insert or ignore into streams values(?1, ?2, 0, 1, 0, ?3, 0)",
            params![
                packet.stream,
                format!("replicas/{}.db", packet.stream),
                [0u8; 32].as_slice()
            ],
        )?;
        for (number, page) in &packet.pages {
            tx.execute(
                "insert or replace into pages values(?1, ?2, ?3)",
                params![packet.stream, number, page],
            )?;
        }
        tx.execute(
            "delete from pages where stream=?1 and number>?2",
            params![packet.stream, packet.page_count],
        )?;
        tx.execute(
            "update streams set epoch=?2, sequence=?3, checksum=?4, page_size=?5 where id=?1",
            params![
                packet.stream,
                i64::try_from(packet.position.epoch)?,
                i64::try_from(packet.position.sequence)?,
                packet.position.checksum.as_slice(),
                packet.page_size
            ],
        )?;
        if local {
            tx.execute("update streams set owned=1 where id=?1", [&packet.stream])?;
        }
        tx.execute(
            "insert or replace into entries values(?1, ?2, ?3, 1)",
            params![
                packet.stream,
                i64::try_from(packet.position.sequence)?,
                bytes
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn pending(&self) -> Result<Vec<Vec<u8>>> {
        let mut query = self
            .0
            .prepare("select packet from entries where applied=0 order by stream, sequence")?;
        Ok(query
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn ready(&self, stream: &str) -> Result<()> {
        let count: u32 = self.0.query_row(
            "select count(*) from entries where stream=?1 and applied=0",
            [stream],
            |row| row.get(0),
        )?;
        require(
            count == 0,
            "stream has an unfinished apply; reopen to recover before reading or replicating",
        )
    }

    pub(crate) fn entry(&self, stream: &str, sequence: u64) -> Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .query_row(
                "select packet from entries where stream=?1 and sequence=?2 and applied=1",
                params![stream, i64::try_from(sequence)?],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(crate) fn prune(&self, stream: &str, through: u64) -> Result<()> {
        self.ready(stream)?;
        self.0.execute(
            "delete from entries where stream=?1 and sequence<=?2 and applied=1",
            params![stream, i64::try_from(through)?],
        )?;
        Ok(())
    }
}

fn read_position(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(column)?).map_err(|_| rusqlite::Error::InvalidQuery)
}
