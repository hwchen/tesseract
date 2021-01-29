use failure::Error;
use futures::{Future, Stream};

use crate::dataframe::DataFrame;
use crate::query_ir::QueryIr;
use crate::sql;


pub trait Backend {
    /// Takes in a SQL string, outputs a DataFrame, which will go on to be formatted into the
    /// desired query output format.
    fn exec_sql(&self, sql: String) -> Box<dyn Future<Item=DataFrame, Error=Error>>;

    /// Takes in a SQL string, outputs a stream of
    /// DataFrames, which will go on to be formatted into the
    /// desired query output format.
    fn exec_sql_stream(&self, _sql: String) -> Box<dyn Stream<Item=Result<DataFrame, Error>, Error=Error>> {
        unimplemented!()
    }

    fn box_clone(&self) -> Box<dyn Backend + Send + Sync>;

    /// Receives an intermediate representation of the Query
    /// (the table, col, and relationship info needed for each drill,
    /// mea, cut, etc.) and generates a `String` of sql. Cannot error,
    /// and all checks should be done before calling this.
    fn generate_sql(&self, query_ir: QueryIr) -> String {
        // standard sql implementation
        sql::standard_sql(
            &query_ir.table,
            &query_ir.cuts,
            &query_ir.drills,
            &query_ir.meas,
            &query_ir.top,
            &query_ir.sort,
            &query_ir.limit,
            &query_ir.rca,
            &query_ir.growth,
        )
    }
}

impl Clone for Box<dyn Backend + Send + Sync> {
    fn clone(&self) -> Box<dyn Backend + Send + Sync> {
        self.box_clone()
    }
}
