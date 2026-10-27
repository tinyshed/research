use tinystore::{Options, Store, sql};

// the plugins an app installed: a row naming each package, and the package's bytes in a set of files
// opened inside the app's database, 0.4.0, so the two commit in one transaction
const MIGRATION: [(&str, &str); 1] = [("0001_plugins.sql", "create table plugins (file text primary key) strict")];

fn open(dir: &std::path::Path) -> tinystore::Result<(Store, tinystore::sql::Database)> {
    let store = Store::open(dir, Options::default())?;
    let db = store.database("app").migrations(MIGRATION).open()?;
    Ok((store, db))
}

fn install(db: &tinystore::sql::Database, name: &str, then: impl Fn()) -> tinystore::Result<()> {
    let packages = db.files("packages").open()?;
    db.tx(|tx| {
        tx.exec(sql!("insert into plugins (file) values (?)", name))?;
        tx.with(&packages).put(name, b"export const charges = 1")?;
        then();
        Ok::<_, tinystore::Error>(())
    })
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("files-with-rows-repro");
    match std::env::args().nth(1).as_deref() {
        // the child: the row and the file written in one transaction, and the process dies before it commits
        Some("crash-before-commit") => {
            let (_store, db) = open(&dir)?;
            install(&db, "slack-1.0.0.js", || std::process::exit(1))?;
            return Ok(());
        }
        _ => {}
    }
    let _ = std::fs::remove_dir_all(&dir);
    {
        let (store, db) = open(&dir)?;
        install(&db, "stripe-1.0.0.js", || {})?;
        let packages = db.files("packages").open()?;
        let rolled = db.tx(|tx| {
            tx.exec(sql!("insert into plugins (file) values (?)", "notion-1.0.0.js"))?;
            tx.with(&packages).put("notion-1.0.0.js", b"export const pages = 1")?;
            Err::<(), _>(tinystore::Error::invalid("the install was cancelled"))
        });
        println!("notion: the transaction rolled back ({})", rolled.map_err(|e| e.to_string()).unwrap_err());
        store.close()?;
    }
    let me = std::env::current_exe().unwrap();
    let status = std::process::Command::new(&me).arg("crash-before-commit").status().unwrap();
    println!("slack: the process died inside the transaction ({status})");

    let (store, db) = open(&dir)?;
    let files = db.files("packages").open()?;
    for name in ["stripe-1.0.0.js", "notion-1.0.0.js", "slack-1.0.0.js"] {
        let row: Option<String> = db.one(sql!("select file from plugins where file = ?", name))?;
        println!("  {name}: row {}, file {}", if row.is_some() { "kept" } else { "none" }, if files.get(name)?.is_some() { "kept" } else { "none" });
    }
    let left: Vec<String> = std::fs::read_dir(dir.join("sql"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .flat_map(|d| walk(&d.path()))
        .collect();
    println!("bytes left beside the database after reopening: {left:?}");
    store.close()
}

fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        if entry.path().is_dir() {
            found.extend(walk(&entry.path()));
        } else {
            found.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    found
}
