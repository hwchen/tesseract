//! Convert clickhouse Block to tesseract_core::DataFrame
// extern crate mysql;

use failure::{Error, format_err};
extern crate mysql_async;
extern crate futures;
use std::str;
use mysql_async::{QueryResult, BinaryProtocol, Conn};
use mysql_async::consts::ColumnType::*;
use tesseract_core::{DataFrame, Column, ColumnData};
use mysql_async::Value::*;
use futures::future::{self, Future};

pub fn rows_to_df(query_result: QueryResult<Conn, BinaryProtocol>) -> Box<Future<Item=DataFrame, Error=Error>> {
    let mut tcolumn_list = vec![];
    let columns = query_result.columns_ref();

    // for each column figure out my type. add it to a vec
    for col in columns.iter() {
        let col_type = col.column_type();
        let col_name = col.name_str();
        // println!("NAME: {:?} TYPE {:?}", col.name_str(), col_type);
        match col_type {
            // confusing but TYPE_LONG is regular integer (32-bit)
            // see https://dev.mysql.com/doc/refman/8.0/en/c-api-prepared-statement-type-codes.html
            MYSQL_TYPE_LONGLONG | MYSQL_TYPE_LONG | MYSQL_TYPE_SHORT | MYSQL_TYPE_TINY => {
                tcolumn_list.push(Column::new(
                    col_name.to_string(),
                    ColumnData::Int64(vec![]),
                ))
            },
            MYSQL_TYPE_VARCHAR | MYSQL_TYPE_VAR_STRING=> {
                tcolumn_list.push(Column::new(
                    col_name.to_string(),
                    ColumnData::Text(vec![]),
                ))
            },
            MYSQL_TYPE_FLOAT => {
                tcolumn_list.push(Column::new(
                    col_name.to_string(),
                    ColumnData::Float32(vec![]),
                ))
            },
            MYSQL_TYPE_DOUBLE => {
                tcolumn_list.push(Column::new(
                    col_name.to_string(),
                    ColumnData::Float64(vec![]),
                ))
            },
            t => return Box::new(future::err(format_err!("Mysql type not yet supported: {:?}", t))),
        }
    }

    let df = DataFrame::from_vec(tcolumn_list);

    let future = query_result.reduce(df, |mut df_accum, r| {
        let row = r.unwrap();

        for col_idx in 0..df_accum.columns.len() {
            let column_data = df_accum.columns
                .get_mut(col_idx)
                .expect("logic checked?")
                .column_data();
            match column_data {
                ColumnData::Int64(col_data) => {
                    let raw_value = row.get(col_idx).unwrap();
                    match raw_value {
                        Int(y) => Some(col_data.push(*y)),
                        _s => None
                    };
                },
                ColumnData::Text(col_data) => {
                    let raw_value = row.get(col_idx).unwrap();
                    match raw_value {
                        Bytes(y) => {
                            let tmp_str = str::from_utf8(y).unwrap();
                            // TODO is there a more memory efficient way to handle this
                            // other than copying the strings into the dataframe
                            Some(col_data.push(tmp_str.to_string()))
                        },
                        _s => None
                    };
                },
                _s => {
                    println!("FAILING HERE!");
                }
            }
        }

        df_accum
    })
    .map(|(_, df)| df)
    .map_err(|err| format_err!("mysql err {}", err));

    Box::new(future)
}