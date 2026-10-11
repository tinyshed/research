// Appended for a run to TinyStore's crates/tinystore/src/records/block_tests.rs
// on its `rust` branch, and removed after: what the C library's zstd makes of
// the corpus's text lines, a thousand and twenty-four a blob, each line ended
// by a break, at four levels. The Go side is ../go/zz_zstd_test.go, which
// compresses the same blobs with klauspost/compress.

#[test]
#[ignore = "scratch"]
fn zz_zstd_levels() {
    #[derive(serde::Deserialize)]
    struct Line {
        body: String,
    }
    let dir = std::env::var("TINYSTORE_DUMP").unwrap();
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().path()).collect();
    files.sort();
    let levels = [3, 7, 11, 19];
    let (mut sizes, mut raw, mut blobs, mut records) = ([0_usize; 4], 0, 0, 0);
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let bodies: Vec<String> = text.lines().map(|line| serde_json::from_str::<Line>(line).unwrap().body).collect();
        records += bodies.len();
        for chunk in bodies.chunks(1024) {
            let mut blob = chunk.join("\n");
            blob.push('\n');
            raw += blob.len();
            for (size, level) in sizes.iter_mut().zip(levels) {
                *size += zstd::bulk::compress(blob.as_bytes(), level).unwrap().len();
            }
            blobs += 1;
        }
    }
    let per = |bytes: usize| bytes as f64 / records as f64;
    println!(
        "rust records={records} blobs={blobs} raw={:.3} level3={:.3} level7={:.3} level11={:.3} level19={:.3}",
        per(raw),
        per(sizes[0]),
        per(sizes[1]),
        per(sizes[2]),
        per(sizes[3])
    );
}
