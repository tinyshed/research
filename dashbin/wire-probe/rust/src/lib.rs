// the frame of chapter 3, two datasets built from a fixed seed, and three encodings of them
pub mod data;
#[cfg(feature = "json")]
pub mod json;
pub mod own;
#[cfg(feature = "arrow")]
pub mod arrow;

pub enum Data {
    Time(Vec<i64>),
    Wall(Vec<i64>),
    Date(Vec<i32>),
    Int(Vec<i64>),
    Float(Vec<f64>),
    Decimal { mantissa: Vec<i128>, scale: i8 },
    Text { offsets: Vec<u32>, bytes: Vec<u8> },
    Bool(Vec<bool>),
}

pub struct Column {
    pub name: String,
    pub unit: Option<String>,
    pub data: Data,
    // true where the value is null
    pub nulls: Option<Vec<bool>>,
}

pub struct Frame {
    pub rows: usize,
    pub columns: Vec<Column>,
}

impl Column {
    pub fn is_null(&self, i: usize) -> bool {
        self.nulls.as_ref().is_some_and(|n| n[i])
    }

    pub fn has_nulls(&self) -> bool {
        self.nulls.as_ref().is_some_and(|n| n.contains(&true))
    }

    pub fn text(&self, i: usize) -> &str {
        match &self.data {
            Data::Text { offsets, bytes } => std::str::from_utf8(&bytes[offsets[i] as usize..offsets[i + 1] as usize]).unwrap(),
            _ => unreachable!(),
        }
    }
}

pub struct Dictionary {
    pub codes: Vec<u32>,
    pub names: Vec<String>,
}

/// A text column with at most one distinct value per two rows, as codes into its distinct values; a null's code is 0.
pub fn dictionary(c: &Column, rows: usize) -> Option<Dictionary> {
    if !matches!(c.data, Data::Text { .. }) {
        return None;
    }
    let mut index = std::collections::HashMap::new();
    let mut names = Vec::new();
    let mut codes = Vec::with_capacity(rows);
    for i in 0..rows {
        if c.is_null(i) {
            codes.push(0);
            continue;
        }
        let code = *index.entry(c.text(i)).or_insert_with(|| {
            names.push(c.text(i).to_string());
            names.len() as u32 - 1
        });
        if names.len() > rows / 2 {
            return None;
        }
        codes.push(code);
    }
    Some(Dictionary { codes, names })
}

pub fn texts(values: impl Iterator<Item = String>) -> Data {
    let mut offsets = vec![0u32];
    let mut bytes = Vec::new();
    for v in values {
        bytes.extend_from_slice(v.as_bytes());
        offsets.push(bytes.len() as u32);
    }
    Data::Text { offsets, bytes }
}

pub fn decimal_text(mantissa: i128, scale: i8) -> String {
    if scale <= 0 {
        return format!("{mantissa}{}", "0".repeat(-scale as usize));
    }
    let digits = mantissa.unsigned_abs().to_string();
    let digits = format!("{digits:0>width$}", width = scale as usize + 1);
    let (whole, fraction) = digits.split_at(digits.len() - scale as usize);
    format!("{}{whole}.{fraction}", if mantissa < 0 { "-" } else { "" })
}

/// Compares what a reader can see: values under a null are ignored, and NaN equals NaN by its bits.
pub fn same(a: &Frame, b: &Frame) -> Result<(), String> {
    if a.rows != b.rows || a.columns.len() != b.columns.len() {
        return Err(format!("shape {}x{} against {}x{}", a.rows, a.columns.len(), b.rows, b.columns.len()));
    }
    for (x, y) in a.columns.iter().zip(&b.columns) {
        let name = &x.name;
        if x.name != y.name || x.unit != y.unit {
            return Err(format!("column {name}: name or unit differs"));
        }
        for i in 0..a.rows {
            if x.is_null(i) != y.is_null(i) {
                return Err(format!("column {name}: row {i} null on one side only"));
            }
            if x.is_null(i) {
                continue;
            }
            let equal = match (&x.data, &y.data) {
                (Data::Time(p), Data::Time(q)) | (Data::Wall(p), Data::Wall(q)) | (Data::Int(p), Data::Int(q)) => p[i] == q[i],
                (Data::Date(p), Data::Date(q)) => p[i] == q[i],
                (Data::Float(p), Data::Float(q)) => p[i].to_bits() == q[i].to_bits(),
                (Data::Decimal { mantissa: p, scale: s }, Data::Decimal { mantissa: q, scale: t }) => p[i] == q[i] && s == t,
                (Data::Text { .. }, Data::Text { .. }) => x.text(i) == y.text(i),
                (Data::Bool(p), Data::Bool(q)) => p[i] == q[i],
                _ => return Err(format!("column {name}: type differs")),
            };
            if !equal {
                return Err(format!("column {name}: row {i} differs"));
            }
        }
    }
    Ok(())
}
