// a real node_exporter's /metrics, saved to a file: how many samples, help and unit lines it has, and what ingest keeps of them
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let path = std::env::args().nth(1).expect("the saved /metrics text");
    let text = std::fs::read_to_string(&path).unwrap();
    let lines = |prefix: &str| text.lines().filter(|l| l.starts_with(prefix)).count();
    let samples = text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')).count();
    println!("{} bytes: {samples} samples, {} # HELP, {} # TYPE, {} # UNIT", text.len(), lines("# HELP"), lines("# TYPE"), lines("# UNIT"));

    let dir = std::env::temp_dir().join("node-exporter-text-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;
    println!("ingest: {:?}", stats.prometheus().label("instance", "node-1").ingest(&text).map_err(|e| e.to_string()));
    for name in ["node_memory_MemAvailable_bytes", "node_cpu_seconds_total", "node_network_receive_bytes_total"] {
        let kept = stats.description(name)?;
        println!("  {name}: help {:?}, unit {:?}", kept.help, kept.unit);
    }
    store.close()
}
