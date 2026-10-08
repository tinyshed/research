use super::*;

fn fixture() -> (Connection, Vec<SeriesRead>, Vec<Vec<Sample>>) {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE TABLE payloads(id INTEGER PRIMARY KEY,body BLOB)")
        .unwrap();
    let mut reads = Vec::new();
    let mut expected = Vec::new();
    for id in 1..=20 {
        let points: Vec<_> = (0..480)
            .map(|i| Sample {
                at: 1700000000000 + i,
                value: ((i * 17 + id) % 997) as f64 / 10.0,
            })
            .collect();
        let mut group = codec::prepare_group(id, &points, 0, -1, 86400000).unwrap();
        for (slot, block) in group.blocks.iter_mut().enumerate() {
            block.payload = id * 10 + slot as i64;
            connection
                .execute(
                    "INSERT INTO payloads VALUES(?,?)",
                    params![block.payload, block.body],
                )
                .unwrap();
            block.body.clear();
        }
        reads.push(SeriesRead {
            series: Series::default(),
            head: Vec::new(),
            blocks: group
                .blocks
                .into_iter()
                .map(|block| SelectedBlock {
                    block,
                    summarized: false,
                })
                .collect(),
        });
        expected.push(points);
    }
    (connection, reads, expected)
}

#[test]
fn arena_bytes_survive_statement_connection_and_move_to_worker() {
    let (connection, mut reads, expected) = fixture();
    fetch_payloads_mode(&connection, &mut reads, true).unwrap();
    assert!(connection.is_autocommit());
    drop(connection);
    let first = reads[0].blocks[0]
        .block
        .shared_body
        .as_ref()
        .unwrap()
        .0
        .clone();
    for read in &reads {
        for selected in &read.blocks {
            let (bytes, _) = selected.block.shared_body.as_ref().unwrap();
            assert!(std::sync::Arc::ptr_eq(bytes, &first));
            assert!(selected.block.body.is_empty());
        }
    }
    std::thread::spawn(move || {
        for (read, wanted) in reads.iter().zip(expected) {
            let got: Vec<_> = read
                .blocks
                .iter()
                .flat_map(|selected| codec::decode_block(&selected.block).unwrap())
                .collect();
            assert_eq!(got.len(), wanted.len());
            for (a, b) in got.iter().zip(wanted) {
                assert_eq!((a.at, a.value.to_bits()), (b.at, b.value.to_bits()));
            }
        }
    })
    .join()
    .unwrap();
}

#[test]
fn arena_preserves_missing_size_repeat_and_checksum_failures() {
    for arena in [false, true] {
        for scenario in 0..4 {
            let (connection, mut reads, _) = fixture();
            let id = reads[0].blocks[0].block.payload;
            match scenario {
                0 => {
                    connection
                        .execute("DELETE FROM payloads WHERE id=?", [id])
                        .unwrap();
                }
                1 => {
                    connection
                        .execute("UPDATE payloads SET body=x'00' WHERE id=?", [id])
                        .unwrap();
                }
                2 => {
                    reads[1].blocks[0].block.payload = id;
                }
                3 => {
                    let size = reads[0].blocks[0].block.body_bytes;
                    connection
                        .execute(
                            "UPDATE payloads SET body=zeroblob(?) WHERE id=?",
                            params![size as i64, id],
                        )
                        .unwrap();
                }
                _ => unreachable!(),
            }
            let fetched = fetch_payloads_mode(&connection, &mut reads, arena);
            if scenario == 3 {
                fetched.unwrap();
                assert!(codec::decode_block(&reads[0].blocks[0].block).is_err());
            } else {
                assert!(
                    fetched
                        .unwrap_err()
                        .to_string()
                        .contains("corrupt metrics data")
                );
            }
            assert!(connection.is_autocommit());
        }
    }
}
