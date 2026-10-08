//! Select the arithmetic kernel once per series, before decoding its points.
use super::{Accumulator, AggregateRequest, Checked, Result, Sample, SeriesRead, bucket_edges};
use crate::{codec, exact};
use num_traits::Zero;

const COUNT: u8 = 0;
const SUM: u8 = 1;
const MIN: u8 = 2;
const MAX: u8 = 3;
const DELTA: u8 = 4;
const INCREASE: u8 = 5;

pub(super) fn fold_read<F>(
    read: &SeriesRead,
    q: &Checked,
    request: &AggregateRequest,
    emit: &mut F,
    fast: bool,
    fused: bool,
) -> Result<()>
where
    F: FnMut(Accumulator) -> Result<()>,
{
    let counter = read.series.kind == "counter";
    if matches!(request.op.as_str(), "increase" | "rate") && !counter
        || request.op == "delta" && read.series.kind != "gauge"
    {
        return Err("invalid metrics request: aggregate kind".into());
    }
    if counter {
        dispatch::<true, F>(read, q, request, emit, fast, fused)
    } else {
        dispatch::<false, F>(read, q, request, emit, fast, fused)
    }
}

fn dispatch<const COUNTER: bool, F>(
    read: &SeriesRead,
    q: &Checked,
    request: &AggregateRequest,
    emit: &mut F,
    fast: bool,
    fused: bool,
) -> Result<()>
where
    F: FnMut(Accumulator) -> Result<()>,
{
    match request.op.as_str() {
        "count" => fold::<COUNT, COUNTER, false, F>(read, q, request.width, emit, fused),
        "sum" | "avg" if fast => fold::<SUM, COUNTER, true, F>(read, q, request.width, emit, fused),
        "sum" | "avg" => fold::<SUM, COUNTER, false, F>(read, q, request.width, emit, fused),
        "min" => fold::<MIN, COUNTER, false, F>(read, q, request.width, emit, fused),
        "max" => fold::<MAX, COUNTER, false, F>(read, q, request.width, emit, fused),
        "delta" => fold::<DELTA, COUNTER, false, F>(read, q, request.width, emit, fused),
        "increase" | "rate" => {
            fold::<INCREASE, COUNTER, false, F>(read, q, request.width, emit, fused)
        }
        _ => Err("invalid metrics request: aggregate operation".into()),
    }
}

#[inline]
fn add<const OP: u8, const COUNTER: bool, const FAST: bool>(
    current: &mut Accumulator,
    value: f64,
) -> Result<()> {
    // Keep counter validation before ordinary finite validation, including -0.
    if COUNTER && (!value.is_finite() || value < 0.0) {
        return Err("invalid counter aggregate input".into());
    }
    match OP {
        COUNT | MIN | MAX => {
            if !value.is_finite() {
                return Err("nonfinite metric value".into());
            }
            if OP == MIN {
                current.min = if current.count == 0 {
                    value
                } else {
                    exact::minimum(current.min, value)
                };
            } else if OP == MAX {
                current.max = if current.count == 0 {
                    value
                } else {
                    exact::maximum(current.max, value)
                };
            }
        }
        SUM if FAST => {
            current
                .fast
                .get_or_insert_with(|| Box::new(exact::ExactSum::default()))
                .add_value(value)?;
        }
        SUM => current.exact += exact::finite_units(value)?,
        DELTA => {
            let units = exact::finite_units(value)?;
            if current.count == 0 {
                current.first = units.clone();
            }
            current.previous = units;
        }
        INCREASE => {
            let units = exact::finite_units(value)?;
            if current.count > 0 {
                if value < current.previous_value {
                    current.exact += &units;
                    current.resets += 1;
                } else {
                    current.exact += &units - &current.previous;
                }
            }
            current.previous = units;
            current.previous_value = value;
        }
        _ => unreachable!(),
    }
    current.count += 1;
    Ok(())
}

fn merge_summary<const OP: u8>(current: &mut Accumulator, block: &codec::Block) -> Result<()> {
    let summary = &block.summary;
    match OP {
        MIN => {
            current.min = if current.count == 0 {
                summary.min
            } else {
                exact::minimum(current.min, summary.min)
            };
        }
        MAX => {
            current.max = if current.count == 0 {
                summary.max
            } else {
                exact::maximum(current.max, summary.max)
            };
        }
        SUM => current.exact += codec::read_exact_value(&summary.exact_sum)?,
        DELTA => {
            if current.count == 0 {
                current.first = exact::finite_units(block.head.first)?;
            }
            current.previous = exact::finite_units(summary.last)?;
        }
        INCREASE => {
            let first = exact::finite_units(block.head.first)?;
            if current.count > 0 {
                if block.head.first < current.previous_value {
                    current.exact += &first;
                    current.resets += 1;
                } else {
                    current.exact += &first - &current.previous;
                }
            }
            let increase = codec::read_exact_value(&summary.exact_increase)?;
            if increase < num_bigint::BigInt::zero() {
                return Err("corrupt metrics data: negative exact increase".into());
            }
            current.exact += increase;
            current.resets += summary.resets as usize;
            current.previous_value = summary.last;
            current.previous = exact::finite_units(summary.last)?;
        }
        COUNT => {}
        _ => unreachable!(),
    }
    current.count = current
        .count
        .checked_add(block.head.count)
        .ok_or("metrics resource limit: aggregate summary count")?;
    Ok(())
}

fn fold<const OP: u8, const COUNTER: bool, const FAST: bool, F>(
    read: &SeriesRead,
    q: &Checked,
    width: i64,
    emit: &mut F,
    fused: bool,
) -> Result<()>
where
    F: FnMut(Accumulator) -> Result<()>,
{
    let mut current = Accumulator::default();
    let mut previous_at = None;
    for selected in &read.blocks {
        if selected.summarized {
            let block = &selected.block;
            if COUNTER && block.summary.min < 0.0 {
                return Err("invalid counter aggregate input".into());
            }
            if previous_at.is_some_and(|at| block.head.start <= at) {
                return Err("corrupt metrics data: overlapping summary samples".into());
            }
            let (start, end) = bucket_edges(q.origin, q.to, block.head.start, width);
            if current.count > 0 && start != current.from {
                emit(std::mem::take(&mut current))?;
            }
            current.from = start;
            current.to = end;
            merge_summary::<OP>(&mut current, block)?;
            previous_at = Some(block.head.end);
        } else if fused {
            codec::visit_block(&selected.block, |point| {
                sample::<OP, COUNTER, FAST, F>(
                    point,
                    q,
                    width,
                    &mut previous_at,
                    &mut current,
                    emit,
                )
                .map_err(|error| error.to_string())
            })?;
        } else {
            for point in codec::decode_block(&selected.block)? {
                sample::<OP, COUNTER, FAST, F>(
                    point,
                    q,
                    width,
                    &mut previous_at,
                    &mut current,
                    emit,
                )?;
            }
        }
    }
    if fused {
        codec::visit_chunks(&read.head, q.from, q.to, |point| {
            sample::<OP, COUNTER, FAST, F>(point, q, width, &mut previous_at, &mut current, emit)
                .map_err(|error| error.to_string())
        })?;
    } else {
        for point in codec::decode_chunks(&read.head, q.from, q.to)? {
            sample::<OP, COUNTER, FAST, F>(point, q, width, &mut previous_at, &mut current, emit)?;
        }
    }
    if current.count > 0 {
        emit(current)?;
    }
    Ok(())
}

#[inline]
fn sample<const OP: u8, const COUNTER: bool, const FAST: bool, F>(
    point: Sample,
    q: &Checked,
    width: i64,
    previous_at: &mut Option<i64>,
    current: &mut Accumulator,
    emit: &mut F,
) -> Result<()>
where
    F: FnMut(Accumulator) -> Result<()>,
{
    if point.at < q.from || point.at >= q.to {
        return Ok(());
    }
    if previous_at.is_some_and(|at| point.at <= at) {
        return Err("corrupt metrics data: overlapping samples".into());
    }
    *previous_at = Some(point.at);
    let (start, end) = bucket_edges(q.origin, q.to, point.at, width);
    if current.count > 0 && start != current.from {
        emit(std::mem::take(current))?;
    }
    current.from = start;
    current.to = end;
    add::<OP, COUNTER, FAST>(current, point.value)
}

pub(super) fn join(current: &mut Accumulator, other: Accumulator, op: &str) -> Result<()> {
    match op {
        "min" => {
            current.min = if current.count == 0 {
                other.min
            } else {
                exact::minimum(current.min, other.min)
            }
        }
        "max" => {
            current.max = if current.count == 0 {
                other.max
            } else {
                exact::maximum(current.max, other.max)
            }
        }
        _ => {}
    }
    current.count = current
        .count
        .checked_add(other.count)
        .ok_or("metrics resource limit: aggregate group count")?;
    if matches!(op, "sum" | "avg" | "delta" | "increase" | "rate") {
        current.resets += other.resets;
        current.exact += other.exact;
        if let Some(other_fast) = other.fast {
            current
                .fast
                .get_or_insert_with(|| Box::new(exact::ExactSum::default()))
                .merge(&other_fast)?;
        }
    }
    Ok(())
}
