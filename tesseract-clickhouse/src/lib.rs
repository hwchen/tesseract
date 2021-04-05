use anyhow::Error;
use async_trait::async_trait;
use clickhouse_rs::Pool;
use clickhouse_rs::types::Options;
use log::*;
use std::time::{Duration, Instant};
use tesseract_core::{Backend, DataFrame, QueryIr};

use regex::Regex;

mod df;
mod sql;

use self::df::block_to_df;
use self::sql::clickhouse_sql;

// Ping timeout in millis
const PING_TIMEOUT: u64 = 100_000;

#[derive(Clone)]
pub struct Clickhouse {
    pool: Pool,
}

impl Clickhouse {
    pub fn from_url(url: &str) -> Result<Self, Error> {
        let rg = Regex::new(r"(?:readonly=)(?P<id>[0-2])").unwrap();

        let options = format!("tcp://{}", url).parse::<Options>()?;

        let options = options.readonly(
            match rg.captures(url) {
                Some(readonly_option) => {
                    let rg_match = readonly_option.name("id").expect("Could not parse a value for readonly").as_str();
                    let num_match = rg_match.parse::<u8>().expect(&format!("Failed to parse {} into a numeric value", rg_match));
                    Some(num_match)
                },
                None => Some(1)
            }
        ).ping_timeout(Duration::from_millis(PING_TIMEOUT));

        let pool = Pool::new(options);

        Ok(Clickhouse {
            pool,
        })
    }
}

#[async_trait]
impl Backend for Clickhouse {
    async fn exec_sql(&self, sql: String) -> Result<DataFrame, Error> {
        let time_start = Instant::now();

        let mut client = self.pool.get_handle().await?;
        let block = client.query(&sql[..]).fetch_all().await?;
        let timing = time_start.elapsed();
        info!("Time for sql execution: {}.{:03}", timing.as_secs(), timing.subsec_millis());
        //debug!("Block: {:?}", block);

        Ok(block_to_df(block)?)
    }

    //fn exec_sql_stream(&self, sql: String) -> Box<dyn Stream<Item=Result<DataFrame, Error>, Error=Error>> {
    //    let fut_stream = self.pool
    //        .get_handle()
    //        .and_then(move |c| {
    //            future::ok(
    //                c.query(&sql[..])
    //                    .stream_blocks()
    //                    .map(move |block: Block<Simple>| {
    //                        block_to_df(block)
    //                    })
    //            )
    //        })
    //        .flatten_stream()
    //        .map_err(|err| format_err!("{}", err));

    //    Box::new(fut_stream)
    //}

    // https://users.rust-lang.org/t/solved-is-it-possible-to-clone-a-boxed-trait-object/1714/4
    fn box_clone(&self) -> Box<dyn Backend + Send + Sync> {
        Box::new((*self).clone())
    }

    fn generate_sql(&self, query_ir: QueryIr) -> String {
        clickhouse_sql(
            &query_ir
        )
    }
}

