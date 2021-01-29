use failure::{Error, format_err};
use futures::future::Future;
use tesseract_core::{Backend, DataFrame};

extern crate futures;
extern crate mysql_async as my;

mod df;
use self::df::{rows_to_df};

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

impl Backend for MySql {
    fn exec_sql(&self, sql: String) -> Box< dyn Future<Item=DataFrame, Error=Error>> {
        let future = self.pool.get_conn()
            .and_then(move |conn| {
                conn.prep_exec(sql.to_string(), ())
            })
            .map_err(|e| {
                format_err!("{}", e.description().to_string())
            })
            .and_then(|result| {
                rows_to_df(result)
            });
        Box::new(future)
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
    #[test]
    #[ignore]
    fn test_simple_query() {
        let mysql_db = env::var("MYSQL_DATABASE_URL").unwrap();
        let sql = r"SELECT 1 as example_int, 'hello' as example_name, 0.5 as example_float;";
        let mysql = MySql::new(&mysql_db);
        let r = mysql.exec_sql(sql.to_string()).wait().unwrap();
        println!("{:?}", r);
    }
}
