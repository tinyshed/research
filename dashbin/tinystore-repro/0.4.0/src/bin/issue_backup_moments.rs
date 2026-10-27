use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tinystore::metrics::Series;
use tinystore::{Options, Store, sql};

// a collector's cursor in the app's database, saying how far its samples in metrics go
const MIGRATION: [(&str, &str); 1] = [("0001_cursor.sql", "create table cursor (id integer primary key, upto integer not null) strict; insert into cursor values (1, -1);")];

fn main() -> tinystore::Result<()> {
    let work = std::env::temp_dir().join("backup-moments-repro");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let base = UNIX_EPOCH + Duration::from_secs(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 3600);

    let store = Store::open(work.join("data"), Options::default())?;
    // a backup copies the engines in the order they were opened; `metrics-first` opens metrics before the database
    let metrics_first = std::env::args().nth(1).as_deref() == Some("metrics-first");
    let early = if metrics_first { Some(store.metrics()?) } else { None };
    let db = store.database("app").migrations(MIGRATION).open()?;
    let stats = match early {
        Some(stats) => stats,
        None => store.metrics()?,
    };
    println!("opened {}", if metrics_first { "metrics, then the database" } else { "the database, then metrics" });
    let stop = AtomicBool::new(false);
    let (mut ahead, mut behind, mut same) = (0, 0, 0);

    std::thread::scope(|scope| -> tinystore::Result<()> {
        // the collector: a sample, then the cursor that says it is kept, again and again
        scope.spawn(|| {
            let mut i: i64 = 0;
            while !stop.load(Ordering::Relaxed) {
                let at = base + Duration::from_millis(i as u64 * 10);
                stats.ingest(&[Series::gauge("reading").label("source", "live").sample(at, i as f64)]).unwrap();
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
            let read = again.metrics()?.query().name("reading").from(base).to(SystemTime::now()).read()?;
            let newest = read.series.iter().flat_map(|s| s.samples().iter().map(|x| x.value as i64)).max().unwrap_or(-1);
            again.close()?;
            match cursor.cmp(&newest) {
                std::cmp::Ordering::Greater => ahead += 1,
                std::cmp::Ordering::Less => behind += 1,
                std::cmp::Ordering::Equal => same += 1,
            }
            println!("backup {n:>2}: the cursor says {cursor:>6}, the newest sample kept is {newest:>6}");
        }
        stop.store(true, Ordering::Relaxed);
        Ok(())
    })?;
    store.close()?;
    println!("of 20 backups: the cursor ahead of the samples {ahead}, behind them {behind}, the same {same}");
    Ok(())
}
