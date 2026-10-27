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

    // a new key, and a secret entered beside the old one, 0.4.0
    std::fs::write(work.join("new.key"), "ab".repeat(32)).unwrap();
    std::fs::copy(work.join("old").join("encryption.key"), work.join("old.key")).unwrap();
    let open = |older: Vec<std::path::PathBuf>| {
        Store::open(work.join("new"), Options { encryption_key_file: Some(work.join("new.key")), older_encryption_keys: older, ..Options::default() })
    };
    let new = open(vec![])?;
    let bucket = secrets(&new)?;
    println!("with a new key, read the old secret: {:?}", bucket.get("pg-kazan").map_err(|e| e.to_string()));
    println!("with a new key, write stripe-eu beside it: {:?}", bucket.set("stripe-eu", &"rk_live_new".to_owned()).map_err(|e| e.to_string()));
    new.close()?;

    // the old key turns up and goes among the older ones
    let found = open(vec![work.join("old.key")])?;
    let bucket = secrets(&found)?;
    println!("with the old key among the older ones, read pg-kazan: {:?}", bucket.get("pg-kazan").map_err(|e| e.to_string()));
    println!("and stripe-eu: {:?}", bucket.get("stripe-eu").map_err(|e| e.to_string()));
    println!("reseal: {:?}", bucket.reseal().map_err(|e| e.to_string()));
    found.close()?;

    // after the reseal the old key can go
    let after = open(vec![])?;
    let bucket = secrets(&after)?;
    println!("after the reseal, without the old key: pg-kazan {:?}, stripe-eu {:?}", bucket.get("pg-kazan").map_err(|e| e.to_string()), bucket.get("stripe-eu").map_err(|e| e.to_string()));
    after.close()
}
