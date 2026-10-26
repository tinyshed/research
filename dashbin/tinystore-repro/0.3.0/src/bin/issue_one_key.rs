use tinystore::{Options, Store};

fn secrets(store: &Store) -> tinystore::Result<tinystore::kv::Bucket<String>> {
    store.bucket::<String>("secrets").encrypted().open()
}

fn main() -> tinystore::Result<()> {
    let work = std::env::temp_dir().join("one-key-repro");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();

    // the old server: a database password sealed, and a backup without the key, as the default is
    let old = Store::open(work.join("old"), Options::default())?;
    secrets(&old)?.set("pg-kazan", &"s3cret".to_owned())?;
    let backup = old.backup().write(work.join("backup.zip"))?;
    old.close()?;
    println!("backup: key left out {}", backup.key_left_out);

    // the new server, restored without the key
    tinystore::backup::restore(work.join("backup.zip"), work.join("new"))?;
    let new = Store::open(work.join("new"), Options::default())?;
    println!("open the bucket with no key: {:?}", secrets(&new).map(|_| "opened").map_err(|e| e.to_string()));
    new.close()?;

    // a new key, to enter the secrets again
    std::fs::write(work.join("new.key"), "ab".repeat(32)).unwrap();
    let with_new = |dir: &str, key: &str| Store::open(work.join(dir), Options { encryption_key_file: Some(work.join(key)), ..Options::default() });
    let new = with_new("new", "new.key")?;
    let bucket = secrets(&new)?;
    println!("with a new key, read the old secret: {:?}", bucket.get("pg-kazan").map_err(|e| e.to_string()));
    println!("with a new key, enter it again: {:?}", bucket.set("pg-kazan", &"s3cret".to_owned()).map_err(|e| e.to_string()));
    bucket.clear()?;
    bucket.set("pg-kazan", &"s3cret".to_owned())?;
    bucket.set("stripe-eu", &"rk_live_new".to_owned())?;
    println!("after clearing the bucket whole, the secrets entered again under the new key");
    new.close()?;

    // the old key turns up
    std::fs::copy(work.join("old").join("encryption.key"), work.join("old.key")).unwrap();
    let found = with_new("new", "old.key")?;
    let bucket = secrets(&found)?;
    println!("with the old key back, read pg-kazan: {:?}", bucket.get("pg-kazan").map_err(|e| e.to_string()));
    println!("with the old key back, read stripe-eu: {:?}", bucket.get("stripe-eu").map_err(|e| e.to_string()));
    found.close()
}
