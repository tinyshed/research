// Owning backing buffers and lazy consumed-column decoding for the restricted
// existing v1 representations. The previous reader remains in the parent.
use super::*;
use std::{
    ops::Range,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug)]
struct Text {
    buffer: u32,
    start: u32,
    end: u32,
}
#[derive(Clone, Copy, Debug)]
struct FieldSpan {
    key: Text,
    value: Text,
}
#[derive(Clone, Copy, Debug)]
enum Fields {
    Empty,
    Inline { start: u32, end: u32 },
    Context(usize),
}
struct Storage {
    buffers: Vec<Arc<[u8]>>,
    fields: Vec<FieldSpan>,
}
impl Storage {
    fn new() -> Self {
        Self {
            buffers: vec![],
            fields: vec![],
        }
    }
    fn buffer(&mut self, b: Vec<u8>) -> usize {
        let id = self.buffers.len();
        self.buffers.push(b.into());
        id
    }
    fn text(&self, t: Text) -> &str {
        std::str::from_utf8(&self.buffers[t.buffer as usize][t.start as usize..t.end as usize])
            .expect("validated UTF-8")
    }
    fn span(&self, id: usize, range: Range<usize>, text: bool) -> Result<Text> {
        if text {
            std::str::from_utf8(&self.buffers[id][range.clone()])?;
        };
        Ok(Text {
            buffer: id as u32,
            start: range.start as u32,
            end: range.end as u32,
        })
    }
    fn parse_fields(&mut self, id: usize, range: Range<usize>) -> Result<Fields> {
        let body = Arc::clone(&self.buffers[id]);
        let mut c = Cursor {
            data: &body[range.clone()],
        };
        let n = c.count(128)?;
        let start = self.fields.len();
        for _ in 0..n {
            let key = read_span(&mut c, id, range.end, true)?;
            let value = read_span(&mut c, id, range.end, true)?;
            self.fields.push(FieldSpan { key, value })
        }
        c.finish()?;
        Ok(if n == 0 {
            Fields::Empty
        } else {
            Fields::Inline {
                start: start as u32,
                end: self.fields.len() as u32,
            }
        })
    }
}
// A cursor points into the suffix of its owning backing buffer. Absolute
// offsets are computed before the checked take; no SQLite address escapes.
fn read_span(c: &mut Cursor, id: usize, whole: usize, text: bool) -> Result<Text> {
    let n = c.count(MAX_BLOCK)?;
    let start = whole - c.data.len();
    let b = c.take(n)?;
    if text {
        std::str::from_utf8(b)?;
    };
    Ok(Text {
        buffer: id as u32,
        start: start as u32,
        end: (start + n) as u32,
    })
}
struct Expansion {
    used: usize,
    limit: usize,
}
impl Expansion {
    fn charge(&mut self, n: usize) -> Result<()> {
        if n > self.limit - self.used {
            return Err("cumulative decoded expansion over bound".into());
        };
        self.used += n;
        Ok(())
    }
}
// Fixed-width extraction uses little-endian words at wider widths; narrow
// columns retain the baseline byte extraction. No native/endian unsafe loads.
fn read_numbers(c: &mut Cursor, n: usize) -> Result<Vec<i64>> {
    if c.byte()? != 0 {
        return Err("unsupported integer layout".into());
    };
    let transform = c.byte()?;
    if transform > 2 {
        return Err("integer transform".into());
    };
    let first = if transform != 0 { c.vi()? } else { 0 };
    let step = if transform == 2 { c.vi()? } else { 0 };
    let base = c.vi()?;
    let divisor = c.uv()?;
    if divisor == 0 || transform == 1 && n == 0 {
        return Err("integer divisor or delta count".into());
    };
    if c.byte()? != 0 {
        return Err("unsupported integer packer".into());
    };
    let width = c.count(64)? as u32;
    let m = n - usize::from(transform == 1);
    let b = c.take((m * width as usize + 7) / 8)?;
    let mut vs = Vec::with_capacity(n);
    if transform == 1 {
        vs.push(first)
    };
    let mut at = 0usize;
    for i in 0..m {
        let mut v = 0u64;
        let mut got = 0u32;
        while got < width {
            let index = at / 8;
            let shift = (at % 8) as u32;
            let available = (b.len() - index).min(8);
            let mut word = [0u8; 8];
            word[..available].copy_from_slice(&b[index..index + available]);
            let take = (width - got).min(available as u32 * 8 - shift);
            let part = u64::from_le_bytes(word) >> shift;
            v |= if take == 64 {
                part
            } else {
                part & ((1u64 << take) - 1)
            } << got;
            got += take;
            at += take as usize
        }
        let x = (base as u64).wrapping_add(v.wrapping_mul(divisor)) as i64;
        vs.push(match transform {
            1 => vs[i].wrapping_add(x),
            2 => first
                .wrapping_add(step.wrapping_mul(i as i64))
                .wrapping_add(x),
            _ => x,
        })
    }
    Ok(vs)
}
fn raw_text(
    c: &mut Cursor,
    n: usize,
    store: &mut Storage,
    budget: &mut Expansion,
    text: bool,
) -> Result<Vec<Text>> {
    if c.byte()? != 0 || c.byte()? != 0 {
        return Err("unsupported text layout".into());
    };
    let lengths = read_numbers(c, n)?;
    let mut total = 0usize;
    for &l in &lengths {
        if l < 0 || l as usize > (4 << 20) - total {
            return Err("text expansion".into());
        };
        total += l as usize
    }
    budget.charge(total)?;
    let raw = match c.byte()? {
        0 => c.take(total)?.to_vec(),
        1 => {
            let n = c.count(4 << 20)?;
            let raw = decompress(c.take(n)?, total)?;
            if raw.len() != total {
                return Err("blob length".into());
            };
            raw
        }
        _ => return Err("blob kind".into()),
    };
    let id = store.buffer(raw);
    let mut at = 0;
    lengths
        .into_iter()
        .map(|n| {
            let span = store.span(id, at..at + n as usize, text);
            at += n as usize;
            span
        })
        .collect()
}
fn value_text(
    c: &mut Cursor,
    n: usize,
    store: &mut Storage,
    budget: &mut Expansion,
) -> Result<Vec<Text>> {
    if c.byte()? != 0 {
        return Err("unsupported typed value".into());
    };
    raw_text(c, n, store, budget, true)
}

pub struct SharedSchema {
    schema: Schema,
}
impl SharedSchema {
    pub fn retained_bytes(&self) -> usize {
        let s = &self.schema;
        std::mem::size_of::<Self>()
            + s.stream.capacity()
            + s.names.capacity() * std::mem::size_of::<String>()
            + s.names.iter().map(String::capacity).sum::<usize>()
            + s.shapes.capacity()
            + s.contexts.capacity() * std::mem::size_of::<Vec<Field>>()
            + s.contexts
                .iter()
                .map(|fs| {
                    fs.capacity() * std::mem::size_of::<Field>()
                        + fs.iter()
                            .map(|f| f.key.capacity() + f.value.capacity() + 64)
                            .sum::<usize>()
                })
                .sum::<usize>()
            + 128
    }
}
pub fn schema(body: &[u8]) -> Result<Arc<SharedSchema>> {
    let mut c = Cursor {
        data: checked(body)?,
    };
    let mut budget = Expansion {
        used: 0,
        limit: 8 << 20,
    };
    let count = c.count(16384)?;
    let stream = c.string()?;
    budget.charge(stream.len())?;
    let names = c.strings(16384)?;
    budget.charge(names.iter().map(String::len).sum())?;
    let mut shapes = Vec::new();
    for _ in 0..c.count(128)? {
        let p = c.byte()?;
        let keys = c.strings(128)?;
        if p & !63 != 0 || p & 32 == 0 || !keys.is_empty() {
            return Err("unsupported typed shape".into());
        };
        shapes.push(p)
    }
    let n = c.count(16384)?;
    let mut contexts = vec![vec![]; n];
    if n > 0 {
        let lists = (0..c.count(n)?)
            .map(|_| c.strings(128))
            .collect::<Result<Vec<_>>>()?;
        let owners = read_numbers(&mut c, n)?;
        let mut cells = 0;
        for &id in &owners {
            let keys = lists.get(usize::try_from(id)?).ok_or("context reference")?;
            if keys.is_empty() {
                return Err("empty context keys".into());
            };
            cells += keys.len();
            if cells > 1 << 20 {
                return Err("context cells".into());
            };
            budget.charge(keys.iter().map(String::len).sum())?
        }
        let mut store = Storage::new();
        for (id, keys) in lists.iter().enumerate() {
            let members = owners
                .iter()
                .enumerate()
                .filter(|(_, v)| **v == id as i64)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            for key in keys {
                let vs = value_text(&mut c, members.len(), &mut store, &mut budget)?;
                for (&i, v) in members.iter().zip(vs) {
                    contexts[i].push(Field {
                        key: key.clone(),
                        value: store.text(v).into(),
                    })
                }
            }
        }
    };
    c.finish()?;
    if count == 0 || names.is_empty() || shapes.is_empty() {
        return Err("empty schema".into());
    };
    Ok(Arc::new(SharedSchema {
        schema: Schema {
            stream,
            count,
            names,
            shapes,
            contexts,
        },
    }))
}
enum Column {
    Ints(Vec<i64>),
    Text(Vec<Text>),
    Raw(Vec<Fields>),
    Ids(Vec<Text>),
}
struct Lazy {
    store: Storage,
    columns: HashMap<u8, Column>,
    budget: Expansion,
    complete: bool,
}
enum Format {
    Head {
        names: Vec<Text>,
        levels: Vec<Option<i32>>,
        bodies: Vec<Option<Text>>,
        traces: Vec<[u8; 16]>,
        spans: Vec<[u8; 8]>,
        contexts: Vec<Fields>,
        attrs: Vec<Fields>,
    },
    Block {
        schema: Arc<SharedSchema>,
        body: Arc<[u8]>,
        payloads: HashMap<u8, Range<usize>>,
        indexes: HashMap<u8, Vec<Option<usize>>>,
        names: Vec<i64>,
        shapes: Vec<i64>,
    },
}
pub struct Chunk {
    pub times: Vec<i64>,
    stream: Arc<str>,
    format: Format,
    lazy: Mutex<Lazy>,
}
impl Chunk {
    pub fn count(&self) -> usize {
        self.times.len()
    }
    pub fn at(&self, i: usize) -> i64 {
        self.times[i]
    }
    pub fn ensure(&self, kind: u8) -> Result<()> {
        let Format::Block {
            schema,
            body,
            payloads,
            indexes,
            ..
        } = &self.format
        else {
            return Ok(());
        };
        let Some(range) = payloads.get(&kind) else {
            return Ok(());
        };
        let mut lazy = self.lazy.lock().unwrap();
        if lazy.columns.contains_key(&kind) {
            return Ok(());
        };
        let n = indexes[&kind].iter().filter(|i| i.is_some()).count();
        let mut c = Cursor {
            data: &body[range.clone()],
        };
        if n == 0 {
            c.finish()?;
            return Ok(());
        };
        let Lazy {
            store,
            columns,
            budget,
            ..
        } = &mut *lazy;
        let column =
            match kind {
                3 | 4 => {
                    let vs = read_numbers(&mut c, n)?;
                    if kind == 3
                        && vs
                            .iter()
                            .any(|v| *v < 0 || *v >= schema.schema.contexts.len() as i64)
                    {
                        return Err("context reference".into());
                    };
                    if kind == 4 && vs.iter().any(|v| i32::try_from(*v).is_err()) {
                        return Err("level range".into());
                    };
                    Column::Ints(vs)
                }
                5 => Column::Text(value_text(&mut c, n, store, budget)?),
                6 | 7 | 8 => {
                    let vs = raw_text(&mut c, n, store, budget, false)?;
                    if kind == 8 {
                        let mut fields = Vec::with_capacity(n);
                        for t in vs {
                            fields.push(store.parse_fields(
                                t.buffer as usize,
                                t.start as usize..t.end as usize,
                            )?)
                        }
                        Column::Raw(fields)
                    } else {
                        if vs
                            .iter()
                            .any(|v| v.end - v.start != if kind == 6 { 16 } else { 8 })
                        {
                            return Err("ID width".into());
                        };
                        Column::Ids(vs)
                    }
                }
                _ => return Err("slot kind".into()),
            };
        c.finish()?;
        columns.insert(kind, column);
        Ok(())
    }
    pub fn ensure_all(&self) -> Result<()> {
        if self.lazy.lock().unwrap().complete {
            return Ok(());
        }
        if let Format::Block { payloads, .. } = &self.format {
            let mut kinds = payloads
                .keys()
                .copied()
                .filter(|k| *k >= 3)
                .collect::<Vec<_>>();
            kinds.sort_unstable();
            for kind in kinds {
                self.ensure(kind)?
            }
        };
        self.lazy.lock().unwrap().complete = true;
        Ok(())
    }
    fn index(&self, kind: u8, row: usize) -> Option<usize> {
        match &self.format {
            Format::Block { indexes, .. } => indexes.get(&kind).and_then(|v| v[row]),
            _ => None,
        }
    }
    fn name<'a>(&'a self, row: usize, lazy: &'a Lazy) -> &'a str {
        match &self.format {
            Format::Head { names, .. } => lazy.store.text(names[row]),
            Format::Block { schema, names, .. } => &schema.schema.names[names[row] as usize],
        }
    }
    fn level(&self, row: usize, lazy: &Lazy) -> Option<i32> {
        match &self.format {
            Format::Head { levels, .. } => levels[row],
            _ => self.index(4, row).map(|i| match &lazy.columns[&4] {
                Column::Ints(vs) => vs[i] as i32,
                _ => unreachable!(),
            }),
        }
    }
    fn body<'a>(&'a self, row: usize, lazy: &'a Lazy) -> Option<&'a str> {
        match &self.format {
            Format::Head { bodies, .. } => bodies[row].map(|t| lazy.store.text(t)),
            _ => self.index(5, row).map(|i| match &lazy.columns[&5] {
                Column::Text(vs) => lazy.store.text(vs[i]),
                _ => unreachable!(),
            }),
        }
    }
    fn id<const N: usize>(&self, kind: u8, row: usize, lazy: &Lazy) -> [u8; N] {
        if let Format::Head { traces, spans, .. } = &self.format {
            return if kind == 6 {
                traces[row].as_slice().try_into().unwrap()
            } else {
                spans[row].as_slice().try_into().unwrap()
            };
        };
        self.index(kind, row)
            .map_or([0; N], |i| match &lazy.columns[&kind] {
                Column::Ids(vs) => {
                    let t = vs[i];
                    lazy.store.buffers[t.buffer as usize][t.start as usize..t.end as usize]
                        .try_into()
                        .unwrap()
                }
                _ => unreachable!(),
            })
    }
    fn fields<'a>(&'a self, kind: u8, row: usize, lazy: &'a Lazy) -> FieldView<'a> {
        let fields = match &self.format {
            Format::Head {
                contexts, attrs, ..
            } => {
                if kind == 3 {
                    contexts[row]
                } else {
                    attrs[row]
                }
            }
            _ => {
                let Some(i) = self.index(kind, row) else {
                    return FieldView::Empty;
                };
                match &lazy.columns[&kind] {
                    Column::Ints(vs) => Fields::Context(vs[i] as usize),
                    Column::Raw(vs) => vs[i],
                    _ => unreachable!(),
                }
            }
        };
        match fields {
            Fields::Empty => FieldView::Empty,
            Fields::Inline { start, end } => FieldView::Spans(
                &lazy.store,
                &lazy.store.fields[start as usize..end as usize],
            ),
            Fields::Context(i) => {
                let Format::Block { schema, .. } = &self.format else {
                    unreachable!()
                };
                FieldView::Context(&schema.schema.contexts[i])
            }
        }
    }
    pub fn matches(&self, row: usize, q: &crate::Query) -> Result<bool> {
        if self.at(row) < q.from || self.at(row) >= q.to {
            return Ok(false);
        };
        if q.level.is_some() {
            self.ensure(4)?
        };
        if q.trace.is_some() {
            self.ensure(6)?
        };
        if !q.context.is_empty() {
            self.ensure(3)?
        };
        if !q.attrs.is_empty() {
            self.ensure(8)?
        };
        if !q.search.is_empty() {
            self.ensure(5)?
        };
        let lazy = self.lazy.lock().unwrap();
        Ok(q.level
            .is_none_or(|l| self.level(row, &lazy).is_some_and(|v| v >= l))
            && q.trace.is_none_or(|t| self.id::<16>(6, row, &lazy) == t)
            && q.context
                .iter()
                .all(|f| self.fields(3, row, &lazy).contains(f))
            && q.attrs
                .iter()
                .all(|f| self.fields(8, row, &lazy).contains(f))
            && (q.search.is_empty()
                || self.name(row, &lazy).to_lowercase().contains(&q.search)
                || self
                    .body(row, &lazy)
                    .is_some_and(|s| s.to_lowercase().contains(&q.search))))
    }
    pub fn record(&self, row: usize) -> Result<Record> {
        self.ensure_all()?;
        let lazy = self.lazy.lock().unwrap();
        Ok(Record {
            at: self.at(row),
            stream: self.stream.to_string(),
            name: self.name(row, &lazy).into(),
            level: self.level(row, &lazy),
            body: self.body(row, &lazy).map(str::to_owned),
            trace: self.id(6, row, &lazy),
            span: self.id(7, row, &lazy),
            context: self.fields(3, row, &lazy).owned(),
            attrs: self.fields(8, row, &lazy).owned(),
        })
    }
    pub fn serialize(&self, row: usize, out: &mut Vec<u8>) -> Result<()> {
        self.ensure_all()?;
        let lazy = self.lazy.lock().unwrap();
        let level = self.level(row, &lazy);
        let body = self.body(row, &lazy);
        let trace = self.id::<16>(6, row, &lazy);
        let span = self.id::<8>(7, row, &lazy);
        let context = self.fields(3, row, &lazy);
        vi(out, self.at(row));
        string(out, self.name(row, &lazy));
        let p = u8::from(level.is_some())
            | u8::from(body.is_some()) << 1
            | u8::from(trace != [0; 16]) << 2
            | u8::from(span != [0; 8]) << 3
            | u8::from(context.len() != 0) << 4;
        out.push(p);
        if let Some(l) = level {
            vi(out, i64::from(l))
        };
        if let Some(s) = body {
            string(out, s)
        };
        if p & 4 != 0 {
            out.extend_from_slice(&trace)
        };
        if p & 8 != 0 {
            out.extend_from_slice(&span)
        };
        context.serialize(out);
        self.fields(8, row, &lazy).serialize(out);
        Ok(())
    }
    pub fn stream(&self) -> &str {
        &self.stream
    }
    pub fn retained_bytes(&self) -> usize {
        let lazy = self.lazy.lock().unwrap();
        let mut n = std::mem::size_of::<Self>()
            + self.times.capacity() * 8
            + lazy.store.buffers.capacity() * std::mem::size_of::<Arc<[u8]>>()
            + lazy
                .store
                .buffers
                .iter()
                .map(|b| b.len() + 64)
                .sum::<usize>()
            + lazy.store.fields.capacity() * std::mem::size_of::<FieldSpan>()
            + lazy.columns.capacity() * (std::mem::size_of::<(u8, Column)>() + 32);
        for c in lazy.columns.values() {
            n += match c {
                Column::Ints(v) => v.capacity() * 8,
                Column::Text(v) | Column::Ids(v) => v.capacity() * std::mem::size_of::<Text>(),
                Column::Raw(v) => v.capacity() * std::mem::size_of::<Fields>(),
            } + 32
        }
        if let Format::Block {
            schema,
            body,
            payloads,
            indexes,
            names,
            shapes,
        } = &self.format
        {
            n += schema.retained_bytes()
                + body.len()
                + 64
                + payloads.capacity() * (std::mem::size_of::<(u8, Range<usize>)>() + 32)
                + indexes.capacity() * (std::mem::size_of::<(u8, Vec<Option<usize>>)>() + 32)
                + indexes
                    .values()
                    .map(|v| v.capacity() * std::mem::size_of::<Option<usize>>() + 32)
                    .sum::<usize>()
                + names.capacity() * 8
                + shapes.capacity() * 8
        };
        n + 512
    }
}
enum FieldView<'a> {
    Empty,
    Spans(&'a Storage, &'a [FieldSpan]),
    Context(&'a [Field]),
}
impl FieldView<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Spans(_, fs) => fs.len(),
            Self::Context(fs) => fs.len(),
        }
    }
    fn each(&self, mut f: impl FnMut(&str, &str)) {
        match self {
            Self::Empty => {}
            Self::Spans(store, fs) => {
                for field in *fs {
                    f(store.text(field.key), store.text(field.value))
                }
            }
            Self::Context(fs) => {
                for field in *fs {
                    f(&field.key, &field.value)
                }
            }
        }
    }
    fn contains(&self, want: &Field) -> bool {
        let mut found = false;
        self.each(|k, v| found |= k == want.key && v == want.value);
        found
    }
    fn owned(&self) -> Vec<Field> {
        let mut out = Vec::with_capacity(self.len());
        self.each(|k, v| {
            out.push(Field {
                key: k.into(),
                value: v.into(),
            })
        });
        out
    }
    fn serialize(&self, out: &mut Vec<u8>) {
        uv(out, self.len() as u64);
        self.each(|k, v| {
            string(out, k);
            string(out, v)
        })
    }
}

pub fn head(stream: &str, body: &[u8]) -> Result<Arc<Chunk>> {
    let mut c = Cursor {
        data: checked(body)?,
    };
    let count = c.count(1024)?;
    if count == 0 {
        return Err("empty head".into());
    };
    let raw = decompress(c.data, 4 << 20)?;
    let bytes: Arc<[u8]> = raw.into();
    let mut c = Cursor { data: &bytes };
    let mut store = Storage::new();
    store.buffers.push(Arc::clone(&bytes));
    let (
        mut times,
        mut names,
        mut levels,
        mut bodies,
        mut traces,
        mut spans,
        mut contexts,
        mut attrs,
    ) = (
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    );
    for _ in 0..count {
        times.push(c.vi()?);
        names.push(read_span(&mut c, 0, bytes.len(), true)?);
        let p = c.byte()?;
        if p & !31 != 0 {
            return Err("head presence".into());
        };
        levels.push(if p & 1 != 0 {
            Some(i32::try_from(c.vi()?)?)
        } else {
            None
        });
        bodies.push(if p & 2 != 0 {
            Some(read_span(&mut c, 0, bytes.len(), true)?)
        } else {
            None
        });
        traces.push(if p & 4 != 0 {
            c.take(16)?.try_into()?
        } else {
            [0; 16]
        });
        spans.push(if p & 8 != 0 {
            c.take(8)?.try_into()?
        } else {
            [0; 8]
        });
        for list in [&mut contexts, &mut attrs] {
            let n = c.count(128)?;
            let start = store.fields.len();
            for _ in 0..n {
                let key = read_span(&mut c, 0, bytes.len(), true)?;
                let value = read_span(&mut c, 0, bytes.len(), true)?;
                store.fields.push(FieldSpan { key, value })
            }
            list.push(if n == 0 {
                Fields::Empty
            } else {
                Fields::Inline {
                    start: start as u32,
                    end: store.fields.len() as u32,
                }
            })
        }
        if (p & 16 != 0) != !matches!(contexts.last().unwrap(), Fields::Empty) {
            return Err("context presence".into());
        }
    }
    c.finish()?;
    Ok(Arc::new(Chunk {
        times,
        stream: stream.into(),
        format: Format::Head {
            names,
            levels,
            bodies,
            traces,
            spans,
            contexts,
            attrs,
        },
        lazy: Mutex::new(Lazy {
            store,
            columns: HashMap::new(),
            budget: Expansion {
                used: bytes.len(),
                limit: 4 << 20,
            },
            complete: true,
        }),
    }))
}
pub fn block(schema: Arc<SharedSchema>, body: Arc<[u8]>) -> Result<Arc<Chunk>> {
    let s = &schema.schema;
    let mut c = Cursor {
        data: checked(&body)?,
    };
    let n = c.count(1024)?;
    if n == 0 {
        return Err("empty block".into());
    };
    let kinds = slots(s);
    let lens = (0..kinds.len())
        .map(|_| c.count(body.len()))
        .collect::<Result<Vec<_>>>()?;
    let mut payloads = HashMap::new();
    for (&k, l) in kinds.iter().zip(lens) {
        let start = body.len() - 4 - c.data.len();
        c.take(l)?;
        payloads.insert(k, start..start + l);
    }
    c.finish()?;
    let read_ids = |kind: u8, limit: usize| -> Result<Vec<i64>> {
        if let Some(range) = payloads.get(&kind) {
            let mut c = Cursor {
                data: &body[range.clone()],
            };
            let ids = read_numbers(&mut c, n)?;
            c.finish()?;
            if ids.iter().any(|v| *v < 0 || *v >= limit as i64) {
                return Err("id reference".into());
            };
            Ok(ids)
        } else {
            Ok(vec![0; n])
        }
    };
    let shapes = read_ids(2, s.shapes.len())?;
    let names = read_ids(1, s.names.len())?;
    let mut c = Cursor {
        data: &body[payloads[&0].clone()],
    };
    let times = read_numbers(&mut c, n)?;
    c.finish()?;
    if times.windows(2).any(|x| x[0] > x[1]) {
        return Err("unordered block".into());
    };
    let mut indexes = HashMap::new();
    for &kind in &kinds {
        let mut next = 0;
        indexes.insert(
            kind,
            shapes
                .iter()
                .map(|id| {
                    if carries(s.shapes[*id as usize], kind) {
                        let i = next;
                        next += 1;
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect(),
        );
    }
    let stream = s.stream.as_str().into();
    Ok(Arc::new(Chunk {
        times,
        stream,
        format: Format::Block {
            schema,
            body,
            payloads,
            indexes,
            names,
            shapes,
        },
        lazy: Mutex::new(Lazy {
            store: Storage::new(),
            columns: HashMap::new(),
            budget: Expansion {
                used: 0,
                limit: 4 << 20,
            },
            complete: false,
        }),
    }))
}
#[derive(Clone)]
pub struct Row {
    chunk: Arc<Chunk>,
    row: usize,
}
impl Row {
    pub fn new(chunk: Arc<Chunk>, row: usize) -> Self {
        Self { chunk, row }
    }
    pub fn ready(&self) -> Result<()> {
        let chunk = &self.chunk;
        chunk.ensure_all()?;
        Ok(())
    }
    pub fn detached(self) -> Result<Record> {
        self.chunk.record(self.row)
    }
    pub fn serialize(&self, out: &mut Vec<u8>) -> Result<()> {
        self.chunk.serialize(self.row, out)
    }
    pub fn stream(&self) -> &str {
        self.chunk.stream()
    }
}
#[derive(Default)]
pub struct Rows {
    pub shared: Vec<Row>,
    pub owned: Vec<Record>,
}
impl Rows {
    pub fn new(shared: Vec<Row>, share: bool) -> Result<Self> {
        if share {
            Ok(Self {
                shared,
                owned: vec![],
            })
        } else {
            Ok(Self {
                shared: vec![],
                owned: shared
                    .into_iter()
                    .map(Row::detached)
                    .collect::<Result<Vec<_>>>()?,
            })
        }
    }
    pub fn len(&self) -> usize {
        self.shared.len() + self.owned.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn extend(&mut self, other: Self) {
        self.shared.extend(other.shared);
        self.owned.extend(other.owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_head_and_block_match_original() {
        let rs = crate::fixture(64);
        let b = encode_head(&rs).unwrap();
        let h = head(&rs[0].stream, &b).unwrap();
        for (i, expected) in decode_head(&rs[0].stream, &b)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            assert_eq!(h.record(i).unwrap(), expected)
        }
        let rs = rs.into_iter().filter(|r| r.stream == "stream-0").collect();
        let s = encode_segment(rs).unwrap();
        let schema = schema(&s.row).unwrap();
        let b = block(schema, s.blocks[0].body.clone().into()).unwrap();
        for (i, expected) in s.records.iter().enumerate() {
            assert_eq!(&b.record(i).unwrap(), expected)
        }
    }
    #[test]
    fn cumulative_expansion_is_checked_before_allocation() {
        let mut budget = Expansion {
            used: (4 << 20) - 10,
            limit: 4 << 20,
        };
        assert!(budget.charge(11).is_err());
        assert_eq!(budget.used, (4 << 20) - 10);
        budget.charge(10).unwrap();
        assert_eq!(budget.used, 4 << 20)
    }
    #[test]
    fn owned_rows_outlive_decode_inputs() {
        let s = encode_segment(
            crate::fixture(8)
                .into_iter()
                .filter(|r| r.stream == "stream-0")
                .collect(),
        )
        .unwrap();
        let schema = schema(&s.row).unwrap();
        let chunk = block(schema, s.blocks[0].body.clone().into()).unwrap();
        let row = Row::new(chunk, 0);
        let expected = s.records[0].clone();
        drop(s);
        let mut actual = vec![];
        row.serialize(&mut actual).unwrap();
        let mut want = vec![];
        serialize_record(&mut want, &expected);
        assert_eq!(actual, want)
    }
    #[test]
    fn returned_fields_validate_before_return() {
        let s = encode_segment(
            crate::fixture(32)
                .into_iter()
                .filter(|r| r.stream == "stream-0")
                .collect(),
        )
        .unwrap();
        let schema = schema(&s.row).unwrap();
        let mut body = s.blocks[0].body.clone();
        let chunk = block(Arc::clone(&schema), body.clone().into()).unwrap();
        let Format::Block { payloads, .. } = &chunk.format else {
            unreachable!()
        };
        body[payloads[&5].start] = 255;
        let end = body.len() - 4;
        let crc = crc32fast::hash(&body[..end]);
        body[end..].copy_from_slice(&crc.to_le_bytes());
        let chunk = block(schema, body.into()).unwrap();
        let row = Row::new(chunk, 0);
        assert!(
            row.ready().is_err(),
            "selected record deferred malformed body validation"
        );
    }
    #[test]
    fn shared_rows_are_send_sync_and_survive_thread_handoff() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Row>();
        let s = encode_segment(
            crate::fixture(16)
                .into_iter()
                .filter(|r| r.stream == "stream-0")
                .collect(),
        )
        .unwrap();
        let chunk = block(schema(&s.row).unwrap(), s.blocks[0].body.clone().into()).unwrap();
        let row = Row::new(chunk, 1);
        row.ready().unwrap();
        let mut expected = vec![];
        serialize_record(&mut expected, &s.records[1]);
        drop(s);
        let actual = std::thread::spawn(move || {
            let mut bytes = vec![];
            row.serialize(&mut bytes).unwrap();
            bytes
        })
        .join()
        .unwrap();
        assert_eq!(actual, expected);
    }
}
