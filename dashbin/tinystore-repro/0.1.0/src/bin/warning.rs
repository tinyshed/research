// built without LIBSQLITE3_FLAGS: the one warning about it is lost when the logger is set up as the docs show
use tinystore::{Options, Store};
use tracing_subscriber::prelude::*;

fn main() -> tinystore::Result<()> {
    let listen_first = std::env::args().any(|a| a == "--listen-first");
    let dir = std::env::temp_dir().join("ts-repro-warning");
    let _ = std::fs::remove_dir_all(&dir);

    if listen_first {
        tracing_subscriber::fmt().with_target(true).init();
        let store = Store::open(&dir, Options::default())?;
        let users = store.bucket::<String>("users").open()?;
        users.set("42", &"ada".to_string())?;
        tracing::info!("bucket written");
        return store.close();
    }

    // the order from "your first store"
    let store = Store::open(&dir, Options::default())?;
    let log = store.logger("hello").layer()?;
    tracing_subscriber::registry().with(log).init();
    let users = store.bucket::<String>("users").open()?;
    users.set("42", &"ada".to_string())?;
    tracing::info!("bucket written");
    store.close()
}
