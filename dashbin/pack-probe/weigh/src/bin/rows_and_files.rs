//! The same, and a pack's files kept beside the row as dashbin keeps them: a folder of a set of
//! files in the database, put and deleted in the row's transaction, one read back whole.

use tinystore::sql;

#[tokio::main(flavor = "current_thread")]
async fn main() -> tinystore::Result<()> {
    let dir = std::env::args().nth(1).expect("a directory");
    let store = tinystore::aio::Store::open(&dir, tinystore::Options::default())?;
    let db = store.database("hub").open()?;
    db.exec(sql!("create table if not exists packs (name text primary key, files text not null)")).await?;
    let packs = db.files("packs").open()?;
    let folder = packs.folder("brands");
    db.tx(move |tx| -> tinystore::Result<()> {
        let files = tx.with(folder.blocking());
        files.delete("icons/old.svg")?;
        files.key("icons/youtube.svg").content_type("image/svg+xml").put(b"<svg/>")?;
        tx.exec(sql!("insert or replace into packs (name, files) values (?, ?)", "brands", "[\"icons/youtube.svg\"]"))?;
        Ok(())
    })
    .await?;
    let kept: i64 = db.scalar(sql!("select count(*) from packs")).await?;
    let stored = packs.folder("brands").get("icons/youtube.svg").await?.expect("a file put");
    println!("{kept} {} {}", stored.info().etag, stored.read_all().await?.len());
    Ok(())
}
