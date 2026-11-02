// an audit event is written beside the change it records; the change rolls back, does the event?

use serde::Serialize;
use tinystore::{Options, Store};

#[derive(Serialize)]
struct RoleChanged {
    user_id: i64,
    role: &'static str,
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-051-audit");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let db = store.database("app").open()?;
    db.exec("create table roles (user_id integer primary key, role text not null)")?;
    let audit = store.events("audit").open()?;

    // the change and its audit line, in one transaction of the database; the second insert breaks it
    let attempt: tinystore::Result<()> = db.tx(|tx| {
        tx.exec(tinystore::sql!("insert into roles (user_id, role) values (?, ?)", 42, "admin"))?;
        audit.append("role.changed", &RoleChanged { user_id: 42, role: "admin" })?;
        tx.exec(tinystore::sql!("insert into roles (user_id, role) values (?, ?)", 42, "admin"))?;
        Ok(())
    });
    println!("the transaction: {}", attempt.map_or_else(|error| format!("failed, {error}"), |()| "committed".to_owned()));
    println!("rows in roles: {}", db.scalar::<i64>("select count(*) from roles")?);
    println!("events in audit: {}", audit.list().count()?);
    Ok(())
}
