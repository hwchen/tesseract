/// Currently dead code, until module gets switched back on.

use bytes::Bytes;
use csv;
use failure::{Error, format_err};
use futures::{Stream, Async, Poll};
use indexmap::IndexMap;
use serde::Serializer;
use serde::ser::{SerializeSeq};
use serde_json::{Value};

use crate::dataframe::{DataFrame, ColumnData};
use super::format::FormatType;

/// Wrapper to format `DataFrame` to the desired output format.
pub fn format_records_stream<S>(headers: Vec<String>, df_stream: S, format_type: FormatType, error: bool) -> RecordBlockStream<S>
    where
    S: Stream<Item=Result<DataFrame, Error>, Error=Error> + 'static
{
    RecordBlockStream::new(df_stream, headers, format_type, error)
}

pub struct RecordBlockStream<S>
    where S: Stream<Item=Result<DataFrame, Error>, Error=Error> + 'static
{
    inner: S,
    sent_header: bool,
    eof: bool,
    sent_first_chunk: bool, // for not setting a leading comma
    format_type: FormatType,
    headers: Vec<String>,
    error: bool
}

impl<S> RecordBlockStream<S>
    where S: Stream<Item=Result<DataFrame, Error>, Error=Error> + 'static
{
    pub fn new(stream: S, headers: Vec<String>, format_type: FormatType, error: bool) -> Self {
        RecordBlockStream {
            inner: stream,
            sent_header: false,
            eof: false,
            sent_first_chunk: false,
            format_type,
            headers,
            error
        }
    }
}

impl<S> Stream for RecordBlockStream<S>
    where S: Stream<Item=Result<DataFrame, Error>, Error=Error> + 'static
{
    type Item = Bytes;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // first check eof
        // this is separate from matching on Asyn::Ready(None),
        // because the json formats need to have a trailing `]}`
        // after all the blocks for the body have been sent
        if self.eof {
            return Ok(Async::Ready(None));
        }


        // first look at header
        // send all the front matter
        // (before the body of data)
        if !self.sent_header {
            match self.format_type {
                FormatType::Csv => {
                    let mut wtr = csv::WriterBuilder::new()
                        .from_writer(vec![]);

                    wtr.write_record(&self.headers)?;

                    let buf = wtr.into_inner()?;
                    let bytes: Bytes = buf.into();

                    self.sent_header = true;
                    // csv doesn't require leading comma, so
                    // don't let any chunks have leading comma
                    self.sent_first_chunk = true;

                    return Ok(Async::Ready(Some(bytes)));
                },
                FormatType::JsonRecords => {
                    let buf = if self.error {
                        b"{\"error\":[".to_vec()
                    } else {
                        b"{\"data\":[".to_vec()
                    };
                    let bytes: Bytes = buf.into();

                    self.sent_header = true;
                    return Ok(Async::Ready(Some(bytes)));
                },
                FormatType::JsonArrays => {
                    let mut ser = serde_json::Serializer::new(
                        b"{\"headers\":".to_vec()
                    );
                    let mut seq_headers = ser.serialize_seq(Some(self.headers.len()))?;

                    for header in &self.headers {
                        seq_headers.serialize_element(header)?;
                    }
                    seq_headers.end()?;

                    // now data prefix
                    let mut buf = ser.into_inner();
                    if self.error {
                        buf.extend(b",\"error\":[");
                    } else {
                        buf.extend(b",\"data\":[");
                    }

                    let bytes: Bytes = buf.into();

                    self.sent_header = true;
                    return Ok(Async::Ready(Some(bytes)));
                },
                _ => return Err(format_err!("just csv first")),
            }
        }

        loop {
            let df_res = match self.inner.poll() {
                Err(err) => return Err(err),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Ok(Async::Ready(Some(df_res))) => df_res,
                Ok(Async::Ready(None)) => {
                    // instead of passing the "eof" straight through to stream,
                    // the json formats need to do a last bit of formatting.
                    // And then they can set the eof state to true,
                    // and that check will end the stream.
                    self.eof = true;
                    match self.format_type {
                        FormatType::Csv => {
                            // this could also send Async::Ready(None),
                            // but I want to end all streams in the same
                            // place, at the eof check
                            return Ok(Async::NotReady);
                        },
                        FormatType::JsonRecords => {
                            let res = b"]}".to_vec().into();
                            return Ok(Async::Ready(Some(res)));
                        },
                        FormatType::JsonArrays => {
                            let res = b"]}".to_vec().into();
                            return Ok(Async::Ready(Some(res)));
                        },
                        _ => return Err(format_err!("just csv first")),
                    }
                },
            };

            match df_res {
                Ok(df) => {
                    let formatted = match self.format_type {
                        FormatType::Csv => {
                            format_csv_body(df)?
                        },
                        FormatType::JsonRecords => {
                            // body should come back clean;
                            // - no trailing comma
                            // - no surrounding brackets
                            //
                            // lead_byte is set to `,` if it's not the first
                            // block, otherwise it's set to ` `

                            let lead_byte = if !self.sent_first_chunk {
                                self.sent_first_chunk = true;
                                ' ' as u8
                            } else {
                                ',' as u8
                            };

                            let body = format_jsonrecords_body(&self.headers, df, lead_byte)?;

                            return Ok(Async::Ready(Some(body)));
                        },
                        FormatType::JsonArrays => {
                            // same as for jsonrecords
                            // body should come back clean;
                            // - no trailing comma
                            // - no surrounding brackets
                            //
                            // lead_byte is set to `,` if it's not the first
                            // block, otherwise it's set to ` `

                            let lead_byte = if !self.sent_first_chunk {
                                self.sent_first_chunk = true;
                                ' ' as u8
                            } else {
                                ',' as u8
                            };

                            let body = format_jsonarrays_body(&self.headers, df, lead_byte)?;

                            return Ok(Async::Ready(Some(body)));
                        }
                        _ => return Err(format_err!("just csv first")),
                    };

                    return Ok(Async::Ready(Some(formatted)));
                },
                Err(err) => return Err(err),

            }
        }
    }
}


/// Formats response `DataFrame` to CSV.
fn format_csv_body(df: DataFrame) -> Result<Bytes, Error>
{
    let mut wtr = csv::WriterBuilder::new()
        .from_writer(vec![]);
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

    let buf = wtr.into_inner()?;
    let bytes = buf.into();

    Ok(bytes)
}

/// Formats response `DataFrame` to JSON records.
fn format_jsonrecords_body(headers: &[String], df: DataFrame, lead_byte: u8) -> Result<Bytes, Error> {
    // use streaming serializer
    // Necessary because this way we don't create a huge vec of rows containing Value
    // (very expensive)

    let mut ser = serde_json::Serializer::new(vec![]);
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
    let mut res = ser.into_inner();

    // because this is intermediate block, can't have `[` or `]`.
    // To prevent reallocation, just replace those with ` `.
    if let Some(v) = res.first_mut() {
        *v = lead_byte;
    }
    if let Some(v) = res.last_mut() {
        *v = lead_byte;
    }

    Ok(res.into())
}

/// Formats response `DataFrame` to JSON arrays.
fn format_jsonarrays_body(_headers: &[String], df: DataFrame, lead_byte: u8) -> Result<Bytes, Error> {
    // use streaming serializer
    // Necessary because this way we don't create a huge vec of rows containing Value
    // (very expensive)

    let mut ser = serde_json::Serializer::new(vec![]);
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
    let mut res = ser.into_inner();

    // because this is intermediate block, can't have `[` or `]`.
    // To prevent reallocation, just replace those with ` `.
    if let Some(v) = res.first_mut() {
        *v = lead_byte;
    }
    if let Some(v) = res.last_mut() {
        *v = lead_byte;
    }

    Ok(res.into())
}
