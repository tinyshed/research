//! An index directory divided by file and by extension: Tantivy's and
//! Lucene's segments keep each part of the index in a file of its own.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

pub fn files(dir: &Path) -> Value {
    let mut by_file = BTreeMap::new();
    let mut by_extension: BTreeMap<String, u64> = BTreeMap::new();
    let mut total = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let bytes = entry.metadata().unwrap().len();
        let name = entry.file_name().to_string_lossy().into_owned();
        let extension = name.rsplit_once('.').map_or(name.as_str(), |(_, ext)| ext).to_owned();
        *by_extension.entry(extension).or_default() += bytes;
        total += bytes;
        by_file.insert(name, bytes);
    }
    json!({"total": total, "by_extension": by_extension, "by_file": by_file})
}
