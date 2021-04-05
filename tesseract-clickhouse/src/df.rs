//! Convert clickhouse Block to tesseract_core::DataFrame

use anyhow::{Error, bail};

use clickhouse_rs::types::{Block, ColumnType, Complex, Simple, SqlType};
use tesseract_core::{DataFrame, Column, ColumnData};

// from source code of clickhouse_rs
//             SqlType::UInt8 => "UInt8".into(),
//            SqlType::UInt16 => "UInt16".into(),
//            SqlType::UInt32 => "UInt32".into(),
//            SqlType::UInt64 => "UInt64".into(),
//            SqlType::Int8 => "Int8".into(),
//            SqlType::Int16 => "Int16".into(),
//            SqlType::Int32 => "Int32".into(),
//            SqlType::Int64 => "Int64".into(),
//            SqlType::String => "String".into(),
//            SqlType::Float32 => "Float32".into(),
//            SqlType::Float64 => "Float64".into(),
//            SqlType::Date => "Date".into(),
//            SqlType::DateTime => "DateTime".into(),

pub trait ColumnBuilder: ColumnType {
    fn build(col_idx: usize, block: &Block<Self>) -> Result<Column, Error>;
}

macro_rules! def_column_builder {
    ( $($k:ty), * ) => {
        $(
            impl ColumnBuilder for $k {
                fn build(
                    col_idx: usize,
                    block: &Block<Self>,
                ) -> Result<Column, Error> {
                    let src_column = &block.columns()[col_idx];
                    match src_column.sql_type() {
                        SqlType::UInt8 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::UInt8(src_column.iter::<u8>()?.copied().collect()),
                        )),
                        SqlType::UInt16 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::UInt16(src_column.iter::<u16>()?.copied().collect()),
                        )),
                        SqlType::UInt32 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::UInt32(src_column.iter::<u32>()?.copied().collect()),
                        )),
                        SqlType::UInt64 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::UInt64(src_column.iter::<u64>()?.copied().collect()),
                        )),
                        SqlType::Int8 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::Int8(src_column.iter::<i8>()?.copied().collect()),
                        )),
                        SqlType::Int16 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::Int16(src_column.iter::<i16>()?.copied().collect()),
                        )),
                        SqlType::Int32 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::Int32(src_column.iter::<i32>()?.copied().collect()),
                        )),
                        SqlType::Int64 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::Int64(src_column.iter::<i64>()?.copied().collect()),
                        )),
                        SqlType::String => {
                            let mut column_data = Vec::with_capacity(block.row_count());

                            for source in src_column.iter::<&[u8]>()? {
                                let text = String::from_utf8(source.into())?;
                                column_data.push(text);
                            }

                            Ok(Column::new(
                                src_column.name().to_owned(),
                                ColumnData::Text(column_data),
                            ))
                        }
                        SqlType::Float32 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::Float32(src_column.iter::<f32>()?.copied().collect()),
                        )),
                        SqlType::Float64 => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::Float64(src_column.iter::<f64>()?.copied().collect()),
                        )),
                        SqlType::Nullable(SqlType::UInt8) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableUInt8(
                                src_column
                                    .iter::<Option<u8>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::UInt16) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableUInt16(
                                src_column
                                    .iter::<Option<u16>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::UInt32) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableUInt32(
                                src_column
                                    .iter::<Option<u32>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::UInt64) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableUInt64(
                                src_column
                                    .iter::<Option<u64>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::Int8) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableInt8(
                                src_column
                                    .iter::<Option<i8>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::Int16) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableInt16(
                                src_column
                                    .iter::<Option<i16>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::Int32) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableInt32(
                                src_column
                                    .iter::<Option<i32>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::Int64) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableInt64(
                                src_column
                                    .iter::<Option<i64>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::String) => {
                            let mut column_data = Vec::with_capacity(block.row_count());

                            for source in src_column.iter::<Option<&[u8]>>()? {
                                let text = match source {
                                    None => None,
                                    Some(source) => Some(String::from_utf8(source.into())?),
                                };
                                column_data.push(text);
                            }

                            Ok(Column::new(
                                src_column.name().to_owned(),
                                ColumnData::NullableText(column_data),
                            ))
                        }
                        SqlType::Nullable(SqlType::Float32) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableFloat32(
                                src_column
                                    .iter::<Option<f32>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        SqlType::Nullable(SqlType::Float64) => Ok(Column::new(
                            src_column.name().to_owned(),
                            ColumnData::NullableFloat64(
                                src_column
                                    .iter::<Option<f64>>()?
                                    .map(|u| u.map(|v| *v))
                                    .collect(),
                            ),
                        )),
                        s => bail!("{} is not supported by tesseract", s),
                    }
                }
            }
        )*
    };
}

def_column_builder! {
    Simple,
    Complex
}

pub fn block_to_df<K: ColumnBuilder>(block: Block<K>) -> Result<DataFrame, Error> {
    let mut df = Vec::with_capacity(block.column_count());

    for (index, _col) in block.columns().iter().enumerate() {
        df.push(K::build(index, &block)?);
    }

    Ok(DataFrame::from_vec(df))
}

#[test]
fn test_block_to_df() {
    let block = Block::new()
        .column("u8", vec![1_u8, 2, 3])
        .column("u16", vec![1_u16, 2, 3])
        .column("u32", vec![1_u32, 2, 3])
        .column("u64", vec![1_u64, 2, 3])
        .column("i8", vec![1_i8, 2, 3])
        .column("i16", vec![1_i16, 2, 3])
        .column("i32", vec![1_i32, 2, 3])
        .column("i64", vec![1_i64, 2, 3])
        .column("str", vec!["A", "B", "C"])
        .column("opt_u8", vec![Some(1_u8), None, Some(3)])
        .column("opt_u16", vec![Some(1_u16), None, Some(3)])
        .column("opt_u32", vec![Some(1_u32), None, Some(3)])
        .column("opt_u64", vec![Some(1_u64), None, Some(3)])
        .column("opt_i8", vec![Some(1_i8), None, Some(3)])
        .column("opt_i16", vec![Some(1_i16), None, Some(3)])
        .column("opt_i32", vec![Some(1_i32), None, Some(3)])
        .column("opt_i64", vec![Some(1_i64), None, Some(3)])
        .column("opt_str", vec![Some("A"), None, Some("C")])
        .column("f32", vec![1.0_f32, 2.0, 3.0])
        .column("f64", vec![1.0_f64, 2.0, 3.0])
        .column("opt_f32", vec![Some(1.0_f32), None, Some(3.0)])
        .column("opt_f64", vec![Some(1.0_f64), None, Some(3.0)]);

    let df = block_to_df(block).unwrap();

    assert_eq!(df.columns[0].column_data, ColumnData::UInt8(vec![1, 2, 3]));
    assert_eq!(df.columns[1].column_data, ColumnData::UInt16(vec![1, 2, 3]));
    assert_eq!(df.columns[2].column_data, ColumnData::UInt32(vec![1, 2, 3]));
    assert_eq!(df.columns[3].column_data, ColumnData::UInt64(vec![1, 2, 3]));

    assert_eq!(df.columns[4].column_data, ColumnData::Int8(vec![1, 2, 3]));
    assert_eq!(df.columns[5].column_data, ColumnData::Int16(vec![1, 2, 3]));
    assert_eq!(df.columns[6].column_data, ColumnData::Int32(vec![1, 2, 3]));
    assert_eq!(df.columns[7].column_data, ColumnData::Int64(vec![1, 2, 3]));

    assert_eq!(
        df.columns[8].column_data,
        ColumnData::Text(vec!["A".to_string(), "B".to_string(), "C".to_string()])
    );

    assert_eq!(
        df.columns[9].column_data,
        ColumnData::NullableUInt8(vec![Some(1), None, Some(3)])
    );
    assert_eq!(
        df.columns[10].column_data,
        ColumnData::NullableUInt16(vec![Some(1), None, Some(3)])
    );
    assert_eq!(
        df.columns[11].column_data,
        ColumnData::NullableUInt32(vec![Some(1), None, Some(3)])
    );
    assert_eq!(
        df.columns[12].column_data,
        ColumnData::NullableUInt64(vec![Some(1), None, Some(3)])
    );

    assert_eq!(
        df.columns[13].column_data,
        ColumnData::NullableInt8(vec![Some(1), None, Some(3)])
    );
    assert_eq!(
        df.columns[14].column_data,
        ColumnData::NullableInt16(vec![Some(1), None, Some(3)])
    );
    assert_eq!(
        df.columns[15].column_data,
        ColumnData::NullableInt32(vec![Some(1), None, Some(3)])
    );
    assert_eq!(
        df.columns[16].column_data,
        ColumnData::NullableInt64(vec![Some(1), None, Some(3)])
    );

    assert_eq!(
        df.columns[17].column_data,
        ColumnData::NullableText(vec![Some("A".to_string()), None, Some("C".to_string())])
    );

    assert_eq!(
        df.columns[18].column_data,
        ColumnData::Float32(vec![1.0, 2.0, 3.0])
    );
    assert_eq!(
        df.columns[19].column_data,
        ColumnData::Float64(vec![1.0, 2.0, 3.0])
    );

    assert_eq!(
        df.columns[20].column_data,
        ColumnData::NullableFloat32(vec![Some(1.0), None, Some(3.0)])
    );
    assert_eq!(
        df.columns[21].column_data,
        ColumnData::NullableFloat64(vec![Some(1.0), None, Some(3.0)])
    );
}
