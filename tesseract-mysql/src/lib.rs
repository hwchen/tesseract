use anyhow::Error;
use async_trait::async_trait;
use tesseract_core::{Backend, DataFrame};

use mysql_async as my;

mod df;
use self::df::rows_to_df;

use my::prelude::*;

#[derive(Clone)]
pub struct MySql {
    pool: my::Pool
}

impl MySql {
    pub fn new(address: &str) -> MySql {
        MySql { pool: my::Pool::new(address.to_string()) }
    }

    pub fn from_addr(address: &str) -> Result<Self, Error> {
        Ok(MySql::new(address))
    }
}

#[async_trait]
impl Backend for MySql {
    async fn exec_sql(&self, sql: String) -> Result<DataFrame, Error> {
        let mut conn = self.pool.get_conn().await?;
        let result = conn.query_iter(sql.to_string()).await?;
        Ok(rows_to_df(result).await?)
    }

    fn box_clone(&self) -> Box<dyn Backend + Send + Sync> {
        Box::new((*self).clone())
    }
}


#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use std::env;

    // TODO move to integration tests
    #[tokio::test]
    #[ignore]
    async fn test_simple_query() {
        let mysql_db = env::var("MYSQL_DATABASE_URL").unwrap();
        let sql = r"SELECT 1 as example_int, 'hello' as example_name, 0.5 as example_float;";
        let mysql = MySql::new(&mysql_db);
        let r = mysql.exec_sql(sql.to_string()).await.unwrap();
        println!("{:?}", r);
    }
}
