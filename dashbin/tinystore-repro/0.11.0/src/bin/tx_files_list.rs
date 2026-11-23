//! A folder of files replaced inside a transaction: the files of the old version that the new one
//! lacks stay, because a transaction can put, delete and head a path and cannot list a folder.

use tinystore::{Store, sql};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join(format!("ts-repro-tx-files-{}", std::process::id()));
    let store = Store::open(&dir, Default::default())?;
    let db = store.database("app").open()?;
    db.exec(sql!("create table packs (name text primary key, version integer not null)"))?;
    let packs = db.files("packs").open()?;

    // version 1 of the pack `brands`: two icons, and its row, in one transaction
    db.tx(|tx| {
        let files = tx.with(&packs);
        files.put("brands/icons/youtube.svg", b"<svg/>")?;
        files.put("brands/icons/vimeo.svg", b"<svg/>")?;
        tx.exec(sql!("insert into packs (name, version) values ('brands', 1)"))?;
        Ok::<_, tinystore::Error>(())
    })?;

    // version 2 has one icon. To replace the pack whole the transaction would remove what the
    // folder holds and write the new files, but `tx.with(&packs)` has no list and no clear:
    //     files.list().prefix("brands/")   // no such method on TxFiles
    //     files.folder("brands").clear()   // nor this
    db.tx(|tx| {
        let files = tx.with(&packs);
        files.put("brands/icons/youtube.svg", b"<svg/>")?;
        tx.exec(sql!("update packs set version = 2 where name = 'brands'"))?;
        Ok::<_, tinystore::Error>(())
    })?;

    let left: Vec<String> = packs.list().prefix("brands/").page()?.files.into_iter().map(|file| file.path).collect();
    println!("version 2 holds youtube.svg alone; the folder holds {left:?}");
    drop((packs, db, store));
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
