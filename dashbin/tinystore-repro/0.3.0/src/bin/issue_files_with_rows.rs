use tinystore::{Options, Store, sql};

// the plugins an app installed: a row naming each package, and the package's bytes in a set of files
const MIGRATION: [(&str, &str); 1] = [("0001_plugins.sql", "create table plugins (file text primary key) strict")];

fn open(dir: &std::path::Path) -> tinystore::Result<(Store, tinystore::sql::Database)> {
    let store = Store::open(dir, Options::default())?;
    let db = store.database("app").migrations(MIGRATION).open()?;
    Ok((store, db))
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("files-with-rows-repro");
    match std::env::args().nth(1).as_deref() {
        // the child: two writes, and the process dies between them, as a crash would
        Some("file-then-row") => {
            let (store, _db) = open(&dir)?;
            store.files("packages").open()?.put("stripe-1.0.0.js", b"export const charges = 1")?;
            std::process::exit(1);
        }
        Some("row-then-file") => {
            let (_store, db) = open(&dir)?;
            db.exec(sql!("insert into plugins (file) values (?)", "slack-1.0.0.js"))?;
            std::process::exit(1);
        }
        _ => {}
    }
    let _ = std::fs::remove_dir_all(&dir);
    let me = std::env::current_exe().unwrap();
    for order in ["file-then-row", "row-then-file"] {
        let status = std::process::Command::new(&me).arg(order).status().unwrap();
        println!("{order}: the process died between the two writes ({status})");
    }
    let (store, db) = open(&dir)?;
    let files = store.files("packages").open()?;
    for name in ["stripe-1.0.0.js", "slack-1.0.0.js"] {
        let row: Option<String> = db.one(sql!("select file from plugins where file = ?", name))?;
        println!("  {name}: row {}, file {}", if row.is_some() { "kept" } else { "none" }, if files.get(name)?.is_some() { "kept" } else { "none" });
    }
    store.close()
}
