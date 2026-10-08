use crate::codec::{self, Group, HeadChunk};
use crate::query::Limits;
use base64::Engine as _;
use rusqlite::config::DbConfig;
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params, params_from_iter};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub at: i64,
    pub value: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Series {
    pub name: String,
    pub kind: String,
    pub labels: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct Batch {
    pub series: Series,
    pub samples: Vec<Sample>,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub retention: i64,
    pub lateness: i64,
    pub clock_skew: i64,
    pub max_block_span: i64,
    pub max_series: usize,
    pub max_head_samples: usize,
    pub max_head_bytes: usize,
    pub max_batch_samples: usize,
    pub max_batch_bytes: usize,
    pub maintenance_series: usize,
    pub max_readers: usize,
    pub max_concurrent_reads: usize,
    pub snapshot_timeout_ms: i64,
    pub limits: Limits,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            retention: 30 * 24 * 60 * 60 * 1000,
            lateness: 0,
            clock_skew: 10 * 60 * 1000,
            max_block_span: 24 * 60 * 60 * 1000,
            max_series: 100000,
            max_head_samples: 4096,
            max_head_bytes: 256 << 10,
            max_batch_samples: 10000,
            max_batch_bytes: 4 << 20,
            maintenance_series: 64,
            max_readers: 2,
            max_concurrent_reads: 2,
            snapshot_timeout_ms: 5000,
            limits: Limits::defaults(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Invalid,
    Limit,
    TooOld,
    TooNew,
    Conflict,
    Corrupt,
    Suspended,
}

#[derive(Debug)]
struct Failure {
    kind: ErrorKind,
    message: String,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Failure {}

pub fn failure(kind: ErrorKind, message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(Failure {
        kind,
        message: message.into(),
    })
}

fn error_kind(error: &(dyn std::error::Error + 'static)) -> Option<ErrorKind> {
    error.downcast_ref::<Failure>().map(|failure| failure.kind)
}

pub struct Engine {
    pub(crate) writer: Connection,
    pub(crate) reader: Connection,
    pub(crate) options: Options,
    pub(crate) now: i64,
    ready_cursor: Cell<i64>,
    pub(crate) ingested_samples: Cell<u64>,
}

fn configure(connection: &Connection, reader: bool) -> Result<()> {
    connection.execute_batch(
        "PRAGMA foreign_keys=1; PRAGMA busy_timeout=5000;
         PRAGMA synchronous=FULL; PRAGMA fullfsync=1;
         PRAGMA checkpoint_fullfsync=1; PRAGMA cache_size=-1024;",
    )?;
    if reader {
        connection.execute_batch("PRAGMA query_only=1")?;
    }
    for config in [
        DbConfig::SQLITE_DBCONFIG_DQS_DDL,
        DbConfig::SQLITE_DBCONFIG_DQS_DML,
        DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA,
    ] {
        connection.set_db_config(config, false)?;
    }
    connection.set_prepared_statement_cache_capacity(32);
    Ok(())
}

impl Engine {
    pub fn open(path: &Path, options: Options, now: i64) -> Result<Self> {
        if options.retention <= 0
            || options.lateness < 0
            || options.clock_skew < 0
            || options.max_block_span <= 0
            || options.max_head_samples == 0
            || options.max_head_samples > 1 << 20
            || options.max_head_bytes == 0
            || options.max_head_bytes > 16 << 20
            || options.max_series == 0
            || options.max_batch_samples == 0
            || options.max_batch_bytes == 0
            || options.maintenance_series == 0
            || options.snapshot_timeout_ms < 0
        {
            return Err(failure(ErrorKind::Invalid, "metrics options"));
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let writer = Connection::open_with_flags(path, flags)?;
        configure(&writer, false)?;
        let version: String = writer.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
        if version != "3.53.4" {
            return Err(format!("expected SQLite 3.53.4, linked {version}").into());
        }
        let application: i64 = writer.query_row("PRAGMA application_id", [], |row| row.get(0))?;
        if application != 0x544d4554 {
            return Err(failure(ErrorKind::Corrupt, "metrics file identity"));
        }
        let mut migration = writer
            .prepare("select version,name,checksum from _tinystore_migrations order by version")?;
        let migrated = migration
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let checksum = [
            0xbe, 0xe5, 0xd2, 0x64, 0xdb, 0xf3, 0xfa, 0xfe, 0x45, 0x92, 0x82, 0xb5, 0xd3, 0x56,
            0x49, 0x91, 0xdd, 0x15, 0xfd, 0xef, 0x61, 0xbe, 0x3b, 0x15, 0x3a, 0x83, 0x97, 0xbd,
            0x1f, 0x45, 0x1a, 0x20,
        ];
        if migrated.len() != 1
            || migrated[0].0 != 1
            || migrated[0].1 != "0001_schema.sql"
            || migrated[0].2 != checksum
        {
            return Err(failure(
                ErrorKind::Corrupt,
                "metrics migration version, name or checksum",
            ));
        }
        drop(migration);
        let journal: String = writer.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        if journal != "wal" {
            return Err("metrics database did not enter WAL".into());
        }
        let reader = Connection::open_with_flags(path, flags)?;
        configure(&reader, true)?;
        // Opening Go's engine counts persisted suspended registrations too.
        let _: i64 = reader.query_row(
            "SELECT count(*) FROM series_state WHERE failed_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(Self {
            writer,
            reader,
            options,
            now,
            ready_cursor: Cell::new(0),
            ingested_samples: Cell::new(0),
        })
    }

    pub fn cutoff(&self) -> i64 {
        self.now.saturating_sub(self.options.retention)
    }

    pub(crate) fn view<T>(&self, work: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        transaction(&self.reader, "BEGIN DEFERRED", work)
    }

    fn update<T>(&self, work: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        transaction(&self.writer, "BEGIN IMMEDIATE", work)
    }
}

fn transaction<T>(
    connection: &Connection,
    begin: &str,
    work: impl FnOnce(&Connection) -> Result<T>,
) -> Result<T> {
    connection.execute_batch(begin)?;
    match work(connection) {
        Ok(output) => match connection.execute_batch("COMMIT") {
            Ok(()) => Ok(output),
            Err(error) => {
                let _ = connection.execute_batch("ROLLBACK");
                Err(error.into())
            }
        },
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn canonical_labels(series: &Series) -> Result<(Vec<Label>, String)> {
    let mut labels = Vec::with_capacity(series.labels.len() + 1);
    if !series.name.is_empty() {
        labels.push(Label {
            name: "__name__".into(),
            value: series.name.clone(),
        });
    }
    for (name, value) in &series.labels {
        if name.starts_with("__") {
            return Err(failure(ErrorKind::Invalid, "reserved label prefix"));
        }
        labels.push(Label {
            name: name.clone(),
            value: value.clone(),
        });
    }
    labels.sort_by(|a, b| a.name.cmp(&b.name));
    if labels.is_empty() || labels.len() > 128 {
        return Err(failure(ErrorKind::Invalid, "label count"));
    }
    let mut bytes = 0;
    for label in &labels {
        bytes += label.name.len() + label.value.len();
        if label.name.is_empty()
            || label.name.len() > 256
            || label.value.len() > 4096
            || bytes > 16 << 10
        {
            return Err(failure(ErrorKind::Invalid, "label name or size"));
        }
    }
    if series.name.is_empty() {
        return Err(failure(ErrorKind::Invalid, "a series needs a name"));
    }
    let pairs: Vec<[&str; 2]> = labels
        .iter()
        .map(|label| [label.name.as_str(), label.value.as_str()])
        .collect();
    // Go's json.Marshal escapes HTML and the two Unicode line separators.
    let identity = serde_json::to_string(&pairs)?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    Ok((labels, identity))
}

pub(crate) fn series_identity(identity: &str) -> String {
    let digest = Sha256::digest(identity.as_bytes());
    format!(
        "@{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    )
}

fn append_unsigned(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(value as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_unsigned(data: &[u8], offset: &mut usize) -> Result<u64> {
    let mut value = 0;
    for i in 0..10 {
        let byte = *data
            .get(*offset)
            .ok_or_else(|| failure(ErrorKind::Corrupt, "label identifier varint"))?;
        *offset += 1;
        if i == 9 && byte > 1 {
            return Err(failure(ErrorKind::Corrupt, "label identifier varint"));
        }
        value |= ((byte & 0x7f) as u64) << (i * 7);
        if byte < 0x80 {
            return Ok(value);
        }
    }
    Err(failure(ErrorKind::Corrupt, "label identifier varint"))
}

pub fn decode_label_ids(data: &[u8]) -> Result<Vec<i64>> {
    let mut offset = 0;
    let count = read_unsigned(data, &mut offset)?;
    if count == 0 || count > 128 {
        return Err(failure(ErrorKind::Corrupt, "label identifier count"));
    }
    let mut ids = Vec::with_capacity(count as usize);
    let mut previous = 0_i64;
    for _ in 0..count {
        let gap = read_unsigned(data, &mut offset)?;
        if gap == 0 || gap > (i64::MAX - previous) as u64 {
            return Err(failure(ErrorKind::Corrupt, "label identifier"));
        }
        previous += gap as i64;
        ids.push(previous);
    }
    if offset != data.len() {
        return Err(failure(ErrorKind::Corrupt, "trailing label identifiers"));
    }
    Ok(ids)
}

fn encode_label_ids(ids: &[i64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 * ids.len() + 1);
    append_unsigned(&mut out, ids.len() as u64);
    let mut previous = 0;
    for &id in ids {
        append_unsigned(&mut out, (id - previous) as u64);
        previous = id;
    }
    out
}

#[derive(Clone)]
struct PreparedBatch {
    identity: String,
    labels: Vec<Label>,
    kind: String,
    samples: Vec<Sample>,
}

struct PendingSeries {
    batch: PreparedBatch,
    by_time: Option<HashMap<i64, Sample>>,
}

impl PendingSeries {
    fn add(&mut self, point: Sample) {
        if self.by_time.is_none()
            && self
                .batch
                .samples
                .last()
                .is_some_and(|last| point.at <= last.at)
        {
            let mut mapped = HashMap::with_capacity(self.batch.samples.len() + 1);
            for earlier in self.batch.samples.drain(..) {
                mapped.insert(earlier.at, earlier);
            }
            self.by_time = Some(mapped);
        }
        if let Some(mapped) = &mut self.by_time {
            mapped.insert(point.at, point);
        } else {
            self.batch.samples.push(point);
        }
    }

    fn sorted(mut self) -> PreparedBatch {
        if let Some(mapped) = self.by_time {
            self.batch.samples.extend(mapped.into_values());
            self.batch.samples.sort_by_key(|sample| sample.at);
        }
        self.batch
    }
}

impl Engine {
    fn prepare_ingest(&self, batches: &[Batch]) -> Result<Vec<PreparedBatch>> {
        let (cutoff, horizon) = (
            self.cutoff(),
            self.now.saturating_add(self.options.clock_skew),
        );
        let (mut samples, mut bytes) = (0, 0);
        let mut pending = HashMap::<String, PendingSeries>::new();
        for batch in batches {
            if batch.samples.is_empty() {
                return Err(failure(ErrorKind::Invalid, "empty series batch"));
            }
            if batch.samples.len() > self.options.max_batch_samples - samples {
                return Err(failure(ErrorKind::Limit, "batch samples"));
            }
            samples += batch.samples.len();
            let (labels, identity) = canonical_labels(&batch.series)?;
            let kind = match batch.series.kind.as_str() {
                "" | "gauge" => "gauge",
                "counter" => "counter",
                _ => return Err(failure(ErrorKind::Invalid, "series kind")),
            };
            let cost = identity.len() + 16 * batch.samples.len();
            if cost > self.options.max_batch_bytes - bytes {
                return Err(failure(ErrorKind::Limit, "batch bytes"));
            }
            bytes += cost;
            let series = pending
                .entry(identity.clone())
                .or_insert_with(|| PendingSeries {
                    batch: PreparedBatch {
                        identity,
                        labels,
                        kind: kind.into(),
                        samples: Vec::new(),
                    },
                    by_time: None,
                });
            if series.batch.kind != kind {
                return Err(failure(ErrorKind::Invalid, "conflicting kinds in batch"));
            }
            for &point in &batch.samples {
                if point.at == i64::MAX {
                    return Err(failure(
                        ErrorKind::Invalid,
                        "MaxInt64 is reserved for the exclusive range bound",
                    ));
                }
                if point.at < cutoff {
                    return Err(failure(ErrorKind::TooOld, "retention cutoff"));
                }
                if point.at > horizon {
                    return Err(failure(ErrorKind::TooNew, "clock and skew horizon"));
                }
                series.add(point);
            }
        }
        let mut input: Vec<_> = pending.into_values().map(PendingSeries::sorted).collect();
        input.sort_by(|a, b| a.identity.cmp(&b.identity));
        Ok(input)
    }

    pub fn ingest(&self, batches: &[Batch]) -> Result<usize> {
        if batches.is_empty() {
            return Ok(0);
        }
        let input = self.prepare_ingest(batches)?;
        let cutoff = self.cutoff();
        self.update(|connection| {
            for batch in &input {
                let id = self.resolve_series(connection, batch)?;
                self.write_head(connection, id, &batch.samples, cutoff)?;
            }
            Ok(())
        })?;
        let count: usize = input.iter().map(|batch| batch.samples.len()).sum();
        self.ingested_samples
            .set(self.ingested_samples.get() + count as u64);
        Ok(count)
    }

    fn resolve_series(&self, connection: &Connection, batch: &PreparedBatch) -> Result<i64> {
        let identity = series_identity(&batch.identity);
        let saved: Option<(i64, String, Vec<u8>)> = connection
            .prepare_cached("select id, kind, label_ids from series where identity = ?")?
            .query_row([&identity], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?;
        if let Some((id, kind, stored)) = saved {
            if kind != batch.kind {
                return Err(failure(ErrorKind::Conflict, "a series cannot change kind"));
            }
            let (want, complete) = lookup_label_ids(connection, &batch.labels)?;
            if !complete || decode_label_ids(&stored)? != want {
                return Err(failure(ErrorKind::Conflict, "series digest collision"));
            }
            return Ok(id);
        }
        let count: i64 = connection
            .prepare_cached("select series_count from store_state where id=1")?
            .query_row([], |row| row.get(0))?;
        if count >= self.options.max_series as i64 {
            return Err(failure(ErrorKind::Limit, "series cardinality"));
        }
        let (mut ids, mut complete) = lookup_label_ids(connection, &batch.labels)?;
        if !complete {
            for label in &batch.labels {
                connection.prepare_cached("insert into label_values(name,value) values(?,?) on conflict(name,value) do nothing")?
                    .execute(params![&label.name, &label.value])?;
            }
            (ids, complete) = lookup_label_ids(connection, &batch.labels)?;
        }
        if !complete {
            return Err(failure(ErrorKind::Corrupt, "label dictionary"));
        }
        connection
            .prepare_cached("insert into series(identity, label_ids, kind) values(?, ?, ?)")?
            .execute(params![identity, encode_label_ids(&ids), batch.kind])?;
        let id = connection.last_insert_rowid();
        for label_id in &ids {
            connection
                .prepare_cached("insert into postings(label_id,series_id) values(?,?)")?
                .execute(params![label_id, id])?;
        }
        let query = format!(
            "update label_values set posting_count=posting_count+1 where id in ({})",
            vec!["?"; ids.len()].join(",")
        );
        let changed = connection
            .prepare_cached(&query)?
            .execute(params_from_iter(ids.iter()))?;
        if changed != ids.len() {
            return Err(failure(ErrorKind::Corrupt, "missing posting counter"));
        }
        connection
            .prepare_cached("insert into series_state(series_id,max_seen_ts) values(?,?)")?
            .execute(params![id, batch.samples[0].at])?;
        connection
            .prepare_cached("update store_state set series_count=series_count+1 where id=1")?
            .execute([])?;
        Ok(id)
    }
}

fn lookup_label_ids(connection: &Connection, labels: &[Label]) -> Result<(Vec<i64>, bool)> {
    let query = format!(
        "select id from label_values where (name,value) in (values {})",
        vec!["(?,?)"; labels.len()].join(",")
    );
    let mut arguments: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(2 * labels.len());
    for label in labels {
        arguments.push(&label.name);
        arguments.push(&label.value);
    }
    let mut statement = connection.prepare_cached(&query)?;
    let mut ids = statement
        .query_map(params_from_iter(arguments), |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.sort_unstable();
    let complete = ids.len() == labels.len();
    Ok((ids, complete))
}

struct HeadState {
    frontier: Option<i64>,
    version: i64,
    max_seen: i64,
    ready: i64,
    next_gc: Option<i64>,
    failed_at: Option<i64>,
    failure: Option<String>,
    count: usize,
    start: Option<i64>,
    end: Option<i64>,
    packed: Vec<u8>,
}

impl Engine {
    fn load_ingest_state(&self, connection: &Connection, id: i64) -> Result<HeadState> {
        let state = connection.prepare_cached(
            "select sealed_before, version, max_seen_ts, ready, next_gc_ts, failed_at, failure_reason,
                    head_count, head_start, head_end, coalesce(length(tail), 0),
                    case when length(tail) <= ? then tail end
             from series_state where series_id = ?")?.query_row(params![self.options.max_head_bytes as i64, id], |row| {
            let count: i64 = row.get(7)?;
            let size: i64 = row.get(10)?;
            Ok((HeadState { frontier: row.get(0)?, version: row.get(1)?, max_seen: row.get(2)?, ready: row.get(3)?,
                next_gc: row.get(4)?, failed_at: row.get(5)?, failure: row.get(6)?, count: count.max(0) as usize,
                start: row.get(8)?, end: row.get(9)?, packed: row.get::<_, Option<Vec<u8>>>(11)?.unwrap_or_default() }, count, size))
        })?;
        let (state, count, size) = state;
        if state.failed_at.is_some() {
            return Ok(state);
        }
        if count == 0 {
            if state.start.is_some() || state.end.is_some() || size != 0 {
                return Err(failure(ErrorKind::Corrupt, "empty mutable head"));
            }
        } else {
            if state.start.is_none() || state.end.is_none() || state.end < state.start {
                return Err(failure(ErrorKind::Corrupt, "mutable endpoints"));
            }
            if count < 0
                || count as usize > self.options.max_head_samples
                || size as usize > self.options.max_head_bytes
            {
                return Err(failure(ErrorKind::Limit, "mutable head capacity"));
            }
            if size == 0 || state.packed.len() != size as usize {
                return Err(failure(ErrorKind::Corrupt, "mutable body missing"));
            }
        }
        Ok(state)
    }

    fn write_head(
        &self,
        connection: &Connection,
        id: i64,
        incoming: &[Sample],
        cutoff: i64,
    ) -> Result<()> {
        let state = self.load_ingest_state(connection, id)?;
        if state.failed_at.is_some() {
            return Err(failure(
                ErrorKind::Suspended,
                state.failure.unwrap_or_default(),
            ));
        }
        if state
            .frontier
            .is_some_and(|frontier| incoming[0].at < frontier)
        {
            return Err(failure(ErrorKind::TooOld, "sealed frontier"));
        }
        if state.version == i64::MAX {
            return Err(failure(ErrorKind::Limit, "series version exhausted"));
        }
        let chunks = if state.count == 0 {
            Vec::new()
        } else {
            codec::parse_head(
                id,
                state.count,
                state.start.unwrap(),
                state.end.unwrap(),
                &state.packed,
                self.options.max_head_samples,
                self.options.max_head_bytes,
            )
            .map_err(|error| failure(ErrorKind::Corrupt, error))?
        };
        let newest = state.max_seen.max(incoming[incoming.len() - 1].at);
        let kept_len = reusable_chunks(&chunks, incoming[0].at);
        let kept = &chunks[..kept_len];
        let kept_samples: usize = kept.iter().map(|chunk| chunk.head.count).sum();
        let only_appends = state.count >= 240
            && self.options.lateness == 0
            && cutoff <= state.start.unwrap()
            && incoming[0].at > state.end.unwrap();
        let (packed, count, first, last, ready) = if only_appends {
            let tail = codec::decode_chunks(&chunks[kept_len..], i64::MIN, i64::MAX)
                .map_err(|error| failure(ErrorKind::Corrupt, error))?;
            let merged = merge_head(
                &tail,
                incoming,
                self.options.max_head_samples - kept_samples,
            )?;
            let packed = codec::encode_head_after(
                id,
                kept,
                &merged,
                self.options.max_head_samples,
                self.options.max_head_bytes,
            )
            .map_err(|error| failure(ErrorKind::Limit, error))?;
            let sealable = kept_samples
                + merged
                    .iter()
                    .filter(|point| point.at >= cutoff && point.at < newest)
                    .count();
            (
                packed,
                kept_samples + merged.len(),
                state.start.unwrap(),
                merged.last().unwrap().at,
                sealable >= 240,
            )
        } else {
            let existing = codec::decode_chunks(&chunks, i64::MIN, i64::MAX)
                .map_err(|error| failure(ErrorKind::Corrupt, error))?;
            let merged = merge_head(&existing, incoming, self.options.max_head_samples)?;
            let packed = codec::encode_head_after(
                id,
                kept,
                &merged[kept_samples..],
                self.options.max_head_samples,
                self.options.max_head_bytes,
            )
            .map_err(|error| failure(ErrorKind::Limit, error))?;
            (
                packed,
                merged.len(),
                merged[0].at,
                merged.last().unwrap().at,
                self.head_ready(&merged, newest, cutoff),
            )
        };
        let mut query = String::from(
            "update series_state set tail=?, head_count=?, head_start=?, head_end=?, max_seen_ts=?, version=version+1",
        );
        let mut arguments = vec![
            Value::Blob(packed),
            Value::Integer(count as i64),
            Value::Integer(first),
            Value::Integer(last),
            Value::Integer(newest),
        ];
        if i64::from(ready) != state.ready {
            query.push_str(", ready=?");
            arguments.push(Value::Integer(i64::from(ready)));
        }
        let due = state
            .next_gc
            .map_or(incoming[0].at, |old| old.min(incoming[0].at));
        if state.next_gc != Some(due) {
            query.push_str(", next_gc_ts=?");
            arguments.push(Value::Integer(due));
        }
        query.push_str(" where series_id=?");
        arguments.push(Value::Integer(id));
        connection
            .prepare_cached(&query)?
            .execute(params_from_iter(arguments))?;
        Ok(())
    }

    fn head_ready(&self, points: &[Sample], max_seen: i64, cutoff: i64) -> bool {
        let watermark = max_seen.saturating_sub(self.options.lateness);
        points
            .iter()
            .filter(|point| point.at >= cutoff)
            .take_while(|point| point.at < watermark)
            .take(240)
            .count()
            == 240
    }
}

fn reusable_chunks(chunks: &[HeadChunk], before: i64) -> usize {
    chunks
        .iter()
        .take_while(|chunk| chunk.head.count == 240 && chunk.head.end < before)
        .count()
}

fn merge_head(existing: &[Sample], incoming: &[Sample], limit: usize) -> Result<Vec<Sample>> {
    let mut merged = Vec::with_capacity((existing.len() + incoming.len()).min(limit));
    let (mut a, mut b) = (0, 0);
    while a < existing.len() || b < incoming.len() {
        let point =
            if b == incoming.len() || (a < existing.len() && existing[a].at < incoming[b].at) {
                let point = existing[a];
                a += 1;
                point
            } else if a == existing.len() || incoming[b].at < existing[a].at {
                let point = incoming[b];
                b += 1;
                point
            } else {
                let point = incoming[b];
                a += 1;
                b += 1;
                point
            };
        if merged.len() == limit {
            return Err(failure(ErrorKind::Limit, "mutable samples in one series"));
        }
        merged.push(point);
    }
    Ok(merged)
}

#[derive(Clone, Debug, Default)]
pub struct Maintenance {
    pub sealed_blocks: usize,
    pub expired_samples: usize,
    pub conflicts: usize,
    pub quarantined_series: usize,
    pub reclaimed_series: usize,
}

struct Candidate {
    series_id: i64,
    version: i64,
    max_seen: i64,
    kind: String,
    model_scale: i32,
    points: Vec<Sample>,
}

struct Publication {
    candidate: Candidate,
    group: Group,
}

impl Engine {
    pub fn maintain(&self) -> Result<Maintenance> {
        let cutoff = self.cutoff();
        let mut result = Maintenance::default();
        let due = self.view(|connection| ids(connection,
            "select series_id from series_state where failed_at is null and next_gc_ts is not null and next_gc_ts<?
             order by next_gc_ts,series_id limit cast(? as integer)", params![cutoff, self.options.maintenance_series as i64]))?;
        for id in due {
            match self.expire_series(id, cutoff) {
                Ok((expired, reclaimed)) => {
                    result.expired_samples += expired;
                    result.reclaimed_series += usize::from(reclaimed);
                }
                Err(error) => {
                    result.quarantined_series += self.isolate(id, "retention", error)?;
                }
            }
        }
        let ready = self.ready_to_seal()?;
        let mut staged = Vec::new();
        let mut staged_bytes = 0;
        for id in ready {
            let mut candidate = match self.read_candidate(id, cutoff) {
                Ok(candidate) => candidate,
                Err(error) => {
                    result.quarantined_series += self.isolate(id, "read head", error)?;
                    continue;
                }
            };
            if candidate.points.len() < 240 {
                self.update(|connection| {
                    connection
                        .prepare_cached(
                            "update series_state set ready=0 where series_id=? and version=?",
                        )?
                        .execute(params![id, candidate.version])?;
                    Ok(())
                })?;
                continue;
            }
            let group = match codec::prepare_group(
                id,
                &candidate.points,
                i32::from(candidate.kind == "counter"),
                candidate.model_scale,
                self.options.max_block_span,
            ) {
                Ok(group) => group,
                Err(error) => {
                    result.quarantined_series +=
                        self.isolate(id, "encode block", failure(ErrorKind::Corrupt, error))?;
                    continue;
                }
            };
            let consumed: usize = group.blocks.iter().map(|block| block.head.count).sum();
            candidate.points.truncate(consumed);
            let bytes = candidate.points.len() * 16
                + group.clock_body.len()
                + group
                    .blocks
                    .iter()
                    .map(|block| block.clock.len() + block.body.len())
                    .sum::<usize>();
            if !staged.is_empty() && (staged.len() == 8 || staged_bytes + bytes > 1 << 20) {
                self.publish_batch(&mut staged, cutoff, &mut result)?;
                staged_bytes = 0;
            }
            staged.push(Publication { candidate, group });
            staged_bytes += bytes;
            if staged.len() == 8 || staged_bytes >= 1 << 20 {
                self.publish_batch(&mut staged, cutoff, &mut result)?;
                staged_bytes = 0;
            }
        }
        self.publish_batch(&mut staged, cutoff, &mut result)?;
        Ok(result)
    }

    fn ready_to_seal(&self) -> Result<Vec<i64>> {
        let cursor = self.ready_cursor.get();
        let ready = self.view(|connection| {
            let mut ready = ids(connection,
                "select series_id from series_state where failed_at is null and ready=1 and series_id>?
                 order by series_id limit cast(? as integer)", params![cursor, self.options.maintenance_series as i64])?;
            if ready.len() < self.options.maintenance_series && cursor > 0 {
                let remaining = self.options.maintenance_series - ready.len();
                ready.extend(ids(connection,
                    "select series_id from series_state where failed_at is null and ready=1 and series_id<=?
                     order by series_id limit cast(? as integer)", params![cursor, remaining as i64])?);
            }
            Ok(ready)
        })?;
        if let Some(id) = ready.last() {
            self.ready_cursor.set(*id);
        }
        Ok(ready)
    }

    fn read_candidate(&self, id: i64, cutoff: i64) -> Result<Candidate> {
        let (mut candidate, head) = self.view(|connection| {
            let (kind, version, max_seen, model_scale) = connection
                .prepare_cached(
                    "select s.kind, state.version, state.max_seen_ts, state.model_scale
                 from series s join series_state state on s.id = state.series_id where s.id = ?",
                )?
                .query_row([id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i32>(3)?,
                    ))
                })?;
            if !(-2..=15).contains(&model_scale) {
                return Err(failure(ErrorKind::Corrupt, "stored model hint"));
            }
            let watermark = max_seen.saturating_sub(self.options.lateness);
            let head = self.fetch_head(connection, id, cutoff, watermark)?;
            Ok((
                Candidate {
                    series_id: id,
                    version,
                    max_seen,
                    kind,
                    model_scale,
                    points: Vec::new(),
                },
                head,
            ))
        })?;
        let points = self.decode_head_state(id, &head)?;
        let watermark = candidate.max_seen.saturating_sub(self.options.lateness);
        for point in points {
            if point.at < cutoff {
                continue;
            }
            if point.at >= watermark || candidate.points.len() == 240 * 32 {
                break;
            }
            candidate.points.push(point);
        }
        candidate
            .points
            .truncate(candidate.points.len() / 240 * 240);
        Ok(candidate)
    }

    fn fetch_head(
        &self,
        connection: &Connection,
        id: i64,
        from: i64,
        to: i64,
    ) -> Result<HeadState> {
        let (count, start, end, size, packed) = connection
            .prepare_cached(
                "select head_count, head_start, head_end, coalesce(length(tail), 0),
             case when length(tail) <= ? and head_end >= ? and head_start < ? then tail end
             from series_state where series_id = ?",
            )?
            .query_row(
                params![self.options.max_head_bytes as i64, from, to, id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<Vec<u8>>>(4)?.unwrap_or_default(),
                    ))
                },
            )?;
        let mut state = HeadState {
            frontier: None,
            version: 0,
            max_seen: 0,
            ready: 0,
            next_gc: None,
            failed_at: None,
            failure: None,
            count: count.max(0) as usize,
            start,
            end,
            packed,
        };
        if count == 0 {
            if start.is_some() || end.is_some() || size != 0 {
                return Err(failure(ErrorKind::Corrupt, "empty mutable head"));
            }
            return Ok(state);
        }
        if start.is_none() || end.is_none() || end < start {
            return Err(failure(ErrorKind::Corrupt, "mutable endpoints"));
        }
        if end.unwrap() < from || start.unwrap() >= to {
            state.count = 0;
            state.packed.clear();
            return Ok(state);
        }
        if count < 0
            || count as usize > self.options.max_head_samples
            || size as usize > self.options.max_head_bytes
        {
            return Err(failure(ErrorKind::Limit, "mutable head capacity"));
        }
        if size == 0 || state.packed.len() != size as usize {
            return Err(failure(ErrorKind::Corrupt, "mutable body missing"));
        }
        Ok(state)
    }

    fn decode_head_state(&self, id: i64, state: &HeadState) -> Result<Vec<Sample>> {
        if state.count == 0 {
            return Ok(Vec::new());
        }
        codec::decode_head(
            id,
            state.count,
            state.start.unwrap(),
            state.end.unwrap(),
            &state.packed,
            self.options.max_head_samples,
            self.options.max_head_bytes,
        )
        .map_err(|error| failure(ErrorKind::Corrupt, error))
    }

    fn save_head(&self, connection: &Connection, id: i64, points: &[Sample]) -> Result<()> {
        let packed = codec::encode_head(
            id,
            points,
            self.options.max_head_samples,
            self.options.max_head_bytes,
        )
        .map_err(|error| failure(ErrorKind::Limit, error))?;
        let tail = if packed.is_empty() {
            None
        } else {
            Some(packed)
        };
        connection.prepare_cached("update series_state set tail=?, head_count=?, head_start=?, head_end=? where series_id=?")?
            .execute(params![tail, points.len() as i64, points.first().map(|point| point.at), points.last().map(|point| point.at), id])?;
        Ok(())
    }

    fn publish_batch(
        &self,
        staged: &mut Vec<Publication>,
        cutoff: i64,
        result: &mut Maintenance,
    ) -> Result<()> {
        if staged.is_empty() {
            return Ok(());
        }
        let outcome = self.update(|connection| {
            let mut outcome = Maintenance::default();
            for item in staged.iter_mut() {
                let sealed_blocks = item.group.blocks.len();
                connection.execute_batch("savepoint maintenance_series")?;
                match self.publish(connection, &item.candidate, &mut item.group, cutoff) {
                    Ok(()) => {
                        connection.execute_batch("release maintenance_series")?;
                        outcome.sealed_blocks += sealed_blocks;
                    }
                    Err(error) => {
                        connection.execute_batch(
                            "rollback to maintenance_series; release maintenance_series",
                        )?;
                        match error_kind(error.as_ref()) {
                            Some(ErrorKind::Conflict) => outcome.conflicts += 1,
                            Some(ErrorKind::Corrupt) => {
                                outcome.quarantined_series += self.suspend(
                                    connection,
                                    item.candidate.series_id,
                                    "publish block",
                                    error.as_ref(),
                                )?
                            }
                            _ => return Err(error),
                        }
                    }
                }
            }
            Ok(outcome)
        })?;
        result.sealed_blocks += outcome.sealed_blocks;
        result.conflicts += outcome.conflicts;
        result.quarantined_series += outcome.quarantined_series;
        staged.clear();
        Ok(())
    }

    fn publish(
        &self,
        connection: &Connection,
        candidate: &Candidate,
        group: &mut Group,
        cutoff: i64,
    ) -> Result<()> {
        let version: i64 = connection
            .prepare_cached("select version from series_state where series_id=?")?
            .query_row([candidate.series_id], |row| row.get(0))?;
        if version != candidate.version {
            return Err(failure(ErrorKind::Conflict, "packing version"));
        }
        if version == i64::MAX {
            return Err(failure(ErrorKind::Limit, "series version exhausted"));
        }
        let replaced = self.merge_preceding(connection, group)?;
        group.clock_id = acquire_clock(connection, &group.clock_body)?;
        store_payloads(connection, group)?;
        let directory =
            codec::write_directory(group).map_err(|error| failure(ErrorKind::Limit, error))?;
        for previous in replaced {
            connection
                .prepare_cached("delete from groups where series_id=? and start_ts=?")?
                .execute(params![previous.series_id, previous.start])?;
            release_clock(connection, previous.clock_id)?;
        }
        connection
            .prepare_cached("insert into groups values(?,?,?,?,?)")?
            .execute(params![
                group.series_id,
                group.start,
                group.end,
                directory,
                group.clock_id
            ])?;
        let head = self.fetch_head(connection, group.series_id, i64::MIN, i64::MAX)?;
        let existing = self.decode_head_state(group.series_id, &head)?;
        let remaining = remove_sealed(&existing, &candidate.points)?;
        self.save_head(connection, group.series_id, &remaining)?;
        connection.prepare_cached(
            "update series_state set sealed_before=?,version=version+1,ready=?,model_scale=? where series_id=?")?
            .execute(params![group.end + 1, i64::from(self.head_ready(&remaining, candidate.max_seen, cutoff)), group.model_scale, group.series_id])?;
        self.refresh_due(connection, group.series_id)?;
        Ok(())
    }

    fn isolate(&self, id: i64, phase: &str, error: Box<dyn std::error::Error>) -> Result<usize> {
        match error_kind(error.as_ref()) {
            Some(ErrorKind::Corrupt | ErrorKind::Limit) => {
                self.update(|connection| self.suspend(connection, id, phase, error.as_ref()))
            }
            _ => Err(error),
        }
    }

    fn suspend(
        &self,
        connection: &Connection,
        id: i64,
        phase: &str,
        error: &dyn std::error::Error,
    ) -> Result<usize> {
        let mut reason = format!("{phase}: {error}");
        if reason.len() > 1024 {
            let mut end = 1024;
            while !reason.is_char_boundary(end) {
                end -= 1;
            }
            reason.truncate(end);
        }
        Ok(connection.prepare_cached("update series_state set failed_at=?,failure_reason=? where series_id=? and failed_at is null")?
            .execute(params![self.now, reason, id])?)
    }
}

fn ids(connection: &Connection, query: &str, arguments: impl rusqlite::Params) -> Result<Vec<i64>> {
    let mut statement = connection.prepare_cached(query)?;
    Ok(statement
        .query_map(arguments, |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn remove_sealed(existing: &[Sample], sealed: &[Sample]) -> Result<Vec<Sample>> {
    let mut out = Vec::with_capacity(existing.len());
    let mut removed = 0;
    for &point in existing {
        if removed < sealed.len() && point.at == sealed[removed].at {
            if point.value.to_bits() != sealed[removed].value.to_bits() {
                return Err(failure(ErrorKind::Conflict, "changed sealed sample"));
            }
            removed += 1;
        } else {
            out.push(point);
        }
    }
    if removed != sealed.len() {
        return Err(failure(ErrorKind::Conflict, "missing sealed sample"));
    }
    Ok(out)
}

fn acquire_clock(connection: &Connection, body: &[u8]) -> Result<i64> {
    let digest = Sha256::digest(body);
    let saved: Option<(i64, Vec<u8>)> = connection
        .prepare_cached("select id,body from clocks where digest=?")?
        .query_row([digest.as_slice()], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    if let Some((id, saved)) = saved {
        if saved != body {
            return Err(failure(ErrorKind::Corrupt, "clock digest collision"));
        }
        connection
            .prepare_cached("update clocks set refs=refs+1 where id=?")?
            .execute([id])?;
        return Ok(id);
    }
    connection
        .prepare_cached("insert into clocks(digest,refs,body) values(?,1,?)")?
        .execute(params![digest.as_slice(), body])?;
    Ok(connection.last_insert_rowid())
}

fn release_clock(connection: &Connection, id: i64) -> Result<()> {
    let changed = connection
        .prepare_cached("update clocks set refs=refs-1 where id=? and refs>0")?
        .execute([id])?;
    if changed != 1 {
        return Err(failure(ErrorKind::Corrupt, "missing clock owner"));
    }
    connection
        .prepare_cached("delete from clocks where id=? and refs=0")?
        .execute([id])?;
    Ok(())
}

fn store_payloads(connection: &Connection, group: &mut Group) -> Result<()> {
    let allocated = group
        .blocks
        .iter()
        .enumerate()
        .filter(|(slot, block)| group.is_external(*slot) && block.payload == 0)
        .count() as i64;
    if allocated == 0 {
        return Ok(());
    }
    let mut next: i64 = connection
        .prepare_cached("select next_payload_id from store_state where id=1")?
        .query_row([], |row| row.get(0))?;
    if next > i64::MAX - allocated {
        return Err(failure(ErrorKind::Limit, "payload identifiers exhausted"));
    }
    connection
        .prepare_cached("update store_state set next_payload_id=? where id=1")?
        .execute([next + allocated])?;
    for slot in 0..group.blocks.len() {
        if !group.is_external(slot) || group.blocks[slot].payload != 0 {
            continue;
        }
        group.blocks[slot].payload = next;
        next += 1;
        connection
            .prepare_cached("insert into payloads values(?,?)")?
            .execute(params![
                group.blocks[slot].payload,
                &group.blocks[slot].body
            ])?;
    }
    Ok(())
}

impl Engine {
    fn load_group(
        &self,
        connection: &Connection,
        id: i64,
        start: i64,
        end: i64,
        clock_id: i64,
        directory: &[u8],
    ) -> Result<Group> {
        let saved: Option<(i64, Option<Vec<u8>>)> = connection.prepare_cached(
            "select length(body),case when length(body)<=? then body else null end from clocks where id=?")?
            .query_row(params![65536, clock_id], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
        let (size, body) =
            saved.ok_or_else(|| failure(ErrorKind::Corrupt, "shared clock missing"))?;
        if size > 65536 {
            return Err(failure(ErrorKind::Corrupt, "shared clock size"));
        }
        let body = body.ok_or_else(|| failure(ErrorKind::Corrupt, "shared clock body missing"))?;
        let blocks =
            codec::decode_clock_group(&body).map_err(|error| failure(ErrorKind::Corrupt, error))?;
        let mut group = codec::read_directory(id, start, end, clock_id, directory, &blocks)
            .map_err(|error| failure(ErrorKind::Corrupt, error))?;
        group.clock_body = body;
        Ok(group)
    }

    fn first_group(&self, connection: &Connection, id: i64) -> Result<Option<Group>> {
        let saved: Option<(i64, i64, Vec<u8>, i64)> = connection.prepare_cached(
            "select start_ts,end_ts,directory,clock_id from groups where series_id=? order by start_ts limit 1")?
            .query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).optional()?;
        saved
            .map(|(start, end, directory, clock_id)| {
                self.load_group(connection, id, start, end, clock_id, &directory)
            })
            .transpose()
    }

    fn merge_preceding(&self, connection: &Connection, group: &mut Group) -> Result<Vec<Group>> {
        let mut replaced = Vec::new();
        let mut before = group.start;
        while group.blocks.len() < 32 {
            let saved: Option<(i64, i64, i64, Vec<u8>)> = connection.prepare_cached(
                "select start_ts,end_ts,clock_id,directory from groups where series_id=? and start_ts<? order by start_ts desc limit 1")?
                .query_row(params![group.series_id, before], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).optional()?;
            let Some((start, end, clock_id, directory)) = saved else {
                break;
            };
            let previous = self.load_group(
                connection,
                group.series_id,
                start,
                end,
                clock_id,
                &directory,
            )?;
            let count = previous.live.count_ones() as usize;
            if count > group.blocks.len() || count + group.blocks.len() > 32 {
                break;
            }
            if previous.end >= group.start {
                return Err(failure(
                    ErrorKind::Corrupt,
                    "overlapping groups during merge",
                ));
            }
            let mut merged = group.clone();
            let mut blocks = Vec::with_capacity(count + group.blocks.len());
            for (slot, block) in previous.blocks.iter().enumerate() {
                if previous.is_live(slot) {
                    blocks.push(block.clone());
                }
            }
            blocks.extend(group.blocks.iter().cloned());
            merged.blocks = blocks;
            merged.start = merged.blocks[0].head.start;
            merged.clock_body = codec::encode_clock_group(&merged);
            if merged.clock_body.len() > 65536 {
                break;
            }
            merged.live = codec::slots_mask(merged.blocks.len());
            merged.allocation = 0;
            for (slot, block) in merged.blocks.iter().enumerate() {
                if block.body_bytes > 16 {
                    merged.allocation |= 1 << slot;
                }
            }
            if codec::expanded_directory_size(&merged) > 8192 - 128 {
                break;
            }
            before = previous.start;
            replaced.push(previous);
            *group = merged;
        }
        Ok(replaced)
    }

    fn refresh_due(&self, connection: &Connection, id: i64) -> Result<()> {
        let mut next: Option<i64> = connection
            .prepare_cached("select head_start from series_state where series_id=?")?
            .query_row([id], |row| row.get(0))?;
        if let Some(group) = self.first_group(connection, id)? {
            for (slot, block) in group.blocks.iter().enumerate() {
                if group.is_live(slot) {
                    next = Some(next.map_or(block.head.end, |old| old.min(block.head.end)));
                    break;
                }
            }
        }
        connection
            .prepare_cached("update series_state set next_gc_ts=? where series_id=?")?
            .execute(params![next, id])?;
        Ok(())
    }

    fn expire_series(&self, id: i64, cutoff: i64) -> Result<(usize, bool)> {
        self.update(|connection| {
            let (version, max_seen): (i64, i64) = connection
                .prepare_cached("select version,max_seen_ts from series_state where series_id=?")?
                .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))?;
            if version == i64::MAX {
                return Err(failure(ErrorKind::Limit, "series version exhausted"));
            }
            let head = self.fetch_head(connection, id, i64::MIN, i64::MAX)?;
            let points = self.decode_head_state(id, &head)?;
            let prefix = points.iter().take_while(|point| point.at < cutoff).count();
            let kept = &points[prefix..];
            if prefix > 0 {
                self.save_head(connection, id, kept)?;
            }
            let expired = prefix + self.expire_blocks(connection, id, cutoff)?;
            if kept.is_empty() {
                let has_groups: bool = connection
                    .prepare_cached("select exists(select 1 from groups where series_id=?)")?
                    .query_row([id], |row| row.get(0))?;
                if !has_groups {
                    self.reclaim_series(connection, id)?;
                    return Ok((expired, true));
                }
            }
            if expired > 0 {
                connection
                    .prepare_cached(
                        "update series_state set version=version+1,ready=? where series_id=?",
                    )?
                    .execute(params![
                        i64::from(self.head_ready(kept, max_seen, cutoff)),
                        id
                    ])?;
            }
            self.refresh_due(connection, id)?;
            Ok((expired, false))
        })
    }

    fn expire_blocks(&self, connection: &Connection, id: i64, cutoff: i64) -> Result<usize> {
        let Some(mut group) = self.first_group(connection, id)? else {
            return Ok(0);
        };
        let mut expired = 0;
        for slot in 0..group.blocks.len() {
            if !group.is_live(slot) {
                continue;
            }
            if group.blocks[slot].head.end >= cutoff {
                break;
            }
            if group.is_external(slot) {
                let removed = connection
                    .prepare_cached("delete from payloads where id=?")?
                    .execute([group.blocks[slot].payload])?;
                if removed != 1 {
                    return Err(failure(ErrorKind::Corrupt, "expired payload missing"));
                }
            }
            group.live &= !(1 << slot);
            expired += group.blocks[slot].head.count;
        }
        if expired == 0 {
            return Ok(0);
        }
        if group.live == 0 {
            connection
                .prepare_cached("delete from groups where series_id=? and start_ts=?")?
                .execute(params![id, group.start])?;
            release_clock(connection, group.clock_id)?;
        } else {
            let directory =
                codec::write_directory(&group).map_err(|error| failure(ErrorKind::Limit, error))?;
            connection
                .prepare_cached("update groups set directory=? where series_id=? and start_ts=?")?
                .execute(params![directory, id, group.start])?;
        }
        Ok(expired)
    }

    fn reclaim_series(&self, connection: &Connection, id: i64) -> Result<()> {
        let mut statement = connection.prepare_cached(
            "select p.label_id,v.posting_count from postings p join label_values v on v.id=p.label_id where p.series_id=? order by p.label_id")?;
        let postings = statement
            .query_map([id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if postings.is_empty() || postings.iter().any(|(_, count)| *count < 1) {
            return Err(failure(
                ErrorKind::Corrupt,
                "posting count before reclamation",
            ));
        }
        drop(statement);
        let changed = connection.prepare_cached(
            "update label_values set posting_count=posting_count-1 where id in (select label_id from postings where series_id=?)")?.execute([id])?;
        if changed != postings.len() {
            return Err(failure(ErrorKind::Corrupt, "posting count update"));
        }
        connection
            .prepare_cached("delete from postings where series_id=?")?
            .execute([id])?;
        connection
            .prepare_cached("delete from series_state where series_id=?")?
            .execute([id])?;
        connection
            .prepare_cached("delete from series where id=?")?
            .execute([id])?;
        let label_ids: Vec<i64> = postings.iter().map(|(label_id, _)| *label_id).collect();
        let encoded = serde_json::to_string(&label_ids)?;
        connection.prepare_cached(
            "delete from label_values where posting_count=0 and id in (select cast(value as integer) from json_each(?))")?.execute([encoded])?;
        connection
            .prepare_cached("update store_state set series_count=series_count-1 where id=1")?
            .execute([])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_keep_go_json_escaping_and_identifier_order() {
        let series = Series {
            name: "metric<&>\u{2028}".into(),
            kind: "gauge".into(),
            labels: BTreeMap::from([("z".into(), "last".into()), ("a".into(), "first".into())]),
        };
        let (labels, identity) = canonical_labels(&series).unwrap();
        assert_eq!(
            labels
                .iter()
                .map(|pair| pair.name.as_str())
                .collect::<Vec<_>>(),
            ["__name__", "a", "z"]
        );
        assert_eq!(
            identity,
            "[[\"__name__\",\"metric\\u003c\\u0026\\u003e\\u2028\"],[\"a\",\"first\"],[\"z\",\"last\"]]"
        );
        assert_eq!(
            decode_label_ids(&encode_label_ids(&[1, 128, 1024])).unwrap(),
            [1, 128, 1024]
        );
        for data in [&[][..], &[0], &[1, 0], &[1, 1, 0], &[1, 0x80]] {
            assert!(decode_label_ids(data).is_err(), "{data:?}");
        }
    }

    #[test]
    fn pending_series_preserves_last_bits_after_order_breaks() {
        let mut pending = PendingSeries {
            batch: PreparedBatch {
                identity: String::new(),
                labels: Vec::new(),
                kind: "gauge".into(),
                samples: Vec::new(),
            },
            by_time: None,
        };
        for sample in [
            Sample { at: 10, value: 0.0 },
            Sample { at: 12, value: 1.0 },
            Sample {
                at: 11,
                value: f64::from_bits(0x7ff8000000000042),
            },
            Sample {
                at: 10,
                value: -0.0,
            },
        ] {
            pending.add(sample);
        }
        let samples = pending.sorted().samples;
        assert_eq!(
            samples.iter().map(|sample| sample.at).collect::<Vec<_>>(),
            [10, 11, 12]
        );
        assert_eq!(samples[0].value.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(samples[1].value.to_bits(), 0x7ff8000000000042);
    }

    #[test]
    fn merge_and_publication_require_exact_sealed_bits() {
        let existing = [Sample { at: 10, value: 0.0 }, Sample { at: 11, value: 1.0 }];
        let incoming = [
            Sample {
                at: 10,
                value: -0.0,
            },
            Sample { at: 12, value: 2.0 },
        ];
        let merged = merge_head(&existing, &incoming, 3).unwrap();
        assert_eq!(merged[0].value.to_bits(), (-0.0_f64).to_bits());
        assert!(merge_head(&existing, &incoming, 2).is_err());
        assert!(remove_sealed(&existing, &incoming[..1]).is_err());
        assert_eq!(remove_sealed(&merged, &incoming[..1]).unwrap().len(), 2);
    }
}
