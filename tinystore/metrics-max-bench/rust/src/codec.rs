//! Research port of tinyshed/tinystore e307c48a40126aad0e2873b6bf3aaedef8115483:
//! codec/*.go and metrics head, clock, values, residuals, directory and summaries.
//! Native libzstd compression and Huffman candidates preserve Go's layouts;
//! their chosen compressed bytes can differ from klauspost/compress.
use crate::engine::Sample;
use std::cell::RefCell;
use std::ops::{Deref, Range};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

static OPTIMIZED: AtomicBool = AtomicBool::new(false);

/// Select once before running queries or starting workers. The baseline remains
/// available so allocation changes can be measured apart from parallelism.
pub fn set_optimized(enabled: bool) {
    OPTIMIZED.store(enabled, Ordering::Relaxed);
}

pub fn optimized() -> bool {
    OPTIMIZED.load(Ordering::Relaxed)
}

#[path = "codec/binary.rs"]
mod binary;
#[path = "codec/clocks.rs"]
mod clocks;
#[path = "codec/directory.rs"]
mod directory;
#[path = "codec/envelope.rs"]
mod envelope;
#[path = "codec/huffman.rs"]
pub(crate) mod huffman;
#[path = "codec/residuals.rs"]
pub(crate) mod residuals;
#[path = "codec/sealed.rs"]
mod sealed;
#[path = "codec/summaries.rs"]
mod summaries;
#[path = "codec/values.rs"]
mod values;

pub type Result<T> = std::result::Result<T, String>;
pub const BLOCK_SAMPLES: usize = 240;
pub const GROUP_SLOTS: usize = 32;
pub const INLINE_BYTES: usize = 16;
pub const MAX_DIRECTORY_BYTES: usize = 8192;
pub const MAX_PAYLOAD_BYTES: usize = 8200;

#[derive(Clone, Copy, Debug, Default)]
pub struct Head {
    pub start: i64,
    pub end: i64,
    pub count: usize,
    pub first: f64,
}

#[derive(Clone, Debug)]
pub struct HeadChunk {
    pub head: Head,
    pub body: HeadBytes,
    pub stored: HeadBytes,
}

/// Bytes in a mutable chunk can share the validated head allocation. The
/// baseline keeps its independent owned byte vectors for flag comparisons.
#[derive(Clone, Debug)]
pub enum HeadBytes {
    Owned(Vec<u8>),
    Shared {
        bytes: Arc<[u8]>,
        range: Range<usize>,
    },
}

impl Deref for HeadBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared { bytes, range } => &bytes[range.clone()],
        }
    }
}
impl AsRef<[u8]> for HeadBytes {
    fn as_ref(&self) -> &[u8] {
        self
    }
}
impl From<Vec<u8>> for HeadBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Owned(bytes)
    }
}
impl PartialEq for HeadBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for HeadBytes {}

// A worker retains at most the default mutable-head sample bound. Unusually
// large heads use transient vectors, so a single read cannot bloat every worker.
const RETAINED_DECODE_SAMPLES: usize = 4096;
thread_local! {
    static DECODE_SCRATCH: RefCell<Vec<Sample>> = const { RefCell::new(Vec::new()) };
}

fn with_decode_scratch<T>(work: impl FnOnce(&mut Vec<Sample>) -> T) -> T {
    let mut points = DECODE_SCRATCH.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    points.clear();
    let result = work(&mut points);
    points.clear();
    if points.capacity() <= RETAINED_DECODE_SAMPLES {
        DECODE_SCRATCH.with(|slot| *slot.borrow_mut() = points);
    }
    result
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub last: f64,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub increase: f64,
    pub resets: u16,
    pub valid: bool,
    pub exact_sum: Vec<u8>,
    pub exact_increase: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct Block {
    pub payload: i64,
    pub clock: Vec<u8>,
    pub head: Head,
    pub summary: Summary,
    pub body: Vec<u8>,
    pub shared_body: Option<(Arc<Vec<u8>>, Range<usize>)>,
    pub body_bytes: usize,
}

impl Block {
    pub fn value_body(&self) -> &[u8] {
        match &self.shared_body {
            Some((bytes, range)) => &bytes[range.clone()],
            None => &self.body,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Group {
    pub clock_id: i64,
    pub clock_body: Vec<u8>,
    pub model_scale: i32,
    pub series_id: i64,
    pub start: i64,
    pub end: i64,
    pub live: u32,
    pub allocation: u32,
    pub blocks: Vec<Block>,
}

impl Group {
    pub fn is_live(&self, slot: usize) -> bool {
        self.live & (1u32 << slot) != 0
    }
    pub fn is_external(&self, slot: usize) -> bool {
        self.allocation & (1u32 << slot) != 0
    }
}

pub fn slots_mask(count: usize) -> u32 {
    if count == 32 {
        u32::MAX
    } else {
        (1u32 << count) - 1
    }
}

pub fn distance(start: i64, end: i64) -> u64 {
    (end as u64).wrapping_sub(start as u64)
}
pub fn advance(start: i64, span: u64) -> i64 {
    (start as u64).wrapping_add(span) as i64
}

pub use sealed::{
    decode_block, decode_block_into, decode_clock_group, encode_clock_group,
    expanded_directory_size, prepare_group, read_directory, visit_block, write_directory,
};
pub use summaries::{encode_exact, finite_units, read_exact_value};

fn checksum(parts: &[&[u8]]) -> u32 {
    let mut crc = crc32fast::Hasher::new();
    for part in parts {
        crc.update(part);
    }
    crc.finalize()
}

pub fn parse_head(
    id: i64,
    count: usize,
    start: i64,
    end: i64,
    packed: &[u8],
    max_samples: usize,
    max_bytes: usize,
) -> Result<Vec<HeadChunk>> {
    let _scope = crate::profile::scope("parse_head");
    parse_head_storage(
        id,
        count,
        start,
        end,
        packed,
        max_samples,
        max_bytes,
        crate::tuning::buffers(),
    )
}

fn parse_head_storage(
    id: i64,
    count: usize,
    start: i64,
    end: i64,
    packed: &[u8],
    max_samples: usize,
    max_bytes: usize,
    shared: bool,
) -> Result<Vec<HeadChunk>> {
    use binary::Reader;
    if packed.len() < 6 || packed.len() > max_bytes || count > max_samples {
        return Err("mutable head size".into());
    }
    let content = &packed[..packed.len() - 4];
    if checksum(&[&id.to_le_bytes(), content])
        != u32::from_le_bytes(packed[packed.len() - 4..].try_into().unwrap())
    {
        return Err("mutable head checksum".into());
    }
    let mut r = Reader::new(content);
    if r.byte()? != 1 || r.size(max_samples)? != count || count == 0 {
        return Err("mutable head version/count".into());
    }
    let shared = shared.then(|| Arc::<[u8]>::from(content));
    let mut chunks: Vec<HeadChunk> = Vec::with_capacity((count + 239) / 240);
    let mut parsed = 0;
    while parsed < count {
        let before = r.position();
        let chunk_start = binary::unfold(r.unsigned()?);
        let span = r.unsigned()?;
        let chunk_count = r.size(BLOCK_SAMPLES.min(count - parsed))?;
        let first = f64::from_bits(r.word()?);
        let length = r.size(MAX_PAYLOAD_BYTES)?;
        let body_start = r.position();
        let body_bytes = r.take(length)?;
        if chunk_count == 0
            || length < 8
            || (chunk_count == 1 && span != 0)
            || (chunk_count > 1 && span == 0)
            || span > distance(chunk_start, i64::MAX)
        {
            return Err("mutable chunk extent".into());
        }
        let chunk_end = advance(chunk_start, span);
        if chunk_end == i64::MAX
            || chunks
                .last()
                .is_some_and(|prev| chunk_start <= prev.head.end)
        {
            return Err("mutable chunk ordering".into());
        }
        chunks.push(HeadChunk {
            head: Head {
                start: chunk_start,
                end: chunk_end,
                count: chunk_count,
                first,
            },
            body: match &shared {
                Some(bytes) => HeadBytes::Shared {
                    bytes: Arc::clone(bytes),
                    range: body_start..r.position(),
                },
                None => HeadBytes::Owned(body_bytes.to_vec()),
            },
            stored: match &shared {
                Some(bytes) => HeadBytes::Shared {
                    bytes: Arc::clone(bytes),
                    range: before..r.position(),
                },
                None => HeadBytes::Owned(content[before..r.position()].to_vec()),
            },
        });
        parsed += chunk_count;
    }
    r.finish()?;
    if chunks[0].head.start != start || chunks.last().unwrap().head.end != end {
        return Err("mutable head endpoints".into());
    }
    Ok(chunks)
}

pub fn decode_chunks(chunks: &[HeadChunk], from: i64, to: i64) -> Result<Vec<Sample>> {
    let count = chunks
        .iter()
        .filter(|c| c.head.end >= from && c.head.start < to)
        .map(|c| c.head.count)
        .sum();
    let mut out = Vec::with_capacity(count);
    if crate::tuning::buffers() {
        decode_chunks_into(chunks, from, to, &mut out)?;
        return Ok(out);
    }
    for chunk in chunks {
        if chunk.head.end >= from && chunk.head.start < to {
            out.extend(envelope::decode(chunk.head, &chunk.body)?);
        }
    }
    Ok(out)
}

/// Append selected chunks without allocating a vector for each chunk. Failed
/// decodes restore the caller's original length; partial samples never escape.
pub fn decode_chunks_into(
    chunks: &[HeadChunk],
    from: i64,
    to: i64,
    out: &mut Vec<Sample>,
) -> Result<()> {
    let before = out.len();
    let result = (|| {
        for chunk in chunks {
            if chunk.head.end >= from && chunk.head.start < to {
                envelope::decode_into(chunk.head, &chunk.body, out)?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        out.truncate(before);
    }
    result
}

/// All selected mutable chunks are validated before the first consumer call.
/// This preserves corruption precedence when a later chunk is malformed.
pub fn visit_chunks(
    chunks: &[HeadChunk],
    from: i64,
    to: i64,
    mut visitor: impl FnMut(Sample) -> Result<()>,
) -> Result<()> {
    with_decode_scratch(|points| {
        let count = chunks
            .iter()
            .filter(|chunk| chunk.head.end >= from && chunk.head.start < to)
            .map(|chunk| chunk.head.count)
            .sum::<usize>();
        points.reserve(count);
        decode_chunks_into(chunks, from, to, points)?;
        for &point in points.iter() {
            visitor(point)?;
        }
        Ok(())
    })
}

pub fn decode_head(
    id: i64,
    count: usize,
    start: i64,
    end: i64,
    packed: &[u8],
    max_samples: usize,
    max_bytes: usize,
) -> Result<Vec<Sample>> {
    if packed.is_empty() {
        return if count == 0 {
            Ok(Vec::new())
        } else {
            Err("mutable head sample count".into())
        };
    }
    decode_chunks(
        &parse_head(id, count, start, end, packed, max_samples, max_bytes)?,
        i64::MIN,
        i64::MAX,
    )
}

pub fn encode_head_after(
    id: i64,
    kept: &[HeadChunk],
    points: &[Sample],
    max_samples: usize,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let total = kept.iter().map(|c| c.head.count).sum::<usize>() + points.len();
    if total == 0 {
        return Ok(Vec::new());
    }
    if total > max_samples {
        return Err("mutable samples in one series".into());
    }
    let mut out = vec![1];
    binary::put_unsigned(&mut out, total as u64);
    for chunk in kept {
        out.extend_from_slice(&chunk.stored);
    }
    for chunk in points.chunks(BLOCK_SAMPLES) {
        let (head, body) = envelope::encode(chunk)?;
        binary::put_unsigned(&mut out, binary::fold(head.start));
        binary::put_unsigned(&mut out, distance(head.start, head.end));
        binary::put_unsigned(&mut out, head.count as u64);
        out.extend_from_slice(&head.first.to_bits().to_le_bytes());
        binary::put_unsigned(&mut out, body.len() as u64);
        out.extend_from_slice(&body);
        if out.len() + 4 > max_bytes {
            return Err("mutable head bytes".into());
        }
    }
    if out.len() + 4 > max_bytes {
        return Err("mutable head bytes".into());
    }
    let crc = checksum(&[&id.to_le_bytes(), &out]);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(out)
}

pub fn encode_head(
    id: i64,
    points: &[Sample],
    max_samples: usize,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    encode_head_after(id, &[], points, max_samples, max_bytes)
}

#[cfg(test)]
#[path = "codec/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "codec/buffer_tests.rs"]
mod buffer_tests;
