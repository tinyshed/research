// Arrow IPC stream: one schema message and one record batch, the unit in each field's metadata
// labels, under the flag, as Dictionary(Int32, Utf8)
use crate::{dictionary, texts, Column, Data, Frame};
use arrow_array::cast::AsArray;
use arrow_array::types::{Date32Type, Decimal128Type, Float64Type, Int32Type, Int64Type, TimestampMicrosecondType};
use arrow_array::{
    Array, ArrayRef, BooleanArray, Date32Array, Decimal128Array, DictionaryArray, Float64Array, Int32Array, Int64Array,
    RecordBatch, StringArray, TimestampMicrosecondArray,
};
use arrow_buffer::{BooleanBuffer, Buffer, NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use std::collections::HashMap;
use std::sync::Arc;

fn array(c: &Column, rows: usize, labels: bool) -> ArrayRef {
    let nulls = c.has_nulls().then(|| NullBuffer::from(c.nulls.as_ref().unwrap().iter().map(|n| !n).collect::<Vec<_>>()));
    if let Some(d) = labels.then(|| dictionary(c, rows)).flatten() {
        let keys = Int32Array::new(d.codes.iter().map(|&k| k as i32).collect::<Vec<_>>().into(), nulls);
        return Arc::new(DictionaryArray::<Int32Type>::try_new(keys, Arc::new(StringArray::from(d.names))).unwrap());
    }
    match &c.data {
        Data::Time(v) => Arc::new(TimestampMicrosecondArray::new(v.clone().into(), nulls).with_timezone("UTC")),
        Data::Wall(v) => Arc::new(TimestampMicrosecondArray::new(v.clone().into(), nulls)),
        Data::Date(v) => Arc::new(Date32Array::new(v.clone().into(), nulls)),
        Data::Int(v) => Arc::new(Int64Array::new(v.clone().into(), nulls)),
        Data::Float(v) => Arc::new(Float64Array::new(v.clone().into(), nulls)),
        Data::Decimal { mantissa, scale } => {
            Arc::new(Decimal128Array::new(mantissa.clone().into(), nulls).with_precision_and_scale(38, *scale).unwrap())
        }
        Data::Text { offsets, bytes } => {
            let offsets: Vec<i32> = offsets.iter().map(|&o| o as i32).collect();
            Arc::new(StringArray::new(OffsetBuffer::new(ScalarBuffer::from(offsets)), Buffer::from(bytes.clone()), nulls))
        }
        Data::Bool(v) => Arc::new(BooleanArray::new(BooleanBuffer::from_iter(v.iter().copied()), nulls)),
    }
}

pub fn encode(frame: &Frame, labels: bool) -> Vec<u8> {
    let arrays: Vec<ArrayRef> = frame.columns.iter().map(|c| array(c, frame.rows, labels)).collect();
    let fields: Vec<Field> = frame
        .columns
        .iter()
        .zip(&arrays)
        .map(|(c, a)| {
            let meta = c.unit.iter().map(|u| ("unit".to_string(), u.clone())).collect::<HashMap<_, _>>();
            Field::new(&c.name, a.data_type().clone(), c.has_nulls()).with_metadata(meta)
        })
        .collect();
    let schema = Arc::new(Schema::new(fields));
    let batch = RecordBatch::try_new(schema.clone(), arrays).unwrap();
    let mut writer = StreamWriter::try_new(Vec::new(), &schema).unwrap();
    writer.write(&batch).unwrap();
    writer.finish().unwrap();
    writer.into_inner().unwrap()
}

pub fn decode(bytes: &[u8]) -> Result<Frame, String> {
    let reader = StreamReader::try_new(std::io::Cursor::new(bytes), None).map_err(|e| e.to_string())?;
    let schema = reader.schema();
    let batches = reader.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let [batch] = batches.as_slice() else { return Err(format!("{} batches, one expected", batches.len())) };
    let mut columns = Vec::new();
    for (field, a) in schema.fields().iter().zip(batch.columns()) {
        let nulls = a.nulls().map(|n| n.iter().map(|valid| !valid).collect());
        let data = match a.data_type() {
            DataType::Timestamp(TimeUnit::Microsecond, Some(_)) => Data::Time(a.as_primitive::<TimestampMicrosecondType>().values().to_vec()),
            DataType::Timestamp(TimeUnit::Microsecond, None) => Data::Wall(a.as_primitive::<TimestampMicrosecondType>().values().to_vec()),
            DataType::Date32 => Data::Date(a.as_primitive::<Date32Type>().values().to_vec()),
            DataType::Int64 => Data::Int(a.as_primitive::<Int64Type>().values().to_vec()),
            DataType::Float64 => Data::Float(a.as_primitive::<Float64Type>().values().to_vec()),
            DataType::Decimal128(_, scale) => {
                Data::Decimal { mantissa: a.as_primitive::<Decimal128Type>().values().to_vec(), scale: *scale }
            }
            DataType::Utf8 => {
                let s = a.as_string::<i32>();
                let (base, end) = (s.value_offsets()[0], *s.value_offsets().last().unwrap());
                let offsets = s.value_offsets().iter().map(|&o| (o - base) as u32).collect();
                Data::Text { offsets, bytes: s.value_data()[base as usize..end as usize].to_vec() }
            }
            DataType::Boolean => Data::Bool(a.as_boolean().values().iter().collect()),
            DataType::Dictionary(_, _) => {
                let d = a.as_dictionary::<Int32Type>();
                let names = d.values().as_string::<i32>();
                let label = |(i, &k): (usize, &i32)| if d.is_null(i) { String::new() } else { names.value(k as usize).to_string() };
                texts(d.keys().values().iter().enumerate().map(label))
            }
            other => return Err(format!("column {}: type {other}", field.name())),
        };
        columns.push(Column { name: field.name().clone(), unit: field.metadata().get("unit").cloned(), data, nulls });
    }
    Ok(Frame { rows: batch.num_rows(), columns })
}
