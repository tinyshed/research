// columnar JSON: a whole number past 2^53 and every decimal travel as strings, a non-finite float by its name
use crate::{decimal_text, Column, Data, Frame};
use serde_json::Value;
use std::io::Write;

const SAFE: i64 = (1 << 53) - 1;

fn type_name(data: &Data) -> &'static str {
    match data {
        Data::Time(_) => "time",
        Data::Wall(_) => "wall",
        Data::Date(_) => "date",
        Data::Int(_) => "int",
        Data::Float(_) => "float",
        Data::Decimal { .. } => "decimal",
        Data::Text { .. } => "text",
        Data::Bool(_) => "bool",
    }
}

fn string(out: &mut Vec<u8>, s: &str) {
    serde_json::to_writer(&mut *out, s).unwrap();
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let mut out = Vec::with_capacity(frame.rows * frame.columns.len() * 12);
    write!(out, "{{\"rows\":{},\"columns\":[", frame.rows).unwrap();
    for (n, c) in frame.columns.iter().enumerate() {
        if n > 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"name\":");
        string(&mut out, &c.name);
        write!(out, ",\"type\":\"{}\"", type_name(&c.data)).unwrap();
        if let Some(unit) = &c.unit {
            out.extend_from_slice(b",\"unit\":");
            string(&mut out, unit);
        }
        if let Data::Decimal { scale, .. } = c.data {
            write!(out, ",\"scale\":{scale}").unwrap();
        }
        let wide = match &c.data {
            Data::Int(v) => v.iter().enumerate().any(|(i, x)| !c.is_null(i) && !(-SAFE..=SAFE).contains(x)),
            _ => false,
        };
        out.extend_from_slice(b",\"values\":[");
        for i in 0..frame.rows {
            if i > 0 {
                out.push(b',');
            }
            if c.is_null(i) {
                out.extend_from_slice(b"null");
                continue;
            }
            match &c.data {
                Data::Time(v) | Data::Wall(v) => write!(out, "{}", v[i]).unwrap(),
                Data::Int(v) if wide => write!(out, "\"{}\"", v[i]).unwrap(),
                Data::Int(v) => write!(out, "{}", v[i]).unwrap(),
                Data::Date(v) => write!(out, "{}", v[i]).unwrap(),
                Data::Float(v) if v[i].is_nan() => out.extend_from_slice(b"\"NaN\""),
                Data::Float(v) if v[i].is_infinite() => {
                    out.extend_from_slice(if v[i] > 0.0 { b"\"Infinity\"" } else { b"\"-Infinity\"" })
                }
                Data::Float(v) => serde_json::to_writer(&mut out, &v[i]).unwrap(),
                Data::Decimal { mantissa, scale } => write!(out, "\"{}\"", decimal_text(mantissa[i], *scale)).unwrap(),
                Data::Text { .. } => string(&mut out, c.text(i)),
                Data::Bool(v) => out.extend_from_slice(if v[i] { b"true" } else { b"false" }),
            }
        }
        out.extend_from_slice(b"]}");
    }
    out.extend_from_slice(b"]}");
    out
}

fn whole(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str()?.parse().ok())
}

fn float(v: &Value) -> Option<f64> {
    match v.as_str() {
        Some("NaN") => Some(f64::NAN),
        Some("Infinity") => Some(f64::INFINITY),
        Some("-Infinity") => Some(f64::NEG_INFINITY),
        _ => v.as_f64(),
    }
}

fn mantissa(v: &Value) -> Option<i128> {
    v.as_str()?.replace('.', "").parse().ok()
}

pub fn decode(bytes: &[u8]) -> Result<Frame, String> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let rows = doc["rows"].as_u64().ok_or("no rows")? as usize;
    let mut columns = Vec::new();
    for c in doc["columns"].as_array().ok_or("no columns")? {
        let name = c["name"].as_str().ok_or("a column without a name")?.to_string();
        let values = c["values"].as_array().filter(|v| v.len() == rows).ok_or(format!("column {name}: wrong length"))?;
        let nulls: Vec<bool> = values.iter().map(Value::is_null).collect();
        let bad = |what: &str| format!("column {name}: a value is not {what}");
        let ints = || values.iter().map(|v| if v.is_null() { Some(0) } else { whole(v) }).collect::<Option<Vec<i64>>>();
        let data = match c["type"].as_str().unwrap_or("") {
            "time" => Data::Time(ints().ok_or(bad("a time"))?),
            "wall" => Data::Wall(ints().ok_or(bad("a wall clock"))?),
            "int" => Data::Int(ints().ok_or(bad("a whole number"))?),
            "date" => Data::Date(ints().ok_or(bad("a date"))?.into_iter().map(|d| d as i32).collect()),
            "float" => Data::Float(
                values.iter().map(|v| if v.is_null() { Some(0.0) } else { float(v) }).collect::<Option<_>>().ok_or(bad("a float"))?,
            ),
            "decimal" => Data::Decimal {
                mantissa: values
                    .iter()
                    .map(|v| if v.is_null() { Some(0) } else { mantissa(v) })
                    .collect::<Option<_>>()
                    .ok_or(bad("a decimal"))?,
                scale: c["scale"].as_i64().ok_or(bad("a scale"))? as i8,
            },
            "text" => {
                let mut offsets = vec![0u32];
                let mut out = Vec::new();
                for v in values {
                    out.extend_from_slice(v.as_str().unwrap_or("").as_bytes());
                    offsets.push(out.len() as u32);
                }
                Data::Text { offsets, bytes: out }
            }
            "bool" => Data::Bool(values.iter().map(|v| v.as_bool().unwrap_or(false)).collect()),
            other => return Err(format!("column {name}: unknown type {other}")),
        };
        let unit = c["unit"].as_str().map(Into::into);
        columns.push(Column { name, unit, data, nulls: Some(nulls) });
    }
    Ok(Frame { rows, columns })
}
