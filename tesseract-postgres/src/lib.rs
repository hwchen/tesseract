use failure::{Error, format_err};
use tesseract_core::{Backend, DataFrame};
use futures::{Future, Stream};
use tokio_postgres::NoTls;
extern crate futures;
extern crate tokio_postgres;
extern crate bb8;
extern crate bb8_postgres;
extern crate futures_state_stream;
extern crate tokio;

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
    fn exec_sql(&self, sql: String) -> Box< dyn Future<Item=DataFrame, Error=Error>> {
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
