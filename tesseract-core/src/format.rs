use csv;
use anyhow::{Error, format_err};
use indexmap::IndexMap;
use serde::Serializer;
use serde::ser::{SerializeSeq};
use serde_json::{Value};

use crate::schema::metadata::SourceMetadata;
use crate::dataframe::{DataFrame, ColumnData};

#[derive(Debug, Clone)]
pub enum FormatType{
    Csv,
    JsonRecords,
    JsonArrays,
}

impl std::str::FromStr for FormatType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "csv" => Ok(FormatType::Csv),
            "jsonrecords" => Ok(FormatType::JsonRecords),
            "jsonarrays" => Ok(FormatType::JsonArrays),
            _ => Err(format_err!("{} is not a supported format", s)),
        }
    }
}

/// Wrapper to format `DataFrame` to the desired output format.
pub fn format_records(
    headers: &[String],
    df: DataFrame,
    format_type: FormatType,
    source_data: Option<SourceMetadata>,
    error: bool
) -> Result<String, Error> {
    match format_type {
        FormatType::Csv => Ok(format_csv(headers, df)?),
        FormatType::JsonRecords => Ok(format_jsonrecords(headers, df, source_data, error)?),
        FormatType::JsonArrays => Ok(format_jsonarrays(headers, df, error)?),
    }
}

/// Formats response `DataFrame` to CSV.
fn format_csv(headers: &[String], df: DataFrame) -> Result<String, Error> {
    let mut wtr = csv::WriterBuilder::new()
        .from_writer(vec![]);

    // write header
    wtr.write_record(headers)?;

    let mut row_buf = vec![];

    // write data
    for row_idx in 0..df.len() {
        for col_idx in 0..df.columns.len() {
            let val = match df.columns[col_idx].column_data {
                ColumnData::Int8(ref ns) =>    ns[row_idx].to_string(),
                ColumnData::Int16(ref ns) =>   ns[row_idx].to_string(),
                ColumnData::Int32(ref ns) =>   ns[row_idx].to_string(),
                ColumnData::Int64(ref ns) =>   ns[row_idx].to_string(),
                ColumnData::UInt8(ref ns) =>   ns[row_idx].to_string(),
                ColumnData::UInt16(ref ns) =>  ns[row_idx].to_string(),
                ColumnData::UInt32(ref ns) =>  ns[row_idx].to_string(),
                ColumnData::UInt64(ref ns) =>  ns[row_idx].to_string(),
                ColumnData::Float32(ref ns) => ns[row_idx].to_string(),
                ColumnData::Float64(ref ns) => ns[row_idx].to_string(),
                ColumnData::Text(ref ss) =>    ss[row_idx].to_string(),
                ColumnData::NullableInt8(ref ns) =>    ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableInt16(ref ns) =>   ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableInt32(ref ns) =>   ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableInt64(ref ns) =>   ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableUInt8(ref ns) =>   ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableUInt16(ref ns) =>  ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableUInt32(ref ns) =>  ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableUInt64(ref ns) =>  ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableFloat32(ref ns) => ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableFloat64(ref ns) => ns[row_idx].map(|n| n.to_string()).unwrap_or("".into()),
                ColumnData::NullableText(ref ss) =>    ss[row_idx].clone().unwrap_or("".into()),
            };

            row_buf.push(val);
        }
        wtr.write_record(&row_buf)?;

        row_buf.clear();
    }

    let res = String::from_utf8(wtr.into_inner()?)?;

    Ok(res)
}

/// Formats response `DataFrame` to JSON records.
fn format_jsonrecords(headers: &[String], df: DataFrame, source_data: Option<SourceMetadata>, error: bool) -> Result<String, Error> {
    // use streaming serializer
    // Necessary because this way we don't create a huge vec of rows containing Value
    // (very expensive)

    // I've ended up doing this a bit more manually than I want; but I don't know how
    // to easily move between manually calling serializing methods, and having some
    // done more automatically. For example, the rows have to be serialized using
    // seq, but I can't easily call serialize_data including those rows, since I'm using
    // a manual method different from the serialize_data.

    // I had a hard time figuring out how to serialize the struct before the data.
    // So I just wrote the bytes in and put a '}' at the end, and serialized
    // the values in between.
    let mut ser = if error {
        serde_json::Serializer::new(
            b"{\"error\":".to_vec()
        )
    } else {
        serde_json::Serializer::new(
            b"{\"data\":".to_vec()
        )
    };

    let mut seq = ser.serialize_seq(Some(df.len()))?;
    // write data
    for row_idx in 0..df.len() {
        let mut row: IndexMap<&str, serde_json::Value> = IndexMap::new();
        for col_idx in 0..df.columns.len() {
            let val = match df.columns[col_idx].column_data {
                ColumnData::Int8(ref ns) =>    ns[row_idx].clone().into(),
                ColumnData::Int16(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::Int32(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::Int64(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::UInt8(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::UInt16(ref ns) =>  ns[row_idx].clone().into(),
                ColumnData::UInt32(ref ns) =>  ns[row_idx].clone().into(),
                ColumnData::UInt64(ref ns) =>  ns[row_idx].clone().into(),
                ColumnData::Float32(ref ns) => ns[row_idx].clone().into(),
                ColumnData::Float64(ref ns) => ns[row_idx].clone().into(),
                ColumnData::Text(ref ss) =>    ss[row_idx].clone().into(),
                ColumnData::NullableInt8(ref ns) =>    ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableInt16(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableInt32(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableInt64(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt8(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt16(ref ns) =>  ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt32(ref ns) =>  ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt64(ref ns) =>  ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableFloat32(ref ns) => ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableFloat64(ref ns) => ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableText(ref ss) =>    ss[row_idx].clone().map(|n| n.into()).unwrap_or(Value::Null),
            };

            row.insert(&headers[col_idx], val);
        }

        seq.serialize_element(&row)?;
    }
    seq.end()?;
    let mut res = String::from_utf8(ser.into_inner())?;
    let source_string = serde_json::to_string(&source_data)?;
    if source_string != "null" {
        res.push_str(",\n\"source\": [\n");
        res.push_str(&source_string);
        res.push_str("\n]");
    }
    res.push('}');
    Ok(res)

//    let res = json!({
//        "data": rows,
//    });
//
//    Ok(res.to_string())
}

/// Formats response `DataFrame` to JSON arrays.
fn format_jsonarrays(headers: &[String], df: DataFrame, error: bool) -> Result<String, Error> {
    // use streaming serializer
    // Necessary because this way we don't create a huge vec of rows containing Value
    // (very expensive)

    // I've ended up doing this a bit more manually than I want; but I don't know how
    // to easily move between manually calling serializing methods, and having some
    // done more automatically. For example, the rows have to be serialized using
    // seq, but I can't easily call serialize_data including those rows, since I'm using
    // a manual method different from the serialize_data.

    // serialize headers
    let mut ser = serde_json::Serializer::new(
        b"{\"headers\":".to_vec()
    );
    let mut seq_headers = ser.serialize_seq(Some(headers.len()))?;

    for header in headers {
        seq_headers.serialize_element(&header)?;
    }
    seq_headers.end()?;


    // now the data
    let mut intermediate = ser.into_inner();
    if error {
        intermediate.extend(b",\"error\":");
    } else {
        intermediate.extend(b",\"data\":");
    }


    let mut ser = serde_json::Serializer::new(intermediate);
    let mut seq_data = ser.serialize_seq(Some(df.len()))?;


    // then write data
    for row_idx in 0..df.len() {
        let mut row: Vec<serde_json::Value> = vec![];
        for col_idx in 0..df.columns.len() {
            let val = match df.columns[col_idx].column_data {
                ColumnData::Int8(ref ns) =>    ns[row_idx].clone().into(),
                ColumnData::Int16(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::Int32(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::Int64(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::UInt8(ref ns) =>   ns[row_idx].clone().into(),
                ColumnData::UInt16(ref ns) =>  ns[row_idx].clone().into(),
                ColumnData::UInt32(ref ns) =>  ns[row_idx].clone().into(),
                ColumnData::UInt64(ref ns) =>  ns[row_idx].clone().into(),
                ColumnData::Float32(ref ns) => ns[row_idx].clone().into(),
                ColumnData::Float64(ref ns) => ns[row_idx].clone().into(),
                ColumnData::Text(ref ss) =>    ss[row_idx].clone().into(),
                ColumnData::NullableInt8(ref ns) =>    ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableInt16(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableInt32(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableInt64(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt8(ref ns) =>   ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt16(ref ns) =>  ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt32(ref ns) =>  ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableUInt64(ref ns) =>  ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableFloat32(ref ns) => ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableFloat64(ref ns) => ns[row_idx].map(|n| n.clone().into()).unwrap_or(Value::Null),
                ColumnData::NullableText(ref ss) =>    ss[row_idx].clone().map(|n| n.into()).unwrap_or(Value::Null),
            };

            row.push(val);
        }

        seq_data.serialize_element(&row)?;
    }

    seq_data.end()?;

    // now take out vec, convert to string, and return
    let mut res = String::from_utf8(ser.into_inner())?;
    res.push('}');
    Ok(res)

//    let res = json!({
//        "headers": headers,
//        "data": rows,
//    });
}
