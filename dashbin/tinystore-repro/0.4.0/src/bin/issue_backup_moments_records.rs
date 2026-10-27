use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tinystore::{Options, Store, sql};

// a flow's state in the app's database, saying which of its events in records it has kept
const MIGRATION: [(&str, &str); 1] = [("0001_cursor.sql", "create table cursor (id integer primary key, upto integer not null) strict; insert into cursor values (1, -1);")];

#[derive(serde::Serialize, serde::Deserialize)]
struct Transition {
    n: i64,
}

fn main() -> tinystore::Result<()> {
    let work = std::env::temp_dir().join("backup-moments-records-repro");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();

    let store = Store::open(work.join("data"), Options::default())?;
    let db = store.database("app").migrations(MIGRATION).open()?;
    let events = store.events("alerts").open()?;
    let stop = AtomicBool::new(false);
    let (mut ahead, mut behind, mut same) = (0, 0, 0);

    std::thread::scope(|scope| -> tinystore::Result<()> {
        // the flow: an alert's transition kept as an event, then the state that says it is kept
        scope.spawn(|| {
            let mut i: i64 = 0;
            while !stop.load(Ordering::Relaxed) {
                events.event("transition").id(i.to_string()).append(&Transition { n: i }).unwrap();
                db.exec(sql!("update cursor set upto = ? where id = 1", i)).unwrap();
                i += 1;
            }
        });
        for n in 0..20 {
            std::thread::sleep(Duration::from_millis(150));
            let zip = work.join(format!("backup-{n}.zip"));
            store.backup().write(&zip)?;
            let restored = work.join(format!("restored-{n}"));
            tinystore::backup::restore(&zip, &restored)?;
            let again = Store::open(&restored, Options { background: false, ..Options::default() })?;
            let cursor: i64 = again.database("app").migrations(MIGRATION).open()?.scalar(sql!("select upto from cursor where id = 1"))?;
            let kept = again.events("alerts").open()?.list().since(Duration::from_secs(3600)).all().count() as i64;
            again.close()?;
            let newest = kept - 1;
            match cursor.cmp(&newest) {
                std::cmp::Ordering::Greater => ahead += 1,
                std::cmp::Ordering::Less => behind += 1,
                std::cmp::Ordering::Equal => same += 1,
            }
            println!("backup {n:>2}: the state says {cursor:>6}, the newest event kept is {newest:>6}");
        }
        stop.store(true, Ordering::Relaxed);
        Ok(())
    })?;
    store.close()?;
    println!("of 20 backups: the state ahead of the events {ahead}, behind them {behind}, the same {same}");
    Ok(())
}
