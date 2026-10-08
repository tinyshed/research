use super::*;
use rusqlite::{Connection, OpenFlags};
use std::collections::BTreeSet;

fn equal(a: &[Sample], b: &[Sample]) {
    assert_eq!(a.len(), b.len());
    for (index, (a, b)) in a.iter().zip(b).enumerate() {
        assert_eq!(a.at, b.at, "timestamp {index}");
        assert_eq!(a.value.to_bits(), b.value.to_bits(), "value {index}");
    }
}
fn fixture(id: usize, count: usize) -> Vec<Sample> {
    let mut at = 1_700_000_000_000i64;
    (0..count)
        .map(|i| {
            let value = match id {
                0 => 42.0,
                1 => (i / 20 % 7) as f64 * 10.0,
                2 => ((i * 17 + id) % 997) as f64 / 10.0,
                3 => f64::from_bits(
                    0x3ff0000000000000
                        | ((i as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15) & 0x000fffffffffffff),
                ),
                4 => (i % 31) as f64,
                5 => f64::from_bits(((i % 2) as u64) << 63),
                6 => f64::from_bits((i % 7 + 1) as u64 | if i % 2 == 1 { 1u64 << 63 } else { 0 }),
                7 => [
                    f64::from_bits(0x7ff8000000000042),
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    0.0,
                    -0.0,
                    1.0,
                ][i % 6],
                8 => [f64::MAX, f64::MAX, -f64::MAX][i % 3],
                9 => [1e300, 1.0, -1e300][i % 3],
                10 => (i % 64) as f64,
                _ => i as f64 / 1000.0,
            };
            let point = Sample { at, value };
            at += if id == 4 { [1, 2, 1, 5][i % 4] } else { 1 };
            point
        })
        .collect()
}

#[test]
fn decode_go_production_fixture_stores() {
    let root = std::env::var_os("TINYSTORE_CODEC_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stores"));
    let mut totals = (0usize, 0usize, 0usize);
    let mut head_flags = BTreeSet::new();
    let mut value_modes = BTreeSet::new();
    let mut clock_modes = BTreeSet::new();
    let mut ordinary_flags = BTreeSet::new();
    let mut residual_modes = BTreeSet::new();
    for name in ["sealed", "ready", "head", "scrape", "edge"] {
        let path = root.join(name).join("metrics.db");
        if !path.exists() {
            panic!("Go fixture missing: {}", path.display());
        }
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut heads=db.prepare("select series_id,head_count,head_start,head_end,tail from series_state where tail is not null").unwrap();
        let rows = heads
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)? as usize,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Vec<u8>>(4)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (id, count, start, end, packed) = row.unwrap();
            let points = decode_head(id, count, start, end, &packed, 1 << 20, 16 << 20)
                .unwrap_or_else(|e| panic!("{name} head {id}: {e}"));
            assert_eq!(points.len(), count);
            totals.0 += 1;
            totals.2 += points.len();
            for chunk in parse_head(id, count, start, end, &packed, 1 << 20, 16 << 20).unwrap() {
                let flag = chunk.body[1];
                head_flags.insert((flag & 3, flag >> 2 & 7, flag >> 5));
            }
        }
        let mut groups=db.prepare("select g.series_id,g.start_ts,g.end_ts,g.directory,g.clock_id,c.body from groups g join clocks c on c.id=g.clock_id").unwrap();
        let rows = groups
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Vec<u8>>(5)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (id, start, end, directory, clock_id, clock_body) = row.unwrap();
            let clock = decode_clock_group(&clock_body).unwrap();
            let mut group = read_directory(id, start, end, clock_id, &directory, &clock).unwrap();
            for block in &mut group.blocks {
                if block.payload != 0 {
                    block.body = db
                        .query_row(
                            "select body from payloads where id=?",
                            [block.payload],
                            |r| r.get(0),
                        )
                        .unwrap();
                }
                let points = decode_block(block).unwrap_or_else(|e| {
                    panic!("{name} series {id} block {}: {e}", block.head.start)
                });
                assert_eq!(points.len(), block.head.count);
                value_modes.insert(if block.body.is_empty() {
                    255
                } else {
                    block.body[0]
                });
                clock_modes.insert(if block.clock.is_empty() {
                    255
                } else {
                    block.clock[0]
                });
                if !block.body.is_empty() {
                    let body = &block.body[..block.body.len() - 4];
                    let ordinary = if body[0] == 0 {
                        Some(body)
                    } else if body[0] == 2 {
                        let mut reader = binary::Reader::new(&body[1..]);
                        reader.byte().unwrap();
                        let length = reader.size(8200).unwrap();
                        let base = reader.take(length).unwrap();
                        residual_modes.insert(reader.remaining()[0]);
                        Some(base)
                    } else {
                        None
                    };
                    if let Some(ordinary) = ordinary {
                        let flags = ordinary[1];
                        let values = if flags >> 5 == 1 {
                            binary::expand(&ordinary[2..], 8192).unwrap()
                        } else {
                            ordinary[2..].to_vec()
                        };
                        let mode = flags >> 2 & 7;
                        ordinary_flags.insert((
                            mode,
                            flags >> 5,
                            if mode == 2 || mode == 4 {
                                values[0]
                            } else {
                                255
                            },
                        ));
                    }
                }
                totals.1 += 1;
                totals.2 += points.len();
            }
        }
    }
    assert_eq!(totals.1, 344);
    assert!(totals.0 >= 132);
    println!(
        "Go fixtures decoded: {} heads, {} blocks, {} samples",
        totals.0, totals.1, totals.2
    );
    println!(
        "Go head flags (time,value,zstd): {head_flags:?}; sealed value modes: {value_modes:?}; clock modes: {clock_modes:?}"
    );
    println!(
        "Go ordinary flags (value,zstd,integer packing): {ordinary_flags:?}; grid residual modes: {residual_modes:?}"
    );
}

#[test]
fn heads_all_edge_value_bits_and_irregular_clocks() {
    for id in 0..12 {
        let points = fixture(id, 481);
        let packed = encode_head(100 + id as i64, &points, 1000, 1 << 20).unwrap();
        let decoded = decode_head(
            100 + id as i64,
            481,
            points[0].at,
            points.last().unwrap().at,
            &packed,
            1000,
            1 << 20,
        )
        .unwrap();
        equal(&points, &decoded);
        assert!(
            decode_head(
                101 + id as i64,
                481,
                points[0].at,
                points.last().unwrap().at,
                &packed,
                1000,
                1 << 20
            )
            .is_err()
        );
        let mut damaged = packed.clone();
        damaged[5] ^= 1;
        assert!(
            parse_head(
                100 + id as i64,
                481,
                points[0].at,
                points.last().unwrap().at,
                &damaged,
                1000,
                1 << 20
            )
            .is_err()
        );
    }
}

#[test]
fn reuse_mutable_chunk_and_replace_tail() {
    let points = fixture(2, 2001);
    let packed = encode_head(7, &points, 10000, 1 << 20).unwrap();
    let chunks = parse_head(
        7,
        points.len(),
        points[0].at,
        points.last().unwrap().at,
        &packed,
        10000,
        1 << 20,
    )
    .unwrap();
    let kept = &chunks[..8];
    let mut tail = decode_chunks(&chunks[8..], i64::MIN, i64::MAX).unwrap();
    let at = tail.last().unwrap().at + 1;
    tail.push(Sample { at, value: -0.0 });
    let appended = encode_head_after(7, kept, &tail, 10000, 1 << 20).unwrap();
    let reparsed = parse_head(7, 2002, points[0].at, at, &appended, 10000, 1 << 20).unwrap();
    for (before, after) in kept.iter().zip(&reparsed) {
        assert_eq!(before.stored, after.stored);
    }
    let decoded = decode_chunks(&reparsed, i64::MIN, i64::MAX).unwrap();
    equal(&points, &decoded[..2001]);
    assert_eq!(decoded[2001].value.to_bits(), (-0.0f64).to_bits());
}

#[test]
fn sealed_group_directory_all_edge_values_round_trip() {
    for id in 0..12 {
        let points = fixture(id, 480);
        let mut group =
            prepare_group(1 + id as i64, &points, i32::from(id == 10), -2, 86400000).unwrap();
        group.clock_id = 1;
        for (slot, block) in group.blocks.iter_mut().enumerate() {
            if group.allocation & (1u32 << slot) != 0 {
                block.payload = slot as i64 + 1;
            }
        }
        let directory = write_directory(&group).unwrap();
        let clock = decode_clock_group(&group.clock_body).unwrap();
        let mut decoded = read_directory(
            group.series_id,
            group.start,
            group.end,
            group.clock_id,
            &directory,
            &clock,
        )
        .unwrap();
        let mut all = Vec::new();
        for (slot, block) in decoded.blocks.iter_mut().enumerate() {
            if block.payload != 0 {
                block.body = group.blocks[slot].body.clone();
            }
            all.extend(decode_block(block).unwrap());
        }
        equal(&points, &all);
        let mut bad = directory.clone();
        bad[2] ^= 1;
        assert!(
            read_directory(
                group.series_id,
                group.start,
                group.end,
                group.clock_id,
                &bad,
                &clock
            )
            .is_err()
        );
        assert!(
            read_directory(
                group.series_id + 1,
                group.start,
                group.end,
                group.clock_id,
                &directory,
                &clock
            )
            .is_err()
        );
    }
}

#[test]
fn huffman_native_single_stream_round_trip() {
    let symbols: Vec<u8> = (0..600)
        .map(|i| {
            if i % 11 == 0 {
                (i % 255) as u8
            } else {
                (i % 3) as u8
            }
        })
        .collect();
    let packed = huffman::compress(&symbols).expect("compressible symbols");
    assert_eq!(huffman::decode(&packed).unwrap(), symbols);
    assert!(huffman::decode(&[0, 0]).is_err());
}

#[test]
fn residual_modes_and_canonical_exact_sums() {
    let shapes: Vec<Vec<i64>> = vec![
        vec![0; 239],
        (0..239).map(|i| if i % 3 == 0 { -1 } else { 1 }).collect(),
        (0..239)
            .map(|i| if i % 31 == 0 { i as i64 * 10000 } else { 0 })
            .collect(),
        (0..239).map(|i| i as i64 - 119).collect(),
        (0..239)
            .map(|i| {
                if i % 5 == 0 {
                    i64::MIN + i as i64
                } else {
                    i64::MAX - i as i64
                }
            })
            .collect(),
    ];
    for shape in shapes {
        let packed = residuals::encode(&shape).unwrap();
        assert_eq!(residuals::decode(&packed, shape.len()).unwrap(), shape);
    }
    for value in [
        0.0,
        -0.0,
        f64::MAX,
        -f64::MAX,
        f64::from_bits(1),
        1.0,
        1e300,
    ] {
        let units = finite_units(value).unwrap();
        assert_eq!(read_exact_value(&encode_exact(&units)).unwrap(), units);
    }
    assert!(read_exact_value(&[1, 0, 2]).is_err());
    assert!(finite_units(f64::INFINITY).is_err());
}

#[test]
fn each_residual_decoder_mode_and_padding_guard() {
    let expected = vec![0, 1, -1, 2, -2];
    let mut compressed = vec![6];
    compressed.extend(binary::compress(&[0, 2, 1, 4, 3]).unwrap());
    let vectors = vec![
        (vec![0], vec![0; 5]),
        (vec![1, 0x15, 0x02], vec![1, 0, -1, 0, 1]),
        (vec![2, 3, 0x15, 0x9a, 0x01], vec![1, 0, -2, 0, 3]),
        (vec![3, 3, 0x50, 0x38], expected.clone()),
        (vec![4, 1, 2, 2, 3, 2, 6], vec![1, 0, -2, 0, 3]),
        (vec![5, 0, 2, 1, 4, 3], expected.clone()),
        (compressed, expected),
    ];
    for (data, want) in vectors {
        assert_eq!(
            residuals::decode(&data, 5).unwrap(),
            want,
            "residual mode {}",
            data[0]
        );
    }
    assert!(residuals::decode(&[3, 3, 0x50, 0xb8], 5).is_err());
    assert!(residuals::decode(&[1, 0x95, 0x02], 5).is_err());
    assert!(residuals::decode(&[4, 0, 2], 5).is_err());
    println!(
        "Explicit residual decoder modes covered: zero, signs, bitmap, packed, sparse, varint, compressed"
    );
}

#[test]
fn each_clock_decoder_mode_and_extent_guards() {
    let head = Head {
        start: 10,
        end: 20,
        count: 5,
        first: 0.0,
    };
    for clock in [
        vec![0, 1, 1, 2, 1, 6],
        vec![1, 1, 1, 1, 1, 2, 1, 1, 1, 6],
        vec![2, 1, 1, 2, 2, 2, 6],
    ] {
        let block = Block {
            head,
            clock,
            ..Default::default()
        };
        assert_eq!(clocks::decode_values(&block).unwrap(), [10, 11, 13, 14, 20]);
    }
    let regular = Block {
        head: Head {
            start: 10,
            end: 18,
            count: 5,
            first: 0.0,
        },
        ..Default::default()
    };
    assert_eq!(
        clocks::decode_values(&regular).unwrap(),
        [10, 12, 14, 16, 18]
    );
    let bad = Block {
        head,
        clock: vec![0, 0, 1, 2, 1, 6],
        ..Default::default()
    };
    assert!(clocks::decode_values(&bad).is_err());
    let bad = Block {
        head,
        clock: vec![0, 1, 1, 2, 1, 5],
        ..Default::default()
    };
    assert!(clocks::decode_values(&bad).is_err());
    println!("Explicit clock decoder modes covered: regular, plain, runs, exceptions");
}

#[test]
fn every_codec_envelope_time_value_and_compression_mode() {
    for time_mode in 0..3u8 {
        let times = match time_mode {
            0 => vec![],
            1 => vec![1, 3, 2],
            _ => vec![1, 2, 2],
        };
        let timestamps = match time_mode {
            0 => vec![10, 12, 14, 16],
            1 => vec![10, 11, 14, 16],
            _ => vec![10, 11, 13, 16],
        };
        for value_mode in 0..5u8 {
            let first = 7.0f64;
            let (values, want): (Vec<u8>, Vec<f64>) = match value_mode {
                0 => (
                    [8.0f64, 6.0, 9.0]
                        .iter()
                        .flat_map(|v| v.to_bits().to_le_bytes())
                        .collect::<Vec<u8>>(),
                    vec![7.0, 8.0, 6.0, 9.0],
                ),
                1 => (vec![], vec![7.0; 4]),
                2 => {
                    let mut out = vec![0];
                    out.extend_from_slice(&((4u64 << 60) | 2 | (3 << 3) | (2 << 6)).to_le_bytes());
                    (out, vec![7.0, 8.0, 6.0, 7.0])
                }
                3 => (vec![0], vec![7.0; 4]),
                _ => {
                    let mut out = vec![0, 1];
                    out.extend_from_slice(&((4u64 << 60) | 4 | (5 << 3) | (4 << 6)).to_le_bytes());
                    (out, vec![7.0, 7.2, 6.9, 7.1])
                }
            };
            let head = Head {
                start: 10,
                end: 16,
                count: 4,
                first,
            };
            for compressed in 0..2u8 {
                let mut stream = times.clone();
                stream.extend_from_slice(&values);
                if compressed == 1 {
                    stream = binary::compress(&stream).unwrap();
                }
                let mut body = vec![1, time_mode | (value_mode << 2) | (compressed << 5)];
                body.extend_from_slice(&(times.len() as u16).to_le_bytes());
                body.extend(stream);
                let mut key = Vec::new();
                key.extend_from_slice(&head.start.to_le_bytes());
                key.extend_from_slice(&head.end.to_le_bytes());
                key.extend_from_slice(&(head.count as u16).to_le_bytes());
                key.extend_from_slice(&first.to_bits().to_le_bytes());
                let crc = binary::crc32c(&[&key, &body]);
                body.extend_from_slice(&crc.to_le_bytes());
                let decoded = envelope::decode(head, &body).unwrap();
                for (i, p) in decoded.iter().enumerate() {
                    assert_eq!(p.at, timestamps[i]);
                    assert_eq!(p.value.to_bits(), want[i].to_bits());
                }
            }
        }
    }
    println!(
        "Explicit codec modes covered: fixed/delta/delta-delta times × raw/const/integer/XOR/scaled values × plain/zstd envelopes"
    );
}

#[test]
fn regular_clock_direct_fill_matches_baseline_and_errors() {
    for (count, start, end) in [
        (1, 42, 42),
        (2, i64::MIN, i64::MAX - 1),
        (240, -99, 239_000_000 - 99),
    ] {
        for value in [
            0.0,
            -0.0,
            f64::from_bits(0x7ff8000000000042),
            f64::INFINITY,
            f64::from_bits(1),
        ] {
            let block = Block {
                head: Head {
                    start,
                    end,
                    count,
                    first: value,
                },
                ..Default::default()
            };
            let mut baseline = values::decode(&block).unwrap();
            let times = clocks::decode_values(&block).unwrap();
            for (point, at) in baseline.iter_mut().zip(times) {
                point.at = at;
            }
            equal(&baseline, &sealed::decode_regular_block(&block).unwrap());
        }
    }
    for (count, start, end) in [
        (0, 0, 0),
        (241, 0, 240),
        (2, 2, 1),
        (1, 0, 1),
        (3, 0, 1),
        (3, 0, 0),
    ] {
        let block = Block {
            head: Head {
                start,
                end,
                count,
                first: 0.0,
            },
            ..Default::default()
        };
        let baseline = clocks::decode_values(&block).unwrap_err();
        let optimized = sealed::decode_regular_block(&block).unwrap_err();
        assert_eq!(baseline, optimized);
    }
}

#[test]
fn optimized_plain_stream_borrows_input_and_compression_stays_owned() {
    use std::borrow::Cow;
    let payload = [0x00, 0xff, 0x42, 0x80];
    let optimized = envelope::payload_stream(0, &payload, true).unwrap();
    assert!(matches!(optimized, Cow::Borrowed(_)));
    assert_eq!(optimized.as_ptr(), payload.as_ptr());
    let baseline = envelope::payload_stream(0, &payload, false).unwrap();
    assert!(matches!(baseline, Cow::Owned(_)));
    assert_eq!(baseline.as_ref(), optimized.as_ref());
    let packed = binary::compress(&payload).unwrap();
    for optimized in [false, true] {
        let expanded = envelope::payload_stream(1 << 5, &packed, optimized).unwrap();
        assert!(matches!(expanded, Cow::Owned(_)));
        assert_eq!(expanded.as_ref(), payload);
    }
}
