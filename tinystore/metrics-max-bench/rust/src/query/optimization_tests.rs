use super::*;

const OPS: [&str; 8] = [
    "count", "sum", "avg", "min", "max", "delta", "increase", "rate",
];

#[derive(Debug, PartialEq)]
struct BucketBits(i64, i64, usize, usize, u64, bool, bool);

fn bits(bucket: AggregateBucket) -> BucketBits {
    BucketBits(
        bucket.from,
        bucket.to,
        bucket.count,
        bucket.resets,
        bucket.value.to_bits(),
        bucket.overflow,
        bucket.partial,
    )
}

fn checked(origin: i64, from: i64, to: i64) -> Checked {
    Checked {
        origin,
        from,
        to,
        limits: Limits::defaults(),
        matchers: BTreeMap::new(),
        conditions: BTreeMap::new(),
    }
}

fn request(op: &str, width: i64) -> AggregateRequest {
    AggregateRequest {
        query: Query::default(),
        width,
        op: op.into(),
        by: None,
        without: None,
    }
}

fn points(start: i64, values: &[f64]) -> Vec<Sample> {
    values
        .iter()
        .enumerate()
        .map(|(index, &value)| Sample {
            at: start + index as i64,
            value,
        })
        .collect()
}

fn read(kind: &str, blocks: &[(&[Sample], bool)], head: &[Sample]) -> SeriesRead {
    let blocks = blocks
        .iter()
        .flat_map(|(samples, summarized)| {
            codec::prepare_group(1, samples, i32::from(kind == "counter"), -2, i64::MAX)
                .unwrap()
                .blocks
                .into_iter()
                .map(|block| SelectedBlock {
                    block,
                    summarized: *summarized,
                })
        })
        .collect();
    let head = if head.is_empty() {
        Vec::new()
    } else {
        let packed = codec::encode_head_after(1, &[], head, 4096, 1 << 20).unwrap();
        codec::parse_head(
            1,
            head.len(),
            head[0].at,
            head.last().unwrap().at,
            &packed,
            4096,
            1 << 20,
        )
        .unwrap()
    };
    SeriesRead {
        series: Series {
            name: "equivalence".into(),
            kind: kind.into(),
            labels: BTreeMap::new(),
        },
        blocks,
        head,
    }
}

fn collect(
    read: &SeriesRead,
    q: &Checked,
    request: &AggregateRequest,
    specialized: bool,
    fast: bool,
    fused: bool,
    cap: usize,
) -> (Vec<BucketBits>, Option<String>) {
    let mut buckets = Vec::new();
    let mut emit = |mut bucket: Accumulator| {
        if buckets.len() == cap {
            return Err("metrics resource limit: output buckets".into());
        }
        bucket.close(&request.op);
        buckets.push(bits(bucket.result(&request.op, q.from)?));
        Ok(())
    };
    let result = if specialized {
        super::specialized::fold_read(read, q, request, &mut emit, fast, fused)
    } else {
        fold_reference(read, q, request, &mut emit, fused)
    };
    (buckets, result.err().map(|error| error.to_string()))
}

fn equivalent(read: &SeriesRead, q: &Checked, request: &AggregateRequest, cap: usize) {
    let expected = collect(read, q, request, false, false, false, cap);
    for (specialized, fast, fused) in [
        (false, false, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        assert_eq!(
            collect(read, q, request, specialized, fast, fused, cap),
            expected,
            "op={} kind={} width={} specialized={specialized} fast={fast} fused={fused}",
            request.op,
            read.series.kind,
            request.width
        );
    }
}

#[test]
fn kernels_preserve_all_ops_kinds_exact_rounding_and_bucket_boundaries() {
    let tiny = f64::from_bits(1);
    let gauge = points(
        -5,
        &[
            -0.0,
            0.0,
            tiny,
            -tiny,
            f64::MAX,
            -f64::MAX,
            3.0,
            1.0,
            -1.0,
            2.0,
        ],
    );
    let counter = points(
        -5,
        &[-0.0, 0.0, tiny, 4.0, 2.0, f64::MAX, 0.0, tiny, 3.0, 1.0],
    );
    for (kind, samples) in [
        ("gauge", &gauge),
        ("counter", &counter),
        ("untyped", &gauge),
    ] {
        let sealed = read(kind, &[(samples, false)], &[]);
        let mutable = read(kind, &[], samples);
        let mixed = read(
            kind,
            &[(&samples[..3], false), (&samples[3..7], false)],
            &samples[7..],
        );
        for q in [checked(-7, -5, 5), checked(-7, -2, 3), checked(-7, -7, 8)] {
            for width in [1, 2, 3, 7, 13, i64::MAX] {
                for op in OPS {
                    for candidate in [&sealed, &mutable, &mixed] {
                        equivalent(candidate, &q, &request(op, width), usize::MAX);
                        equivalent(candidate, &q, &request(op, width), 1);
                    }
                }
            }
        }
    }

    // The time span crosses the sign boundary and exceeds i64::MAX.
    let low = points(i64::MIN, &[1.0, -0.0, tiny]);
    let high = points(i64::MAX - 4, &[2.0, 3.0, 4.0]);
    let full = read("gauge", &[(&low, false), (&high, false)], &[]);
    for width in [1, 7, i64::MAX] {
        for op in OPS {
            equivalent(
                &full,
                &checked(i64::MIN, i64::MIN, i64::MAX),
                &request(op, width),
                usize::MAX,
            );
        }
    }
}

#[test]
fn kernels_preserve_mixed_summary_and_point_arithmetic() {
    let first = points(0, &[-0.0, 0.0, 1.0, f64::from_bits(1)]);
    let second = points(5, &[f64::MAX, -f64::MAX, 3.0]);
    let last = points(10, &[7.0, 9.0, 2.0]);
    for kind in ["gauge", "counter"] {
        let counter_second = points(5, &[f64::MAX, 0.0, 3.0]);
        let second = if kind == "counter" {
            &counter_second
        } else {
            &second
        };
        for summarized in [true, false] {
            let candidate = read(kind, &[(&first, summarized), (second, true)], &last);
            for op in OPS {
                equivalent(&candidate, &checked(0, 0, 15), &request(op, 5), usize::MAX);
            }
        }
    }
}

#[test]
fn grouped_kernels_preserve_signed_zero_resets_and_overflow() {
    for kind in ["gauge", "counter"] {
        let a = points(0, &[-0.0, 0.0, 3.0, 1.0, f64::MAX]);
        let b = points(0, &[0.0, -0.0, 4.0, 2.0, f64::MAX]);
        let members = [
            read(kind, &[(&a, false)], &[]),
            read(kind, &[(&b[..2], true), (&b[2..], false)], &[]),
        ];
        let q = checked(0, 0, 5);
        for op in OPS {
            let request = request(op, 5);
            let gather = |specialized: bool, fast: bool, fused: bool| {
                let mut grouped = Accumulator {
                    from: 0,
                    to: 5,
                    ..Default::default()
                };
                let status = (|| -> Result<()> {
                    for member in &members {
                        let mut emit = |mut bucket: Accumulator| {
                            bucket.close(op);
                            if specialized {
                                super::specialized::join(&mut grouped, bucket, op)
                            } else {
                                grouped.join(bucket)
                            }
                        };
                        if specialized {
                            super::specialized::fold_read(
                                member, &q, &request, &mut emit, fast, fused,
                            )?;
                        } else {
                            fold_reference(member, &q, &request, &mut emit, fused)?;
                        }
                    }
                    Ok(())
                })();
                match status {
                    Ok(()) => Ok(bits(grouped.result(op, q.from).unwrap())),
                    Err(error) => Err(error.to_string()),
                }
            };
            let expected = gather(false, false, false);
            for (specialized, fast, fused) in [
                (false, false, true),
                (true, false, false),
                (true, true, true),
            ] {
                assert_eq!(
                    gather(specialized, fast, fused),
                    expected,
                    "grouped op={op} kind={kind}"
                );
            }
        }
    }
}

#[test]
fn kernels_preserve_nonfinite_counter_and_corruption_error_precedence() {
    for kind in ["gauge", "counter"] {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            let samples = points(0, &[1.0, bad, 2.0]);
            let candidate = read(kind, &[(&samples, false)], &[]);
            for op in OPS {
                equivalent(&candidate, &checked(0, 0, 3), &request(op, 1), usize::MAX);
            }
        }
    }
    let samples = points(0, &[1.0, 2.0, 3.0]);
    let mut overlapping = read("gauge", &[(&samples, false), (&samples, false)], &[]);
    for op in OPS {
        equivalent(&overlapping, &checked(0, 0, 3), &request(op, 1), 1);
    }
    overlapping.blocks[1].summarized = true;
    for op in OPS {
        equivalent(&overlapping, &checked(0, 0, 3), &request(op, 1), usize::MAX);
    }

    let samples: Vec<_> = (0..241)
        .map(|at| Sample {
            at,
            value: if at == 0 { f64::NAN } else { at as f64 },
        })
        .collect();
    let mut corrupt_head = read("gauge", &[], &samples);
    corrupt_head.head[1].body = vec![255].into();
    for op in OPS {
        equivalent(&corrupt_head, &checked(0, 0, 241), &request(op, 1), 0);
        let result = collect(
            &corrupt_head,
            &checked(0, 0, 241),
            &request(op, 1),
            false,
            false,
            false,
            usize::MAX,
        );
        assert_ne!(result.1.as_deref(), Some("nonfinite metric value"));
    }
}

#[test]
fn fused_reads_preserve_output_limit_and_decode_error_precedence() {
    let samples = points(0, &[1.0, 2.0, 3.0]);
    let q = checked(0, 0, 3);
    for cap in [0, 1, 2, 3, 4] {
        let reference =
            decode_series_with(read("gauge", &[(&samples, false)], &[]), &q, cap, false);
        let fused = decode_series_with(read("gauge", &[(&samples, false)], &[]), &q, cap, true);
        assert_eq!(reference.error, fused.error);
        assert_eq!(reference.requires_slot, fused.requires_slot);
        assert_eq!(
            reference
                .value
                .samples
                .iter()
                .map(|p| (p.at, p.value.to_bits()))
                .collect::<Vec<_>>(),
            fused
                .value
                .samples
                .iter()
                .map(|p| (p.at, p.value.to_bits()))
                .collect::<Vec<_>>()
        );
    }
    let malformed = || {
        let mut candidate = read("gauge", &[(&samples, false)], &[]);
        candidate.blocks[0].block.body = vec![255];
        candidate
    };
    assert_eq!(
        decode_series_with(malformed(), &q, 0, false).error,
        decode_series_with(malformed(), &q, 0, true).error
    );
}
