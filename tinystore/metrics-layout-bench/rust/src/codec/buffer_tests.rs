use super::*;

fn equal(a: &[Sample], b: &[Sample]) {
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a.at, b.at);
        assert_eq!(a.value.to_bits(), b.value.to_bits());
    }
}

fn points(shape: usize, count: usize) -> Vec<Sample> {
    (0..count)
        .map(|i| Sample {
            at: -1000 + i as i64 * 3 + if shape == 1 { (i / 7) as i64 } else { 0 },
            value: match shape {
                0 => -0.0,
                1 => (i / 17) as f64,
                2 => ((i * 97) % 499) as f64 / 100.0,
                3 => f64::from_bits((i as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15)),
                4 => [
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::from_bits(0x7ff0000000000001),
                    f64::from_bits(0xfff8000000000042),
                    f64::MAX,
                    -f64::MAX,
                    f64::from_bits(1),
                    f64::from_bits(0x8000000000000001),
                    0.0,
                    -0.0,
                ][i % 10],
                _ => (i % 31) as f64 / 10.0 + 0.0000000000000001,
            },
        })
        .collect()
}

#[test]
fn shared_heads_preserve_bytes_and_share_one_backing_allocation() {
    for shape in 0..6 {
        let input = points(shape, 721);
        let packed = encode_head(13, &input, 1000, 1 << 20).unwrap();
        let start = input[0].at;
        let end = input.last().unwrap().at;
        let owned =
            parse_head_storage(13, input.len(), start, end, &packed, 1000, 1 << 20, false).unwrap();
        let shared =
            parse_head_storage(13, input.len(), start, end, &packed, 1000, 1 << 20, true).unwrap();
        let HeadBytes::Shared { bytes: backing, .. } = &shared[0].body else {
            panic!("shared head body")
        };
        let mut baseline_bytes = 0;
        for (owned, shared) in owned.iter().zip(&shared) {
            assert_eq!(owned.body, shared.body);
            assert_eq!(owned.stored, shared.stored);
            baseline_bytes += owned.body.len() + owned.stored.len();
            for bytes in [&shared.body, &shared.stored] {
                let HeadBytes::Shared { bytes, range } = bytes else {
                    panic!("shared chunk bytes")
                };
                assert!(Arc::ptr_eq(backing, bytes));
                assert_eq!(range.len(), bytes[range.clone()].len());
            }
        }
        assert!(baseline_bytes > backing.len());
        equal(&input, &decode_chunks(&shared, i64::MIN, i64::MAX).unwrap());
        let mut out = Vec::with_capacity(input.len());
        decode_chunks_into(&shared, i64::MIN, i64::MAX, &mut out).unwrap();
        equal(&input, &out);
        // Kept chunks can still be copied byte-for-byte into a new mutable head.
        assert_eq!(
            encode_head_after(13, &owned[..2], &input[480..], 1000, 1 << 20).unwrap(),
            encode_head_after(13, &shared[..2], &input[480..], 1000, 1 << 20).unwrap()
        );
    }
}

#[test]
fn indexed_value_encoding_copy_can_be_removed_without_changing_bytes() {
    for shape in 0..6 {
        for count in [1, 2, 8, 31, 32, 239, 240] {
            let input = points(shape, count);
            assert_eq!(
                envelope::encode_value_stream(&input).unwrap(),
                envelope::encode_value_stream_direct(&input).unwrap()
            );
        }
    }
}

#[test]
fn reusable_block_output_preserves_all_float_bits_and_capacity() {
    let mut scratch = Vec::with_capacity(BLOCK_SAMPLES + 1);
    let sentinel = Sample {
        at: i64::MIN,
        value: f64::from_bits(0x7ff0000000000001),
    };
    let pointer = scratch.as_ptr();
    for shape in 0..6 {
        let input = points(shape, 481);
        let group = prepare_group(13, &input, 0, -2, 86400000).unwrap();
        let mut all = Vec::new();
        for block in &group.blocks {
            scratch.clear();
            scratch.push(sentinel);
            let reference = decode_block(block).unwrap();
            decode_block_into(block, &mut scratch).unwrap();
            assert_eq!(scratch.as_ptr(), pointer);
            equal(&[sentinel], &scratch[..1]);
            equal(&reference, &scratch[1..]);
            visit_block(block, |point| {
                all.push(point);
                Ok(())
            })
            .unwrap();
        }
        equal(&input, &all);
    }
}

fn trailing_value_block() -> Block {
    let mut block = Block {
        head: Head {
            start: 10,
            end: 11,
            count: 2,
            first: 7.0,
        },
        ..Default::default()
    };
    // Ordinary constant values with a valid checksum and an extra payload byte.
    block.body = values::seal(&block, vec![0, 4, 0]);
    block
}

#[test]
fn consumers_never_hide_trailing_corruption_and_decode_rolls_back() {
    let block = trailing_value_block();
    let expected = decode_block(&block).unwrap_err();
    assert_eq!(expected, "trailing binary fields");
    let mut visits = 0;
    assert_eq!(
        visit_block(&block, |_| {
            visits += 1;
            Err("consumer stopped".into())
        })
        .unwrap_err(),
        expected
    );
    assert_eq!(visits, 0);
    let sentinel = Sample {
        at: -1,
        value: -0.0,
    };
    let mut out = vec![sentinel];
    assert_eq!(decode_block_into(&block, &mut out).unwrap_err(), expected);
    equal(&[sentinel], &out);

    let input = points(2, 481);
    let packed = encode_head(13, &input, 1000, 1 << 20).unwrap();
    let mut chunks = parse_head_storage(
        13,
        input.len(),
        input[0].at,
        input.last().unwrap().at,
        &packed,
        1000,
        1 << 20,
        true,
    )
    .unwrap();
    chunks.last_mut().unwrap().body = vec![255].into();
    let expected = decode_chunks(&chunks, i64::MIN, i64::MAX).unwrap_err();
    assert_eq!(
        visit_chunks(&chunks, i64::MIN, i64::MAX, |_| {
            visits += 1;
            Err("consumer stopped".into())
        })
        .unwrap_err(),
        expected
    );
    assert_eq!(visits, 0);
    assert_eq!(
        decode_chunks_into(&chunks, i64::MIN, i64::MAX, &mut out).unwrap_err(),
        expected
    );
    equal(&[sentinel], &out);

    let mut both_bad = block.clone();
    both_bad.clock = vec![0, 0, 1];
    both_bad.body = vec![255];
    assert_eq!(
        decode_block_into(&both_bad, &mut out).unwrap_err(),
        decode_block(&both_bad).unwrap_err()
    );
    assert_eq!(decode_block(&both_bad).unwrap_err(), "clock quantum");
}

#[test]
fn every_truncation_keeps_partial_samples_private() {
    for shape in [0, 2, 3, 4, 5] {
        let input = points(shape, BLOCK_SAMPLES);
        let (head, body) = envelope::encode(&input).unwrap();
        let sentinel = Sample {
            at: -1,
            value: -0.0,
        };
        for length in 0..body.len() {
            let mut out = vec![sentinel];
            let expected = envelope::decode(head, &body[..length]).unwrap_err();
            assert_eq!(
                envelope::decode_into(head, &body[..length], &mut out).unwrap_err(),
                expected
            );
            equal(&[sentinel], &out);
        }
    }
}

#[test]
fn compressed_storage_is_reused_with_the_same_limits_and_errors() {
    let payload: Vec<u8> = (0..8192).map(|i| (i % 17) as u8).collect();
    let packed = binary::compress(&payload).unwrap();
    let mut scratch = Vec::new();
    binary::expand_into(&packed, 8192, &mut scratch).unwrap();
    let pointer = scratch.as_ptr();
    for _ in 0..3 {
        binary::expand_into(&packed, 8192, &mut scratch).unwrap();
        assert_eq!(scratch.as_ptr(), pointer);
        assert_eq!(scratch, payload);
    }
    for length in 0..packed.len() {
        match binary::expand(&packed[..length], 8192) {
            Ok(expected) => {
                binary::expand_into(&packed[..length], 8192, &mut scratch).unwrap();
                assert_eq!(scratch, expected);
            }
            Err(expected) => {
                assert_eq!(
                    binary::expand_into(&packed[..length], 8192, &mut scratch).unwrap_err(),
                    expected
                );
                assert!(scratch.is_empty());
            }
        }
    }
    for maximum in [0, 1, 4096, 8191] {
        assert_eq!(
            binary::expand_into(&packed, maximum, &mut scratch).unwrap_err(),
            binary::expand(&packed, maximum).unwrap_err()
        );
        assert!(scratch.is_empty());
    }
    for suffix in [vec![0], vec![255], packed.clone()] {
        let mut trailing = packed.clone();
        trailing.extend(suffix);
        match binary::expand(&trailing, 8192) {
            Ok(expected) => {
                binary::expand_into(&trailing, 8192, &mut scratch).unwrap();
                assert_eq!(scratch, expected);
            }
            Err(expected) => assert_eq!(
                binary::expand_into(&trailing, 8192, &mut scratch).unwrap_err(),
                expected
            ),
        }
    }

    let input = points(2, 240);
    let (head, body) = envelope::encode(&input).unwrap();
    let plain = if body[1] >> 5 == 1 {
        binary::expand(&body[4..body.len() - 4], 8192).unwrap()
    } else {
        body[4..body.len() - 4].to_vec()
    };
    let compressed = binary::compress(&plain).unwrap();
    let mut decoded = Vec::with_capacity(240);
    envelope::decode_compressed_into(
        head,
        body[1] | 32,
        u16::from_le_bytes(body[2..4].try_into().unwrap()) as usize,
        &compressed,
        &mut decoded,
    )
    .unwrap();
    equal(&input, &decoded);
}

#[test]
fn visitor_scratch_allows_recursion_and_does_not_retain_oversized_heads() {
    let input = points(0, 2);
    let group = prepare_group(13, &input, 0, -1, 86400000).unwrap();
    let block = &group.blocks[0];
    let mut outer = 0;
    let mut inner = 0;
    visit_block(block, |_| {
        outer += 1;
        visit_block(block, |_| {
            inner += 1;
            Ok(())
        })
    })
    .unwrap();
    assert_eq!((outer, inner), (2, 4));
    assert_eq!(
        visit_block(block, |_| Err("consumer stopped".into())).unwrap_err(),
        "consumer stopped"
    );
    visit_block(block, |_| Ok(())).unwrap();
    with_decode_scratch(|points| points.reserve_exact(RETAINED_DECODE_SAMPLES + 1));
    assert_eq!(DECODE_SCRATCH.with(|slot| slot.borrow().capacity()), 0);
}
