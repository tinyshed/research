//! Native port of metrics matching, bounded snapshot fetch, reads and exact aggregates.
#[cfg(test)]
mod adapter_tests;
#[cfg(test)]
mod optimization_tests;
mod parallel;
mod quote;
mod specialized;

use crate::codec::{self, Block, HeadChunk};
use crate::engine::{Engine, Result, Sample, Series, decode_label_ids};
use crate::exact;
use num_bigint::BigInt;
use num_traits::Zero;
use rayon::prelude::*;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Configure the global pool before timing or using query methods. Zero stays serial.
pub fn configure_parallel(threads: usize, force: bool) -> Result<()> {
    parallel::configure(threads, force)
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub series: usize,
    pub blocks: usize,
    pub payload_bytes: usize,
    pub decoded_samples: usize,
    pub output_samples: usize,
}
impl Limits {
    pub fn zero() -> Self {
        Self {
            series: 0,
            blocks: 0,
            payload_bytes: 0,
            decoded_samples: 0,
            output_samples: 0,
        }
    }
    pub fn defaults() -> Self {
        Self {
            series: 1000,
            blocks: 4096,
            payload_bytes: 16 << 20,
            decoded_samples: 1 << 20,
            output_samples: 100000,
        }
    }
    fn narrow(&self, configured: &Self) -> Result<Self> {
        fn field(n: usize, bound: usize) -> Result<usize> {
            Ok(if n == 0 || n > bound { bound } else { n })
        }
        Ok(Self {
            series: field(self.series, configured.series)?,
            blocks: field(self.blocks, configured.blocks)?,
            payload_bytes: field(self.payload_bytes, configured.payload_bytes)?,
            decoded_samples: field(self.decoded_samples, configured.decoded_samples)?,
            output_samples: field(self.output_samples, configured.output_samples)?,
        })
    }
}
impl Default for Limits {
    fn default() -> Self {
        Self::defaults()
    }
}

#[derive(Clone, Debug)]
pub enum Condition {
    OneOf(Vec<String>),
    NoneOf(Vec<String>),
    Prefix(String),
}

#[derive(Clone, Debug)]
pub struct Query {
    pub name: String,
    pub match_labels: BTreeMap<String, String>,
    pub conditions: BTreeMap<String, Condition>,
    pub from: i64,
    pub to: i64,
    pub since: i64,
    pub limits: Limits,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            name: String::new(),
            match_labels: BTreeMap::new(),
            conditions: BTreeMap::new(),
            from: 0,
            to: 0,
            since: 0,
            limits: Limits::zero(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReadResult {
    pub series: Series,
    pub samples: Vec<Sample>,
}

#[derive(Clone, Debug)]
pub struct AggregateRequest {
    pub query: Query,
    pub width: i64,
    pub op: String,
    pub by: Option<Vec<String>>,
    pub without: Option<Vec<String>>,
}
#[derive(Clone, Debug)]
pub struct AggregateBucket {
    pub from: i64,
    pub to: i64,
    pub count: usize,
    pub resets: usize,
    pub value: f64,
    pub overflow: bool,
    pub partial: bool,
}
#[derive(Clone, Debug)]
pub struct AggregateResult {
    pub series: Series,
    pub buckets: Vec<AggregateBucket>,
}

#[derive(Clone, Debug, Default)]
pub struct Spending {
    pub bytes: usize,
    pub decoded: usize,
    pub blocks: usize,
    pub groups: usize,
    pub series: usize,
    pub summarized: usize,
}
struct Budget {
    limits: Limits,
    spent: Spending,
    clocks: HashMap<i64, Vec<Block>>,
    order: VecDeque<i64>,
}
impl Budget {
    fn bytes(&mut self, n: usize) -> Result<()> {
        if n > self.limits.payload_bytes.saturating_sub(self.spent.bytes) {
            return Err("metrics resource limit: fetched bytes".into());
        }
        self.spent.bytes += n;
        Ok(())
    }
    fn samples(&mut self, n: usize) -> Result<()> {
        if n > self
            .limits
            .decoded_samples
            .saturating_sub(self.spent.decoded)
        {
            return Err("metrics resource limit: decoded samples".into());
        }
        self.spent.decoded += n;
        Ok(())
    }
    fn group(&mut self) -> Result<()> {
        self.spent.groups += 1;
        if self.spent.groups > self.limits.blocks {
            return Err("metrics resource limit: group descriptors".into());
        }
        Ok(())
    }
    fn block(&mut self) -> Result<()> {
        self.spent.blocks += 1;
        if self.spent.blocks > self.limits.blocks {
            return Err("metrics resource limit: decoded blocks".into());
        }
        Ok(())
    }
}
struct Checked {
    origin: i64,
    from: i64,
    to: i64,
    limits: Limits,
    matchers: BTreeMap<String, String>,
    conditions: BTreeMap<String, Condition>,
}
struct Registered {
    id: i64,
    series: Series,
    ids: Vec<i64>,
}
struct SelectedBlock {
    block: Block,
    summarized: bool,
}
struct SeriesRead {
    series: Series,
    blocks: Vec<SelectedBlock>,
    head: Vec<HeadChunk>,
}
struct GroupRow {
    id: i64,
    start: i64,
    end: i64,
    clock_id: i64,
    size: usize,
    data: Vec<u8>,
}

fn placeholders(n: usize) -> String {
    std::iter::repeat_n("?", n).collect::<Vec<_>>().join(",")
}
fn query_row_cached<T, P, F>(
    conn: &Connection,
    sql: &str,
    arguments: P,
    read: F,
) -> rusqlite::Result<T>
where
    P: rusqlite::Params,
    F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    conn.prepare_cached(sql)?.query_row(arguments, read)
}
fn public_series(pairs: Vec<(String, String)>, kind: String) -> Result<Series> {
    let mut labels = BTreeMap::new();
    let mut name = String::new();
    if pairs.len() > 128 || pairs.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() > 16 << 10 {
        return Err("corrupt metrics data: registry label budget".into());
    }
    for (k, v) in pairs {
        if k.is_empty() || k.len() > 256 || v.len() > 4096 {
            return Err("corrupt metrics data: registry labels".into());
        }
        if k == "__name__" {
            if !name.is_empty() {
                return Err("corrupt metrics data: repeated registry name".into());
            }
            name = v;
        } else if k.starts_with("__") || labels.insert(k, v).is_some() {
            return Err("corrupt metrics data: registry labels".into());
        }
    }
    if name.is_empty() {
        return Err("corrupt metrics data: registry name".into());
    }
    Ok(Series { name, kind, labels })
}

impl Engine {
    fn check_query(&self, request: &Query) -> Result<Checked> {
        if request.since < 0 || (request.since != 0 && request.from != 0) {
            return Err("invalid metrics request: range origin".into());
        }
        let origin = if request.since == 0 {
            request.from
        } else {
            self.now.saturating_sub(request.since)
        };
        let to = if request.to == 0 {
            i64::MAX
        } else {
            request.to
        };
        if to < origin {
            return Err("invalid metrics request: inverted range".into());
        }
        let mut matchers = request.match_labels.clone();
        if !request.name.is_empty() {
            matchers.insert("__name__".into(), request.name.clone());
        }
        if request
            .match_labels
            .keys()
            .any(|k| k.is_empty() || k.starts_with("__") || k.len() > 256)
            || matchers.values().any(|v| v.len() > 4096)
            || matchers.len() > 128
            || matchers
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
                > 16 << 10
        {
            return Err("invalid metrics request: labels".into());
        }
        let mut finds = !matchers.is_empty();
        let mut conditions = request.conditions.clone();
        for (name, condition) in &mut conditions {
            if name.is_empty()
                || name.starts_with("__")
                || name.len() > 256
                || matchers.contains_key(name)
            {
                return Err("invalid metrics request: condition label".into());
            }
            match condition {
                Condition::OneOf(values) | Condition::NoneOf(values) => {
                    if values.is_empty()
                        || values.len() > 1000
                        || values.iter().any(|v| v.len() > 4096)
                    {
                        return Err("invalid metrics request: condition values".into());
                    }
                    values.sort();
                    values.dedup();
                }
                Condition::Prefix(value) => {
                    if value.is_empty() || value.len() > 4096 {
                        return Err("invalid metrics request: condition prefix".into());
                    }
                }
            }
            if !matches!(condition, Condition::NoneOf(_)) {
                finds = true;
            }
        }
        if !finds {
            return Err("invalid metrics request: range needs a finding selector".into());
        }
        Ok(Checked {
            origin,
            from: origin.max(self.now.saturating_sub(self.options.retention)),
            to,
            limits: request.limits.narrow(&self.options.limits)?,
            matchers,
            conditions,
        })
    }

    pub fn read(&self, request: &Query) -> Result<Vec<ReadResult>> {
        let mut out = Vec::new();
        self.stream(request, |result| {
            out.push(result);
            Ok(())
        })?;
        Ok(out)
    }

    pub fn stream<F>(&self, request: &Query, mut yield_result: F) -> Result<()>
    where
        F: FnMut(ReadResult) -> Result<()>,
    {
        let _profile = crate::profile::scope("read_total");
        let q = self.check_query(request)?;
        if q.from >= q.to {
            return Ok(());
        }
        let (reads, _) = self.fetch_snapshot(&q, None)?;
        let _process = crate::profile::scope("read_process");
        stream_reads(reads, &q, &mut yield_result)?;
        Ok(())
    }

    /// A diagnostic for matching production resource-error and shortcut paths.
    pub fn query_spending(&self, request: &Query, width: Option<i64>) -> Result<Spending> {
        let q = self.check_query(request)?;
        if q.from >= q.to {
            return Ok(Spending::default());
        }
        Ok(self.fetch_snapshot(&q, width)?.1)
    }

    fn fetch_snapshot(
        &self,
        q: &Checked,
        width: Option<i64>,
    ) -> Result<(Vec<SeriesRead>, Spending)> {
        let _profile = crate::profile::scope("snapshot");
        let deadline =
            Instant::now() + Duration::from_millis(self.options.snapshot_timeout_ms.max(1) as u64);
        self.reader
            .progress_handler(1000, Some(move || Instant::now() >= deadline))?;
        let result = (|| {
            self.reader.execute_batch("BEGIN DEFERRED")?;
            let out = self.fetch_inside(q, width)?;
            self.reader.execute_batch("COMMIT")?;
            Ok(out)
        })();
        // Remove interruption before rollback so an expired deadline cannot
        // prevent returning the connection to autocommit. Preserve the
        // original query failure if cleanup also fails.
        let cleanup = self.reader.progress_handler(0, None::<fn() -> bool>);
        let rollback = if self.reader.is_autocommit() {
            Ok(())
        } else {
            self.reader.execute_batch("ROLLBACK")
        };
        match result {
            Err(err) => Err(err),
            Ok(out) => {
                cleanup?;
                rollback?;
                Ok(out)
            }
        }
    }

    fn fetch_inside(&self, q: &Checked, width: Option<i64>) -> Result<(Vec<SeriesRead>, Spending)> {
        let mut budget = Budget {
            limits: q.limits.clone(),
            spent: Spending::default(),
            clocks: HashMap::new(),
            order: VecDeque::new(),
        };
        let matched = match_series(&self.reader, q, &mut budget)?;
        let batched = matched.len() >= 16;
        let mut reads: Vec<SeriesRead> = matched
            .iter()
            .map(|r| SeriesRead {
                series: r.series.clone(),
                blocks: Vec::new(),
                head: Vec::new(),
            })
            .collect();
        if batched {
            for start in (0..matched.len()).step_by(64) {
                let end = (start + 64).min(matched.len());
                self.fetch_heads(q, &matched[start..end], &mut reads[start..end], &mut budget)?;
            }
            let groups = fetch_group_rows(&self.reader, q, &matched, &mut budget)?;
            for (i, series) in matched.iter().enumerate() {
                for row in groups.get(&series.id).into_iter().flatten() {
                    self.select_group(q, width, row, &mut reads[i], &mut budget, true)?;
                }
            }
            fetch_payloads(&self.reader, &mut reads)?;
        } else {
            for (i, series) in matched.iter().enumerate() {
                let rows = fetch_each_groups(&self.reader, q, series.id, &mut budget)?;
                for row in &rows {
                    self.select_group(q, width, row, &mut reads[i], &mut budget, false)?;
                }
                self.fetch_one_head(q, series.id, &mut reads[i], &mut budget)?;
            }
        }
        Ok((reads, budget.spent))
    }

    fn select_group(
        &self,
        q: &Checked,
        width: Option<i64>,
        row: &GroupRow,
        read: &mut SeriesRead,
        budget: &mut Budget,
        defer: bool,
    ) -> Result<()> {
        let _profile = crate::profile::scope("select_group");
        let clock = if let Some(clock) = budget.clocks.get(&row.clock_id) {
            clock.clone()
        } else {
            let maximum = 65536.min(
                budget
                    .limits
                    .payload_bytes
                    .saturating_sub(budget.spent.bytes),
            );
            let (length, data): (i64, Option<Vec<u8>>) = query_row_cached(
                &self.reader,
                "SELECT length(body),CASE WHEN length(body)<=? THEN body ELSE NULL END FROM clocks WHERE id=?",
                params![maximum as i64, row.clock_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if !(0..=65536).contains(&length) {
                return Err("corrupt metrics data: clock size".into());
            }
            budget.bytes(length as usize)?;
            let data = data.ok_or("corrupt metrics data: shared clock missing")?;
            if data.len() != length as usize {
                return Err("corrupt metrics data: clock length".into());
            }
            let blocks = codec::decode_clock_group(&data)?;
            if budget.clocks.len() == 8 {
                if let Some(old) = budget.order.pop_front() {
                    budget.clocks.remove(&old);
                }
            }
            budget.order.push_back(row.clock_id);
            budget.clocks.insert(row.clock_id, blocks.clone());
            blocks
        };
        let group =
            codec::read_directory(row.id, row.start, row.end, row.clock_id, &row.data, &clock)?;
        for (slot, mut block) in group.blocks.into_iter().enumerate() {
            if group.live & (1_u32 << slot) == 0
                || block.head.end < q.from
                || block.head.start >= q.to
            {
                continue;
            }
            budget.block()?;
            let summarized = width.is_some_and(|width| complete_summary(&block, q, width));
            if summarized {
                budget.spent.summarized += 1;
            } else {
                budget.samples(block.head.count)?;
                if block.payload != 0 {
                    budget.bytes(block.body_bytes)?;
                    if !defer {
                        let (length, body): (i64, Option<Vec<u8>>) = query_row_cached(
                            &self.reader,
                            "SELECT length(body),CASE WHEN length(body)=? THEN body ELSE NULL END FROM payloads WHERE id=?",
                            params![block.body_bytes as i64, block.payload],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )?;
                        block.body = body.ok_or("corrupt metrics data: payload missing or size")?;
                        if length < 0 || block.body.len() != block.body_bytes {
                            return Err("corrupt metrics data: payload size".into());
                        }
                    }
                }
            }
            read.blocks.push(SelectedBlock { block, summarized });
        }
        Ok(())
    }

    fn parse_selected_head(
        &self,
        q: &Checked,
        id: i64,
        count: i64,
        start: Option<i64>,
        end: Option<i64>,
        size: i64,
        packed: Option<Vec<u8>>,
        read: &mut SeriesRead,
        budget: &mut Budget,
    ) -> Result<()> {
        if count == 0 {
            if start.is_some() || end.is_some() || size != 0 {
                return Err("corrupt metrics data: empty mutable head".into());
            }
            return Ok(());
        }
        let start = start.ok_or("corrupt metrics data: mutable start")?;
        let end = end.ok_or("corrupt metrics data: mutable end")?;
        if end < start {
            return Err("corrupt metrics data: mutable endpoints".into());
        }
        if end < q.from || start >= q.to {
            return Ok(());
        }
        if count < 0
            || count as usize > self.options.max_head_samples
            || size < 0
            || size as usize > self.options.max_head_bytes
        {
            return Err("metrics resource limit: mutable head capacity".into());
        }
        if size == 0 {
            return Err("corrupt metrics data: mutable body missing".into());
        }
        budget.bytes(size as usize)?;
        let packed = packed.ok_or("corrupt metrics data: mutable body missing")?;
        if packed.len() != size as usize {
            return Err("corrupt metrics data: mutable body size".into());
        }
        read.head = codec::parse_head(
            id,
            count as usize,
            start,
            end,
            &packed,
            self.options.max_head_samples,
            self.options.max_head_bytes,
        )?;
        budget.samples(
            read.head
                .iter()
                .filter(|c| c.head.end >= q.from && c.head.start < q.to)
                .map(|c| c.head.count)
                .sum(),
        )?;
        Ok(())
    }

    fn fetch_one_head(
        &self,
        q: &Checked,
        id: i64,
        read: &mut SeriesRead,
        budget: &mut Budget,
    ) -> Result<()> {
        let _profile = crate::profile::scope("fetch_heads");
        let maximum = self.options.max_head_bytes.min(
            budget
                .limits
                .payload_bytes
                .saturating_sub(budget.spent.bytes),
        );
        let (count, start, end, size, packed): (
            i64,
            Option<i64>,
            Option<i64>,
            i64,
            Option<Vec<u8>>,
        ) = query_row_cached(
            &self.reader,
            "SELECT head_count,head_start,head_end,coalesce(length(tail),0),CASE WHEN length(tail)<=? AND head_end>=? AND head_start<? THEN tail END FROM series_state WHERE series_id=?",
            params![maximum as i64, q.from, q.to, id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        self.parse_selected_head(q, id, count, start, end, size, packed, read, budget)
    }

    fn fetch_heads(
        &self,
        q: &Checked,
        matched: &[Registered],
        reads: &mut [SeriesRead],
        budget: &mut Budget,
    ) -> Result<()> {
        let _profile = crate::profile::scope("fetch_heads");
        let ids: Vec<i64> = matched.iter().map(|r| r.id).collect();
        let encoded = serde_json::to_string(&ids)?;
        let mut descriptors = HashMap::new();
        let mut statement=self.reader.prepare_cached("SELECT state.series_id,state.head_count,state.head_start,state.head_end,coalesce(length(state.tail),0) FROM json_each(?) ids JOIN series_state state ON state.series_id=cast(ids.value as integer)")?;
        let mut rows = statement.query([&encoded])?;
        let mut selected = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let count: i64 = row.get(1)?;
            let start: Option<i64> = row.get(2)?;
            let end: Option<i64> = row.get(3)?;
            let size: i64 = row.get(4)?;
            if descriptors.insert(id, (count, start, end, size)).is_some() {
                return Err("corrupt metrics data: duplicate head descriptor".into());
            }
            if count != 0 && end.is_some_and(|v| v >= q.from) && start.is_some_and(|v| v < q.to) {
                if count < 0
                    || count as usize > self.options.max_head_samples
                    || size < 0
                    || size as usize > self.options.max_head_bytes
                {
                    return Err("metrics resource limit: mutable head capacity".into());
                }
                if size == 0 {
                    return Err("corrupt metrics data: mutable body missing".into());
                }
                selected.push(id);
            }
        }
        drop(rows);
        drop(statement);
        if descriptors.len() != ids.len() {
            return Err("corrupt metrics data: mutable descriptor missing".into());
        }
        let mut bodies = HashMap::new();
        // Descriptor accounting precedes any body fetch.
        let selected_size: usize = selected
            .iter()
            .map(|id| descriptors[id].3.max(0) as usize)
            .sum();
        if selected_size
            > budget
                .limits
                .payload_bytes
                .saturating_sub(budget.spent.bytes)
        {
            return Err("metrics resource limit: fetched bytes".into());
        }
        if !selected.is_empty() {
            let json = serde_json::to_string(&selected)?;
            let mut statement=self.reader.prepare_cached("SELECT state.series_id,state.tail FROM json_each(?) ids JOIN series_state state ON state.series_id=cast(ids.value as integer)")?;
            let mut rows = statement.query([json])?;
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                if bodies.insert(id, data).is_some() {
                    return Err("corrupt metrics data: mutable body repeats".into());
                }
            }
        }
        for (registered, read) in matched.iter().zip(reads) {
            let (count, start, end, size) = descriptors
                .remove(&registered.id)
                .ok_or("corrupt metrics data: mutable descriptor missing")?;
            self.parse_selected_head(
                q,
                registered.id,
                count,
                start,
                end,
                size,
                bodies.remove(&registered.id),
                read,
                budget,
            )?;
        }
        Ok(())
    }
}

struct Outcome<T> {
    value: T,
    error: Option<String>,
    requires_slot: bool,
}
fn parallel_reads(reads: &[SeriesRead]) -> bool {
    if !parallel::active() {
        return false;
    }
    let samples = reads
        .iter()
        .map(|read| {
            read.blocks
                .iter()
                .filter(|b| !b.summarized)
                .map(|b| b.block.head.count)
                .sum::<usize>()
                + read.head.iter().map(|h| h.head.count).sum::<usize>()
        })
        .sum();
    let summaries = reads
        .iter()
        .map(|read| read.blocks.iter().filter(|b| b.summarized).count())
        .sum();
    parallel::enabled(reads.len(), samples, summaries)
}
fn decode_series(read: SeriesRead, q: &Checked, cap: usize) -> Outcome<ReadResult> {
    decode_series_with(read, q, cap, crate::tuning::fused())
}
fn decode_series_with(
    read: SeriesRead,
    q: &Checked,
    cap: usize,
    fused: bool,
) -> Outcome<ReadResult> {
    let reserve = if codec::optimized() {
        let upper = read
            .blocks
            .iter()
            .map(|b| b.block.head.count)
            .sum::<usize>()
            + read.head.iter().map(|h| h.head.count).sum::<usize>();
        upper
            .min(cap)
            .min((q.to as u64).wrapping_sub(q.from as u64) as usize)
    } else {
        0
    };
    let mut result = ReadResult {
        series: read.series,
        samples: Vec::with_capacity(reserve),
    };
    let mut requires_slot = false;
    let status = (|| -> Result<()> {
        let mut visit = |point: Sample| -> Result<()> {
            if point.at < q.from || point.at >= q.to {
                return Ok(());
            }
            requires_slot = true;
            if result.samples.len() == cap {
                return Err("metrics resource limit: output samples".into());
            }
            if result
                .samples
                .last()
                .is_some_and(|previous| point.at <= previous.at)
            {
                return Err("corrupt metrics data: overlapping samples".into());
            }
            result.samples.push(point);
            requires_slot = false;
            Ok(())
        };
        for selected in read.blocks {
            if fused {
                codec::visit_block(&selected.block, |point| {
                    visit(point).map_err(|error| error.to_string())
                })?;
            } else {
                for point in codec::decode_block(&selected.block)? {
                    visit(point)?;
                }
            }
        }
        for chunk in &read.head {
            if fused {
                codec::visit_chunks(std::slice::from_ref(chunk), q.from, q.to, |point| {
                    visit(point).map_err(|error| error.to_string())
                })?;
            } else {
                for point in codec::decode_chunks(std::slice::from_ref(chunk), q.from, q.to)? {
                    visit(point)?;
                }
            }
        }
        Ok(())
    })();
    Outcome {
        value: result,
        error: status.err().map(|error| error.to_string()),
        requires_slot,
    }
}
fn consume_read<F>(
    outcome: Outcome<ReadResult>,
    q: &Checked,
    output: &mut usize,
    yield_result: &mut F,
) -> Result<()>
where
    F: FnMut(ReadResult) -> Result<()>,
{
    let remaining = q.limits.output_samples.saturating_sub(*output);
    if outcome.value.samples.len() > remaining
        || (outcome.requires_slot && outcome.value.samples.len() >= remaining)
    {
        return Err("metrics resource limit: output samples".into());
    }
    if let Some(error) = outcome.error {
        return Err(error.into());
    }
    *output += outcome.value.samples.len();
    if !outcome.value.samples.is_empty() {
        yield_result(outcome.value)?;
    }
    Ok(())
}
fn stream_reads<F>(reads: Vec<SeriesRead>, q: &Checked, yield_result: &mut F) -> Result<()>
where
    F: FnMut(ReadResult) -> Result<()>,
{
    let mut output = 0;
    if parallel_reads(&reads) {
        let mut pending = reads.into_iter();
        loop {
            let batch: Vec<_> = pending.by_ref().take(parallel::batch_size()).collect();
            if batch.is_empty() {
                break;
            }
            let cap = q.limits.output_samples.saturating_sub(output);
            // Indexed parallel collection preserves registry order. The closure
            // receives owned snapshot data and never a SQLite connection.
            let outcomes: Vec<_> = batch
                .into_par_iter()
                .map(|read| decode_series(read, q, cap))
                .collect();
            for outcome in outcomes {
                consume_read(outcome, q, &mut output, yield_result)?;
            }
        }
    } else {
        for read in reads {
            let cap = q.limits.output_samples.saturating_sub(output);
            consume_read(decode_series(read, q, cap), q, &mut output, yield_result)?;
        }
    }
    Ok(())
}

fn complete_summary(block: &Block, q: &Checked, width: i64) -> bool {
    if block.summary.exact_sum.is_empty() || block.head.start < q.from || block.head.end >= q.to {
        return false;
    }
    let (start, end) = bucket_edges(q.origin, q.to, block.head.start, width);
    block.head.start >= start && block.head.end < end
}

struct PostingSet {
    ids: Vec<i64>,
    count: i64,
    prefix: Option<(String, String)>,
}
impl PostingSet {
    fn membership(&self, column: &str) -> (String, Vec<Value>) {
        if let Some((name, prefix)) = &self.prefix {
            // Go uses the next byte as its upper bound. UTF-8 labels never end in 0xff.
            let mut end = prefix.as_bytes().to_vec();
            let last = end.last_mut().unwrap();
            *last += 1;
            return (
                format!(
                    "{column} IN (SELECT id FROM label_values WHERE name=? AND value>=? AND value<CAST(? AS TEXT))"
                ),
                vec![
                    Value::Text(name.clone()),
                    Value::Text(prefix.clone()),
                    Value::Blob(end),
                ],
            );
        }
        (
            format!("{column} IN ({})", placeholders(self.ids.len())),
            self.ids.iter().map(|id| Value::Integer(*id)).collect(),
        )
    }
}

fn match_series(conn: &Connection, q: &Checked, budget: &mut Budget) -> Result<Vec<Registered>> {
    let _profile = crate::profile::scope("match_registry");
    let mut finding = Vec::new();
    let mut leaving = Vec::new();
    if q.conditions.is_empty() && q.matchers.len() == 1 {
        let (name, value) = q.matchers.iter().next().unwrap();
        let id: Option<i64> = query_row_cached(
            conn,
            "SELECT id FROM label_values WHERE name=? AND value=?",
            params![name, value],
            |r| r.get(0),
        )
        .optional()?;
        let Some(id) = id else { return Ok(Vec::new()) };
        finding.push(PostingSet {
            ids: vec![id],
            count: 1,
            prefix: None,
        });
    } else if q.conditions.is_empty() && q.matchers.len() >= 8 {
        let pairs: Vec<[&str; 2]> = q
            .matchers
            .iter()
            .map(|(name, value)| [name.as_str(), value.as_str()])
            .collect();
        let json = serde_json::to_string(&pairs)?;
        let mut statement=conn.prepare_cached("SELECT v.id,v.posting_count FROM json_each(?) j LEFT JOIN label_values v ON v.name=json_extract(j.value,'$[0]') AND v.value=json_extract(j.value,'$[1]') ORDER BY cast(j.key AS INTEGER)")?;
        let mut rows = statement.query([json])?;
        while let Some(row) = rows.next()? {
            let id: Option<i64> = row.get(0)?;
            let count: Option<i64> = row.get(1)?;
            let (Some(id), Some(count)) = (id, count) else {
                return Ok(Vec::new());
            };
            if count == 0 {
                return Ok(Vec::new());
            }
            if count < 0 {
                return Err("corrupt metrics data: posting count".into());
            }
            finding.push(PostingSet {
                ids: vec![id],
                count,
                prefix: None,
            });
        }
        if finding.len() != q.matchers.len() {
            return Err("corrupt metrics data: matcher resolution count".into());
        }
    } else {
        for (name, value) in &q.matchers {
            let posting: Option<(i64, i64)> = query_row_cached(
                conn,
                "SELECT id,posting_count FROM label_values WHERE name=? AND value=?",
                params![name, value],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
            let Some((id, count)) = posting else {
                return Ok(Vec::new());
            };
            if count == 0 {
                return Ok(Vec::new());
            }
            if count < 0 {
                return Err("corrupt metrics data: posting count".into());
            }
            finding.push(PostingSet {
                ids: vec![id],
                count,
                prefix: None,
            });
        }
    }
    for (name, condition) in &q.conditions {
        let set = match condition {
            Condition::Prefix(prefix) => {
                let mut end = prefix.as_bytes().to_vec();
                *end.last_mut().unwrap() += 1;
                let count: i64 = query_row_cached(
                    conn,
                    "SELECT coalesce(sum(posting_count),0) FROM label_values WHERE name=? AND value>=? AND value<CAST(? AS TEXT)",
                    params![name, prefix, end],
                    |r| r.get(0),
                )?;
                PostingSet {
                    ids: Vec::new(),
                    count,
                    prefix: Some((name.clone(), prefix.clone())),
                }
            }
            Condition::OneOf(values) | Condition::NoneOf(values) => {
                let json = serde_json::to_string(values)?;
                let mut statement=conn.prepare_cached("SELECT v.id,v.posting_count FROM json_each(?) j JOIN label_values v ON v.name=? AND v.value=j.value")?;
                let pairs = statement.query_map(params![json, name], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                })?;
                let mut ids = Vec::new();
                let mut count = 0;
                for pair in pairs {
                    let (id, n) = pair?;
                    if n < 0 {
                        return Err("corrupt metrics data: posting count".into());
                    }
                    ids.push(id);
                    count += n;
                }
                PostingSet {
                    ids,
                    count,
                    prefix: None,
                }
            }
        };
        if matches!(condition, Condition::NoneOf(_)) {
            if !set.ids.is_empty() {
                leaving.push(set);
            }
        } else if set.count == 0 {
            return Ok(Vec::new());
        } else {
            finding.push(set);
        }
    }
    finding.sort_by_key(|set| set.count);
    let membership = |set: &PostingSet, column: &str| {
        if q.conditions.is_empty() {
            (format!("{column}=?"), vec![Value::Integer(set.ids[0])])
        } else {
            set.membership(column)
        }
    };
    let (first, mut arguments) = membership(&finding[0], "p.label_id");
    let mut filter = format!("SELECT p.series_id FROM postings p WHERE {first}");
    for set in &finding[1..] {
        let (membership, more) = membership(set, "q.label_id");
        filter.push_str(&format!(
            " AND EXISTS(SELECT 1 FROM postings q WHERE {membership} AND q.series_id=p.series_id)"
        ));
        arguments.extend(more);
    }
    for set in leaving {
        let (membership, more) = set.membership("q.label_id");
        filter.push_str(&format!(" AND NOT EXISTS(SELECT 1 FROM postings q WHERE {membership} AND q.series_id=p.series_id)"));
        arguments.extend(more);
    }
    let sql = format!(
        "SELECT id,kind,length(label_ids),CASE WHEN length(label_ids)<=? THEN label_ids ELSE NULL END FROM (SELECT s.id,s.kind,s.label_ids FROM ({filter}) m JOIN series s ON s.id=m.series_id) ORDER BY id LIMIT CAST(? AS INTEGER)"
    );
    arguments.insert(
        0,
        Value::Integer(
            budget
                .limits
                .payload_bytes
                .saturating_sub(budget.spent.bytes) as i64,
        ),
    );
    arguments.push(Value::Integer(budget.limits.series as i64 + 1));
    let mut statement = conn.prepare_cached(&sql)?;
    let mut rows = statement.query(params_from_iter(arguments))?;
    let mut matched = Vec::new();
    while let Some(row) = rows.next()? {
        budget.spent.series = matched.len() + 1;
        if matched.len() == budget.limits.series {
            return Err("metrics resource limit: matched series".into());
        }
        let id: i64 = row.get(0)?;
        let kind: String = row.get(1)?;
        let size: i64 = row.get(2)?;
        if size < 0 {
            return Err("corrupt metrics data: label ids size".into());
        }
        budget.bytes(size as usize)?;
        let owned;
        let source = if crate::adapter::borrow_metadata() {
            row.get_ref(3)?
                .as_blob()
                .map_err(|_| "corrupt metrics data: series metadata")?
        } else {
            owned = row
                .get::<_, Option<Vec<u8>>>(3)?
                .ok_or("corrupt metrics data: series metadata")?;
            &owned
        };
        if kind != "gauge" && kind != "counter" {
            return Err("corrupt metrics data: kind".into());
        }
        matched.push(Registered {
            id,
            series: Series {
                name: String::new(),
                kind,
                labels: BTreeMap::new(),
            },
            ids: decode_label_ids(source)?,
        });
    }
    drop(rows);
    drop(statement);
    let ids: BTreeSet<i64> = matched
        .iter()
        .flat_map(|series| series.ids.iter().copied())
        .collect();
    let ids: Vec<i64> = ids.into_iter().collect();
    let mut dictionary = HashMap::new();
    for chunk in ids.chunks(256) {
        let sql = format!(
            "SELECT id,name,value FROM label_values WHERE id IN ({})",
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare_cached(&sql)?;
        let mut rows = statement.query(params_from_iter(chunk))?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let value: String = row.get(2)?;
            budget.bytes(name.len() + value.len())?;
            dictionary.insert(id, (name, value));
        }
    }
    for series in &mut matched {
        let mut pairs = Vec::new();
        for id in &series.ids {
            pairs.push(
                dictionary
                    .get(id)
                    .ok_or("corrupt metrics data: label missing")?
                    .clone(),
            );
        }
        series.series = public_series(pairs, series.series.kind.clone())?;
    }
    Ok(matched)
}

fn fetch_each_groups(
    conn: &Connection,
    q: &Checked,
    id: i64,
    budget: &mut Budget,
) -> Result<Vec<GroupRow>> {
    let _profile = crate::profile::scope("fetch_group_rows");
    let mut statement=conn.prepare_cached("SELECT start_ts,end_ts,length(directory),CASE WHEN length(directory)<=? THEN directory ELSE NULL END,clock_id FROM groups WHERE series_id=? AND start_ts>=coalesce((SELECT start_ts FROM groups WHERE series_id=? AND start_ts<=? ORDER BY start_ts DESC LIMIT 1),?) AND start_ts<? AND end_ts>=? ORDER BY start_ts LIMIT CAST(? AS INTEGER)")?;
    let mut rows = statement.query(params![
        budget
            .limits
            .payload_bytes
            .saturating_sub(budget.spent.bytes) as i64,
        id,
        id,
        q.from,
        q.from,
        q.to,
        q.from,
        budget.limits.blocks as i64 + 1
    ])?;
    let mut groups = Vec::new();
    while let Some(row) = rows.next()? {
        let start = row.get(0)?;
        let end = row.get(1)?;
        let size: i64 = row.get(2)?;
        let data: Option<Vec<u8>> = row.get(3)?;
        let clock_id = row.get(4)?;
        budget.group()?;
        if !(0..=8192).contains(&size) {
            return Err("corrupt metrics data: directory size".into());
        }
        budget.bytes(size as usize)?;
        groups.push(GroupRow {
            id,
            start,
            end,
            clock_id,
            size: size as usize,
            data: data.ok_or("corrupt metrics data: directory missing")?,
        });
    }
    Ok(groups)
}

fn fetch_group_rows(
    conn: &Connection,
    q: &Checked,
    matched: &[Registered],
    budget: &mut Budget,
) -> Result<HashMap<i64, Vec<GroupRow>>> {
    let _profile = crate::profile::scope("fetch_group_rows");
    let mut groups: HashMap<i64, Vec<GroupRow>> = HashMap::new();
    for chunk in matched.chunks(64) {
        let ids: Vec<i64> = chunk.iter().map(|s| s.id).collect();
        let json = serde_json::to_string(&ids)?;
        let mut statement=conn.prepare_cached("SELECT g.series_id,g.start_ts,g.end_ts,length(g.directory),g.clock_id FROM json_each(?) ids JOIN groups g ON g.series_id=CAST(ids.value AS INTEGER) WHERE g.start_ts>=coalesce((SELECT prior.start_ts FROM groups prior WHERE prior.series_id=g.series_id AND prior.start_ts<=? ORDER BY prior.start_ts DESC LIMIT 1),?) AND g.start_ts<? AND g.end_ts>=? ORDER BY g.series_id,g.start_ts LIMIT CAST(? AS INTEGER)")?;
        let mut rows = statement.query(params![
            json,
            q.from,
            q.from,
            q.to,
            q.from,
            (budget.limits.blocks - budget.spent.groups + 1) as i64
        ])?;
        let mut addresses = Vec::new();
        while let Some(row) = rows.next()? {
            let id = row.get(0)?;
            let start = row.get(1)?;
            let end = row.get(2)?;
            let size: i64 = row.get(3)?;
            let clock_id = row.get(4)?;
            budget.group()?;
            if !(0..=8192).contains(&size) {
                return Err("corrupt metrics data: directory size".into());
            }
            budget.bytes(size as usize)?;
            let list = groups.entry(id).or_default();
            let position = list.len();
            list.push(GroupRow {
                id,
                start,
                end,
                clock_id,
                size: size as usize,
                data: Vec::new(),
            });
            addresses.push((id, start, position));
        }
        drop(rows);
        drop(statement);
        for selected in addresses.chunks(64) {
            let pairs: Vec<[i64; 2]> = selected
                .iter()
                .map(|(id, start, _)| [*id, *start])
                .collect();
            let json = serde_json::to_string(&pairs)?;
            let locations: HashMap<(i64, i64), usize> = selected
                .iter()
                .map(|(id, start, p)| ((*id, *start), *p))
                .collect();
            let mut statement=conn.prepare_cached("SELECT g.series_id,g.start_ts,g.directory FROM json_each(?) selected JOIN groups g ON g.series_id=CAST(json_extract(selected.value,'$[0]') AS INTEGER) AND g.start_ts=CAST(json_extract(selected.value,'$[1]') AS INTEGER)")?;
            let mut rows = statement.query([json])?;
            let mut seen = BTreeSet::new();
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                let start: i64 = row.get(1)?;
                let body: Vec<u8> = row.get(2)?;
                let position = *locations
                    .get(&(id, start))
                    .ok_or("corrupt metrics data: selected group address")?;
                let into = &mut groups.get_mut(&id).unwrap()[position];
                if !seen.insert((id, start)) || body.len() != into.size {
                    return Err("corrupt metrics data: selected group size or repeat".into());
                }
                into.data = body;
            }
            if seen.len() != selected.len() {
                return Err("corrupt metrics data: selected group missing".into());
            }
        }
    }
    Ok(groups)
}

fn fetch_payloads(conn: &Connection, reads: &mut [SeriesRead]) -> Result<()> {
    fetch_payloads_mode(conn, reads, crate::adapter::arena())
}

fn fetch_payloads_mode(conn: &Connection, reads: &mut [SeriesRead], use_arena: bool) -> Result<()> {
    let _profile = crate::profile::scope("fetch_payloads");
    let mut destinations = HashMap::new();
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    for (ri, read) in reads.iter().enumerate() {
        for (bi, selected) in read.blocks.iter().enumerate() {
            let block = &selected.block;
            if block.payload == 0 {
                continue;
            }
            if !seen.insert(block.payload) {
                return Err("corrupt metrics data: selected payload repeats".into());
            }
            if selected.summarized {
                continue;
            }
            destinations.insert(block.payload, (ri, bi, block.body_bytes));
            ids.push(block.payload);
        }
    }
    // Every selected body was charged to the payload budget before this call.
    // Reserve exactly that admitted total, never decode a borrowed SQLite row.
    let total = destinations
        .values()
        .try_fold(0usize, |sum, &(_, _, size)| sum.checked_add(size))
        .ok_or("metrics resource limit: arena capacity")?;
    let mut arena = Vec::new();
    let mut ranges = Vec::new();
    if use_arena {
        arena.try_reserve_exact(total)?;
        ranges.try_reserve_exact(ids.len())?;
    }
    for chunk in ids.chunks(64) {
        let json = serde_json::to_string(chunk)?;
        let mut statement=conn.prepare_cached("SELECT p.id,p.body FROM json_each(?) ids JOIN payloads p ON p.id=CAST(ids.value AS INTEGER)")?;
        let mut rows = statement.query([json])?;
        let mut fetched = BTreeSet::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let owned = if use_arena {
                None
            } else {
                Some(row.get::<_, Vec<u8>>(1)?)
            };
            let body = if use_arena {
                row.get_ref(1)?.as_blob()?
            } else {
                owned.as_deref().unwrap()
            };
            let &(ri, bi, size) = destinations
                .get(&id)
                .ok_or("corrupt metrics data: payload id")?;
            if !fetched.insert(id) || body.len() != size {
                return Err("corrupt metrics data: payload size or repeat".into());
            }
            if use_arena {
                if body.len() > total.saturating_sub(arena.len()) {
                    return Err("metrics resource limit: arena capacity".into());
                }
                let start = arena.len();
                arena.extend_from_slice(body);
                ranges.push((ri, bi, start..arena.len()));
            } else {
                reads[ri].blocks[bi].block.body = owned.unwrap();
            }
        }
        if fetched.len() != chunk.len() {
            return Err("corrupt metrics data: selected payload missing".into());
        }
    }
    if use_arena {
        if arena.len() != total {
            return Err("corrupt metrics data: arena size".into());
        }
        let bytes = std::sync::Arc::new(arena);
        for (ri, bi, range) in ranges {
            reads[ri].blocks[bi].block.shared_body = Some((bytes.clone(), range));
        }
    }
    Ok(())
}

pub fn bucket_edges(origin: i64, to: i64, at: i64, width: i64) -> (i64, i64) {
    let span = (at as u64).wrapping_sub(origin as u64);
    let step = width as u64;
    let offset = span / step * step;
    let start = (origin as u64).wrapping_add(offset) as i64;
    let remaining = (to as u64).wrapping_sub(origin as u64) - offset;
    (
        start,
        if step > remaining {
            to
        } else {
            (start as u64).wrapping_add(step) as i64
        },
    )
}

#[derive(Default)]
struct Accumulator {
    from: i64,
    to: i64,
    count: usize,
    resets: usize,
    min: f64,
    max: f64,
    previous_value: f64,
    first: BigInt,
    previous: BigInt,
    exact: BigInt,
    fast: Option<Box<exact::ExactSum>>,
}
impl Accumulator {
    fn add(&mut self, value: f64, op: &str, kind: &str) -> Result<()> {
        if kind == "counter" && (!value.is_finite() || value < 0.0) {
            return Err("invalid counter aggregate input".into());
        }
        let current = if matches!(op, "sum" | "avg") && exact::fast_enabled() {
            self.fast
                .get_or_insert_with(|| Box::new(exact::ExactSum::default()))
                .add_value(value)?;
            None
        } else if exact::fast_enabled() && matches!(op, "count" | "min" | "max") {
            if !value.is_finite() {
                return Err("nonfinite metric value".into());
            }
            None
        } else {
            Some(exact::finite_units(value)?)
        };
        if self.count == 0 {
            self.min = value;
            self.max = value;
        } else {
            self.min = exact::minimum(self.min, value);
            self.max = exact::maximum(self.max, value);
        }
        match op {
            "sum" | "avg" => {
                if let Some(current) = current {
                    self.exact += current;
                }
            }
            "delta" => {
                let current = current.unwrap();
                if self.count == 0 {
                    self.first = current.clone();
                }
                self.previous = current;
            }
            "increase" | "rate" => {
                let current = current.unwrap();
                if self.count > 0 {
                    if value < self.previous_value {
                        self.exact += &current;
                        self.resets += 1;
                    } else {
                        self.exact += &current - &self.previous;
                    }
                }
                self.previous = current;
                self.previous_value = value;
            }
            _ => {}
        }
        self.count += 1;
        Ok(())
    }
    fn merge_summary(&mut self, block: &Block, op: &str) -> Result<()> {
        let s = &block.summary;
        if self.count == 0 {
            self.min = s.min;
            self.max = s.max;
        } else {
            self.min = exact::minimum(self.min, s.min);
            self.max = exact::maximum(self.max, s.max);
        }
        match op {
            "sum" | "avg" => {
                let value = codec::read_exact_value(&s.exact_sum)?;
                // Summaries already own exact BigInts; keep their cheap limb
                // addition and reserve the fixed accumulator for decoded points.
                self.exact += value;
            }
            "delta" => {
                if self.count == 0 {
                    self.first = exact::finite_units(block.head.first)?;
                }
                self.previous = exact::finite_units(s.last)?;
            }
            "increase" | "rate" => {
                let current = exact::finite_units(block.head.first)?;
                if self.count > 0 {
                    if block.head.first < self.previous_value {
                        self.exact += &current;
                        self.resets += 1;
                    } else {
                        self.exact += &current - &self.previous;
                    }
                }
                let increase = codec::read_exact_value(&s.exact_increase)?;
                if increase < num_bigint::BigInt::zero() {
                    return Err("corrupt metrics data: negative exact increase".into());
                }
                self.exact += increase;
                self.resets += s.resets as usize;
                self.previous_value = s.last;
                self.previous = exact::finite_units(s.last)?;
            }
            _ => {}
        }
        self.count = self
            .count
            .checked_add(block.head.count)
            .ok_or("metrics resource limit: aggregate summary count")?;
        Ok(())
    }
    fn close(&mut self, op: &str) {
        if op == "delta" {
            self.exact = &self.previous - &self.first;
        }
    }
    fn join(&mut self, other: Self) -> Result<()> {
        if self.count == 0 {
            self.min = other.min;
            self.max = other.max;
        } else {
            self.min = exact::minimum(self.min, other.min);
            self.max = exact::maximum(self.max, other.max);
        }
        self.count = self
            .count
            .checked_add(other.count)
            .ok_or("metrics resource limit: aggregate group count")?;
        self.resets += other.resets;
        self.exact += other.exact;
        if let Some(other_fast) = other.fast {
            let into = self
                .fast
                .get_or_insert_with(|| Box::new(exact::ExactSum::default()));
            into.merge(&other_fast)?;
        }
        Ok(())
    }
    fn join_operation(&mut self, other: Self, op: &str) -> Result<()> {
        if crate::tuning::specialized() {
            specialized::join(self, other, op)
        } else {
            self.join(other)
        }
    }
    fn result(&self, op: &str, retained_from: i64) -> Result<AggregateBucket> {
        if op == "count" && self.count > 1_usize << 53 {
            return Err("metrics resource limit: exact count representation".into());
        }
        let fixed = self.fast.as_ref().map(|sum| sum.to_bigint() + &self.exact);
        let total = fixed.as_ref().unwrap_or(&self.exact);
        let (value, overflow) = match op {
            "count" => (self.count as f64, false),
            "min" => (self.min, false),
            "max" => (self.max, false),
            "sum" | "increase" | "delta" => exact::rounded_exact(total),
            "avg" => exact::rounded_quotient(total, &BigInt::from(self.count)),
            "rate" => exact::rounded_quotient(
                &(&self.exact * 1000),
                &BigInt::from((self.to as u64).wrapping_sub(self.from as u64)),
            ),
            _ => return Err("invalid metrics request: aggregate operation".into()),
        };
        Ok(AggregateBucket {
            from: self.from,
            to: self.to,
            count: self.count,
            resets: if matches!(op, "sum" | "increase" | "delta" | "rate") {
                self.resets
            } else {
                0
            },
            value,
            overflow,
            partial: self.from < retained_from,
        })
    }
}

impl Engine {
    pub fn aggregate(&self, request: &AggregateRequest) -> Result<Vec<AggregateResult>> {
        let _profile = crate::profile::scope("aggregate_total");
        if request.width <= 0
            || !matches!(
                request.op.as_str(),
                "count" | "sum" | "avg" | "min" | "max" | "delta" | "increase" | "rate"
            )
        {
            return Err("invalid metrics request: aggregate width or operation".into());
        }
        if request.by.is_some() && request.without.is_some() {
            return Err("invalid metrics request: aggregate grouping".into());
        }
        for list in [&request.by, &request.without].into_iter().flatten() {
            if list.len() > 128
                || list
                    .iter()
                    .any(|n| n.is_empty() || n.starts_with("__") || n.len() > 256)
            {
                return Err("invalid metrics request: grouping labels".into());
            }
        }
        let q = self.check_query(&request.query)?;
        if q.from >= q.to {
            return Ok(Vec::new());
        }
        let (reads, _) = self.fetch_snapshot(&q, Some(request.width))?;
        let _process = crate::profile::scope("aggregate_process");
        if parallel_reads(&reads) {
            return parallel_aggregate(reads, &q, request);
        }
        let grouped = request.by.is_some() || request.without.is_some();
        let mut groups: BTreeMap<String, (Series, BTreeMap<i64, Accumulator>)> = BTreeMap::new();
        let mut results = Vec::new();
        let mut output = 0;
        for read in reads {
            let kind = &read.series.kind;
            if matches!(request.op.as_str(), "increase" | "rate") && kind != "counter"
                || request.op == "delta" && kind != "gauge"
            {
                return Err("invalid metrics request: aggregate kind".into());
            }
            let mut member = read.series.clone();
            if grouped {
                member.labels.retain(|name, _| {
                    request.by.as_ref().is_some_and(|by| by.contains(name))
                        || request
                            .without
                            .as_ref()
                            .is_some_and(|without| !without.contains(name))
                });
            }
            let key = series_key(&member);
            let mut output_buckets = Vec::new();
            let mut emit = |mut bucket: Accumulator| -> Result<()> {
                bucket.close(&request.op);
                if grouped {
                    let (_, joined) = groups
                        .entry(key.clone())
                        .or_insert_with(|| (member.clone(), BTreeMap::new()));
                    if !joined.contains_key(&bucket.from) {
                        if output == q.limits.output_samples {
                            return Err("metrics resource limit: output buckets".into());
                        }
                        output += 1;
                        joined.insert(
                            bucket.from,
                            Accumulator {
                                from: bucket.from,
                                to: bucket.to,
                                ..Default::default()
                            },
                        );
                    }
                    joined
                        .get_mut(&bucket.from)
                        .unwrap()
                        .join_operation(bucket, &request.op)?;
                } else {
                    if output == q.limits.output_samples {
                        return Err("metrics resource limit: output buckets".into());
                    }
                    output += 1;
                    output_buckets.push(bucket.result(&request.op, q.from)?);
                }
                Ok(())
            };
            fold_read(&read, &q, request, &mut emit)?;
            if !grouped && !output_buckets.is_empty() {
                results.push(AggregateResult {
                    series: read.series,
                    buckets: output_buckets,
                });
            }
        }

        if grouped {
            for (_, (series, buckets)) in groups {
                let mut out = Vec::new();
                for (_, bucket) in buckets {
                    out.push(bucket.result(&request.op, q.from)?);
                }
                if !out.is_empty() {
                    results.push(AggregateResult {
                        series,
                        buckets: out,
                    });
                }
            }
        }
        Ok(results)
    }
}

struct FoldedSeries {
    series: Series,
    buckets: Vec<Accumulator>,
}
fn fold_series(
    read: SeriesRead,
    q: &Checked,
    request: &AggregateRequest,
    cap: usize,
) -> Outcome<FoldedSeries> {
    let mut buckets = Vec::new();
    let status = fold_read(&read, q, request, &mut |mut bucket| {
        if buckets.len() == cap {
            return Err("metrics resource limit: output buckets".into());
        }
        bucket.close(&request.op);
        buckets.push(bucket);
        Ok(())
    });
    Outcome {
        value: FoldedSeries {
            series: read.series,
            buckets,
        },
        error: status.err().map(|e| e.to_string()),
        requires_slot: false,
    }
}
fn parallel_aggregate(
    reads: Vec<SeriesRead>,
    q: &Checked,
    request: &AggregateRequest,
) -> Result<Vec<AggregateResult>> {
    let grouped = request.by.is_some() || request.without.is_some();
    let mut groups: BTreeMap<String, (Series, BTreeMap<i64, Accumulator>)> = BTreeMap::new();
    let mut results = Vec::new();
    let mut output = 0;
    let mut pending = reads.into_iter();
    loop {
        let batch: Vec<_> = pending.by_ref().take(parallel::batch_size()).collect();
        if batch.is_empty() {
            break;
        }
        // Group members may reuse all existing group buckets, even when no new
        // bucket remains. A per-member cap still bounds speculative output.
        let cap = if grouped {
            q.limits.output_samples
        } else {
            q.limits.output_samples.saturating_sub(output)
        };
        let outcomes: Vec<_> = batch
            .into_par_iter()
            .map(|read| fold_series(read, q, request, cap))
            .collect();
        for outcome in outcomes {
            let mut member = outcome.value.series;
            let mut output_buckets = Vec::new();
            if grouped {
                member.labels.retain(|name, _| {
                    request.by.as_ref().is_some_and(|by| by.contains(name))
                        || request
                            .without
                            .as_ref()
                            .is_some_and(|without| !without.contains(name))
                });
                let key = series_key(&member);
                let (_, joined) = groups
                    .entry(key)
                    .or_insert_with(|| (member.clone(), BTreeMap::new()));
                for bucket in outcome.value.buckets {
                    if !joined.contains_key(&bucket.from) {
                        if output == q.limits.output_samples {
                            return Err("metrics resource limit: output buckets".into());
                        }
                        output += 1;
                        joined.insert(
                            bucket.from,
                            Accumulator {
                                from: bucket.from,
                                to: bucket.to,
                                ..Default::default()
                            },
                        );
                    }
                    joined
                        .get_mut(&bucket.from)
                        .unwrap()
                        .join_operation(bucket, &request.op)?;
                }
            } else {
                for bucket in outcome.value.buckets {
                    if output == q.limits.output_samples {
                        return Err("metrics resource limit: output buckets".into());
                    }
                    output += 1;
                    output_buckets.push(bucket.result(&request.op, q.from)?);
                }
            }
            if let Some(error) = outcome.error {
                return Err(error.into());
            }
            if !grouped && !output_buckets.is_empty() {
                results.push(AggregateResult {
                    series: member,
                    buckets: output_buckets,
                });
            }
        }
    }
    if grouped {
        for (_, (series, buckets)) in groups {
            let mut out = Vec::with_capacity(buckets.len());
            for (_, bucket) in buckets {
                out.push(bucket.result(&request.op, q.from)?);
            }
            if !out.is_empty() {
                results.push(AggregateResult {
                    series,
                    buckets: out,
                });
            }
        }
    }
    Ok(results)
}

fn fold_read<F>(
    read: &SeriesRead,
    q: &Checked,
    request: &AggregateRequest,
    emit: &mut F,
) -> Result<()>
where
    F: FnMut(Accumulator) -> Result<()>,
{
    let _profile = crate::profile::scope("fold_series");
    if crate::tuning::specialized() {
        specialized::fold_read(
            read,
            q,
            request,
            emit,
            exact::fast_enabled(),
            crate::tuning::fused(),
        )
    } else {
        fold_reference(read, q, request, emit, crate::tuning::fused())
    }
}

fn fold_reference<F>(
    read: &SeriesRead,
    q: &Checked,
    request: &AggregateRequest,
    emit: &mut F,
    fused: bool,
) -> Result<()>
where
    F: FnMut(Accumulator) -> Result<()>,
{
    let kind = &read.series.kind;
    if matches!(request.op.as_str(), "increase" | "rate") && kind != "counter"
        || request.op == "delta" && kind != "gauge"
    {
        return Err("invalid metrics request: aggregate kind".into());
    }
    let mut current = Accumulator::default();
    let mut previous_at = None;
    for selected in &read.blocks {
        if selected.summarized {
            let block = &selected.block;
            if kind == "counter" && block.summary.min < 0.0 {
                return Err("invalid counter aggregate input".into());
            }
            if previous_at.is_some_and(|at| block.head.start <= at) {
                return Err("corrupt metrics data: overlapping summary samples".into());
            }
            let (start, end) = bucket_edges(q.origin, q.to, block.head.start, request.width);
            if current.count > 0 && start != current.from {
                emit(std::mem::take(&mut current))?;
            }
            current.from = start;
            current.to = end;
            current.merge_summary(block, &request.op)?;
            previous_at = Some(block.head.end);
        } else {
            if fused {
                codec::visit_block(&selected.block, |p| {
                    fold_sample(p, q, request, kind, &mut previous_at, &mut current, emit)
                        .map_err(|error| error.to_string())
                })?;
            } else {
                for p in codec::decode_block(&selected.block)? {
                    fold_sample(p, q, request, kind, &mut previous_at, &mut current, emit)?;
                }
            }
        }
    }
    if fused {
        codec::visit_chunks(&read.head, q.from, q.to, |p| {
            fold_sample(p, q, request, kind, &mut previous_at, &mut current, emit)
                .map_err(|error| error.to_string())
        })?;
    } else {
        for p in codec::decode_chunks(&read.head, q.from, q.to)? {
            fold_sample(p, q, request, kind, &mut previous_at, &mut current, emit)?;
        }
    }
    if current.count > 0 {
        emit(current)?;
    }
    Ok(())
}

fn fold_sample<F>(
    p: Sample,
    q: &Checked,
    request: &AggregateRequest,
    kind: &str,
    previous_at: &mut Option<i64>,
    current: &mut Accumulator,
    emit: &mut F,
) -> Result<()>
where
    F: FnMut(Accumulator) -> Result<()>,
{
    if p.at < q.from || p.at >= q.to {
        return Ok(());
    }
    if previous_at.is_some_and(|at| p.at <= at) {
        return Err("corrupt metrics data: overlapping samples".into());
    }
    *previous_at = Some(p.at);
    let (start, end) = bucket_edges(q.origin, q.to, p.at, request.width);
    if current.count > 0 && start != current.from {
        emit(std::mem::take(current))?;
    }
    current.from = start;
    current.to = end;
    current.add(p.value, &request.op, kind)
}

fn series_key(series: &Series) -> String {
    let mut text = format!("{} {}", series.kind, series.name);
    if !series.labels.is_empty() {
        text.push('{');
        for (i, (key, value)) in series.labels.iter().enumerate() {
            if i > 0 {
                text.push(',');
            }
            text.push_str(key);
            text.push('=');
            text.push_str(&quote::quote(value));
        }
        text.push('}');
    }
    text
}

#[cfg(test)]
mod parallel_tests {
    use super::*;

    fn reads(corrupt_last: bool) -> Vec<SeriesRead> {
        (0..3)
            .map(|id| {
                let points = [
                    Sample {
                        at: 0,
                        value: id as f64,
                    },
                    Sample {
                        at: 1,
                        value: id as f64 + 1.0,
                    },
                ];
                let mut group = codec::prepare_group(id, &points, 0, -2, 100).unwrap();
                if corrupt_last && id == 2 {
                    group.blocks[0].body = vec![255];
                }
                SeriesRead {
                    series: Series {
                        name: "ordered".into(),
                        kind: "gauge".into(),
                        labels: BTreeMap::from([("id".into(), id.to_string())]),
                    },
                    blocks: group
                        .blocks
                        .into_iter()
                        .map(|block| SelectedBlock {
                            block,
                            summarized: false,
                        })
                        .collect(),
                    head: Vec::new(),
                }
            })
            .collect()
    }

    #[test]
    fn ordered_parallel_stream_preserves_limits_errors_and_callback_stop() {
        configure_parallel(2, true).unwrap();
        let mut q = Checked {
            origin: 0,
            from: 0,
            to: 2,
            limits: Limits::defaults(),
            matchers: BTreeMap::new(),
            conditions: BTreeMap::new(),
        };
        let mut ids = Vec::new();
        stream_reads(reads(false), &q, &mut |r| {
            ids.push(r.series.labels["id"].clone());
            assert_eq!(r.samples.iter().map(|p| p.at).collect::<Vec<_>>(), [0, 1]);
            Ok(())
        })
        .unwrap();
        assert_eq!(ids, ["0", "1", "2"]);

        // A later speculative corruption must not replace the earlier global
        // output-limit error, and no partial series reaches the callback.
        q.limits.output_samples = 3;
        ids.clear();
        let error = stream_reads(reads(true), &q, &mut |r| {
            ids.push(r.series.labels["id"].clone());
            Ok(())
        })
        .unwrap_err()
        .to_string();
        assert_eq!(error, "metrics resource limit: output samples");
        assert_eq!(ids, ["0"]);

        q.limits.output_samples = 100;
        ids.clear();
        let error = stream_reads(reads(false), &q, &mut |r| {
            ids.push(r.series.labels["id"].clone());
            Err("consumer stop".into())
        })
        .unwrap_err()
        .to_string();
        assert_eq!(error, "consumer stop");
        assert_eq!(ids, ["0"]);

        let request = AggregateRequest {
            query: Query::default(),
            width: 2,
            op: "sum".into(),
            by: Some(Vec::new()),
            without: None,
        };
        let out = parallel_aggregate(reads(false), &q, &request).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].buckets[0].count, 6);
        assert_eq!(out[0].buckets[0].value.to_bits(), 9.0_f64.to_bits());
    }
}
