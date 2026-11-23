//! What a hub did before packs: a database, a row written in a transaction and read back.

use tinystore::sql;

#[tokio::main(flavor = "current_thread")]
async fn main() -> tinystore::Result<()> {
    let dir = std::env::args().nth(1).expect("a directory");
    let store = tinystore::aio::Store::open(&dir, tinystore::Options::default())?;
    let db = store.database("hub").open()?;
    db.exec(sql!("create table if not exists packs (name text primary key, files text not null)")).await?;
    db.tx(|tx| -> tinystore::Result<()> {
        tx.exec(sql!("insert or replace into packs (name, files) values (?, ?)", "brands", "[]"))?;
        Ok(())
    })
    .await?;
    let kept: i64 = db.scalar(sql!("select count(*) from packs")).await?;
    println!("{kept}");
    Ok(())
}
