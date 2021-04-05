use anyhow::Error;
use async_trait::async_trait;
use tesseract_core::{Backend, DataFrame};
use tokio_postgres::NoTls;

use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;

mod df;
use self::df::rows_to_df;

#[derive(Clone)]
pub struct Postgres {
    db_url: String,
    pool: Pool<PostgresConnectionManager<NoTls>>
}

impl Postgres {
    pub fn new(address: &str) -> Postgres {
        let pg_mgr: PostgresConnectionManager<NoTls> = PostgresConnectionManager::new(address.parse().unwrap(), tokio_postgres::NoTls);
        let future = Pool::builder().build(pg_mgr);

        // synchronously setup pg pool
        // TODO Pool should just be initialized outside and passed into this constructor, to avoid
        // having to create another runtime. Or this fn can simply be async.
        let rt = tokio::runtime::Builder::new_current_thread().build().expect("Unable to create a runtime");
        let pool = rt.block_on(future).unwrap();

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

#[async_trait]
impl Backend for Postgres {
    async fn exec_sql(&self, sql: String) -> Result<DataFrame, Error> {
        let connection = self.pool.get().await?;
        let statement = connection.prepare(&sql).await?;
        let rows = connection.query(&statement, &[]).await?;
        let df = rows_to_df(rows, statement.columns());
        Ok(df)
    }

    fn box_clone(&self) -> Box<dyn Backend + Send + Sync> {
        Box::new((*self).clone())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tesseract_core::ColumnData;

    // TODO move to integration tests
    #[tokio::test]
    #[ignore]
    async fn test_pg_query() {
        let postgres_db= env::var("TESSERACT_DATABASE_URL").expect("Please provide TESSERACT_DATABASE_URL");
        let pg = Postgres::new(&postgres_db);
        let df = pg.exec_sql("SELECT 1337 as hello;".to_string()).await.unwrap();
        println!("Result was: {:?}", df);
        let expected_len: usize = 1;
        let val = match df.columns[0].column_data {
            ColumnData::Int32(ref internal_data) => internal_data[0],
            _ => -1
        };
        assert_eq!(df.len(), expected_len);
        assert_eq!(val, 1337);
    }
}
