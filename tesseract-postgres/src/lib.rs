use failure::{Error, format_err};

use tesseract_core::{Backend, DataFrame};
use tesseract_core::schema::metadata::SchemaPhysicalData;
use futures::{Future, Stream};
use tokio_postgres::NoTls;
extern crate futures;
extern crate tokio_postgres;
extern crate bb8;
extern crate bb8_postgres;
extern crate futures_state_stream;
extern crate tokio;
use std::thread;
use tokio_postgres::{Column , Row};
use tokio::executor::current_thread;
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use futures::{
    future::{err, lazy, Either},
};

mod df;
use self::df::{rows_to_df};

#[derive(Clone)]
pub struct Postgres {
    db_url: String,
    pool: Pool<PostgresConnectionManager<NoTls>>
}

impl Postgres {
    pub fn new(address: &str) -> Postgres {
        let pg_mgr: PostgresConnectionManager<NoTls> = PostgresConnectionManager::new(address, tokio_postgres::NoTls);
        let future = lazy(|| {
            Pool::builder()
                .build(pg_mgr)
        });
        // synchronously setup pg pool
        let mut runtime = tokio::runtime::Runtime::new().expect("Unable to create a runtime");
        let pool = runtime.block_on(future).unwrap();

        Postgres {
            db_url: address.to_string(),
            pool
        }
    }

    pub fn from_addr(address: &str) -> Result<Self, Error> {
        Ok(Postgres::new(address))
    }

    pub fn hangup() {
        println!("Done with connection! TODO!");
    }
}

// TODO:
// 1. better connection lifecycle management!
// 2. dataframe creation

impl Backend for Postgres {
    fn retrieve_schemas(&self, tablepath: &str, id: Option<&str>) -> Box<dyn Future<Item=Vec<SchemaPhysicalData>, Error=Error>> {
        let sql = match id {
            None => format!("SELECT id, schema FROM {}", tablepath),
            Some(id_val) => format!("SELECT id, schema FROM {} WHERE id = '{}", tablepath, id_val),
        };
        let fut = self.pool.run(move |mut connection| {
            connection.prepare(&sql).then( |r| match r {
                Ok(select) => {
                    let f = connection.query(&select, &[])
                        .collect()
                        .then(move |r| {
                            let rows: Vec<Row> = r.expect("Failure in query of schema rows");
                            let res = rows.into_iter().map(|row| {
                                SchemaPhysicalData {
                                    id: row.get::<usize, String>(0),
                                    content: row.get::<usize, String>(1),
                                    format: "json".to_string(),
                                }
                            }).collect();
                            Ok((res, connection))
                        });
                    Either::A(f)
                }
                Err(e) => Either::B(err((e, connection))),
            })
        }).map_err(|err| format_err!("Postgres error {:?}", err));
        return Box::new(fut);
    }

    fn exec_sql(&self, sql: String) -> Box<Future<Item=DataFrame, Error=Error>> {
        let fut = self.pool.run(move |mut connection| {
            connection.prepare(&sql).then( |r| match r {
                Ok(select) => {
                    let f = connection.query(&select, &[])
                        .collect()
                        .then(move |r| {
                            let df = rows_to_df(r.expect("Unable to retrieve rows"), select.columns());
                            Ok((df, connection))
                        });
                    Either::A(f)
                }
                Err(e) => Either::B(err((e, connection))),
            })
        }).map_err(|err| format_err!("Postgres error {:?}", err));
        Box::new(fut)
    }

    fn box_clone(&self) -> Box<dyn Backend + Send + Sync> {
        Box::new((*self).clone())
    }

    fn update_schema(&self, tablepath: &str, schema_name_id: &str, schema_content: &str) -> Box<dyn Future<Item=bool, Error=Error>> {
        let sql = format!("UPDATE {} SET schema = '{}' WHERE id = '{}'", tablepath, schema_content, schema_name_id);
        let fut = self.pool.run(move |mut connection| {
            connection.prepare(&sql).then( |r| match r {
                Ok(select) => {
                    let f = connection.query(&select, &[])
                        .collect()
                        .then(|_r| {
                            Ok((true, connection))
                        });
                    Either::A(f)
                }
                Err(e) => Either::B(err((e, connection))),
            })
        }).map_err(|err| format_err!("Postgres error {:?}", err));
        return Box::new(fut);
    }

    fn delete_schema(&self, tablepath: &str, schema_name_id: &str) -> Box<dyn Future<Item=bool, Error=Error>> {
        let sql = format!("DELETE FROM {} WHERE id = '{}'", tablepath, schema_name_id);
        let fut = self.pool.run(move |mut connection| {
            connection.prepare(&sql).then( |r| match r {
                Ok(select) => {
                    let f = connection.query(&select, &[])
                        .collect()
                        .then(|_r| {
                            Ok((true, connection))
                        });
                    Either::A(f)
                }
                Err(e) => Either::B(err((e, connection))),
            })
        }).map_err(|err| format_err!("Postgres error {:?}", err));
        return Box::new(fut);
    }

    fn add_schema(&self, tablepath: &str, schema_name_id: &str, content: &str) -> Box<dyn Future<Item=bool, Error=Error>> {
        let sql = format!("INSERT INTO {} (\"id\", \"schema\") VALUES ('{}', '{}') RETURNING \"id\"", tablepath, schema_name_id, content);
        let fut = self.pool.run(move |mut connection| {
            connection.prepare(&sql).then( |r| match r {
                Ok(select) => {
                    let f = connection.query(&select, &[])
                        .collect()
                        .then(|_r| {
                            Ok((true, connection))
                        });
                    Either::A(f)
                }
                Err(e) => Either::B(err((e, connection))),
            })
        }).map_err(|err| format_err!("Postgres error {:?}", err));
        return Box::new(fut);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tokio::runtime::current_thread::Runtime;
    use tesseract_core::{ColumnData};

    // TODO move to integration tests
    #[test]
    #[ignore]
    fn test_pg_query() {
        let postgres_db= env::var("TESSERACT_DATABASE_URL").expect("Please provide TESSERACT_DATABASE_URL");
        let pg = Postgres::new(&postgres_db);
        let future = pg.exec_sql("SELECT 1337 as hello;".to_string()).map(|df| {
            println!("Result was: {:?}", df);
            let expected_len: usize = 1;
            let val = match df.columns[0].column_data {
                ColumnData::Int32(ref internal_data) => internal_data[0],
                _ => -1
            };
            assert_eq!(df.len(), expected_len);
            assert_eq!(val, 1337);
            })
            .map_err(|err| {
               println!("Got error {:?}", err);
                ()
            });

        let mut rt = Runtime::new().unwrap();
        rt.block_on(future).unwrap();
    }
}
