use std::fmt::Write;

use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("prometheus-rest-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;

    // 1. help and unit: what node_exporter writes above every family
    let text = "# HELP node_memory_MemAvailable_bytes Memory information field MemAvailable_bytes.\n\
                # TYPE node_memory_MemAvailable_bytes gauge\n\
                # UNIT node_memory_MemAvailable_bytes bytes\n\
                node_memory_MemAvailable_bytes 8.123e+09\n";
    stats.prometheus().label("instance", "web-1").ingest(text)?;
    let kept = stats.description("node_memory_MemAvailable_bytes")?;
    println!("help and unit after ingesting the family: help {:?}, unit {:?}", kept.help, kept.unit);

    // 2. a scrape past what one ingest carries: a big exporter, kube-state-metrics or a busy cadvisor
    let mut big = String::from("# TYPE kube_pod_status_ready gauge\n");
    for pod in 0..12_000 {
        writeln!(big, "kube_pod_status_ready{{pod=\"pod-{pod}\",condition=\"true\"}} 1").unwrap();
    }
    let answer = stats.prometheus().label("instance", "ksm").ingest(&big);
    println!("12000 samples in one scrape: {:?}", answer.map_err(|e| e.to_string()));

    // 3. prometheus 3's quoted utf-8 names, as an otel collector exports them
    let utf8 = "# TYPE \"http.server.request.duration\" gauge\n{\"http.server.request.duration\",method=\"GET\"} 0.25\n";
    let answer = stats.prometheus().label("instance", "otel").ingest(utf8);
    println!("a quoted utf-8 name: {:?}", answer.map_err(|e| e.to_string()));
    store.close()
}
