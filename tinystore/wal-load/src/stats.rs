use serde_json::{Value, json};
#[derive(Default, Debug, Clone)]
pub(super) struct Stats {
    pub wal_ns: u64,
    pub tree_ns: u64,
    pub encode_ns: u64,
    pub digest_ns: u64,
    pub append_ns: u64,
    pub sync_ns: u64,
    pub checkpoint_ns: u64,
    pub admission_ns: u64,
    pub capture_ns: u64,
    pub commits: u64,
    pub pages: u64,
    pub raw_bytes: u64,
    pub copied_bytes: u64,
    pub syncs: u64,
    pub checkpoints: u64,
    pub denied: u64,
    pub max_wal: u64,
    pub max_cache: usize,
}
impl Stats {
    pub(super) fn json(&self) -> Value {
        json!({"wal_ns":self.wal_ns,"tree_ns":self.tree_ns,"encode_ns":self.encode_ns,
        "digest_ns":self.digest_ns,"append_ns":self.append_ns,"sync_ns":self.sync_ns,"checkpoint_ns":self.checkpoint_ns,
        "admission_ns":self.admission_ns,"capture_ns":self.capture_ns,"commits":self.commits,"pages":self.pages,
        "raw_bytes":self.raw_bytes,"copied_bytes":self.copied_bytes,"syncs":self.syncs,"checkpoints":self.checkpoints,
        "denied":self.denied,"max_wal":self.max_wal,"max_cache":self.max_cache})
    }
}
pub(super) fn elapsed(start: std::time::Instant) -> u64 {
    start.elapsed().as_nanos() as u64
}
