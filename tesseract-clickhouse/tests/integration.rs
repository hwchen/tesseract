extern crate clickhouse_rs;
extern crate tokio;

// Basic testing structured taken from
// clickhouse-rs database tests by suharev7.
use std::env;
use clickhouse_rs::Pool;

fn database_url() -> String {
    let tmp = env::var("TESSERACT_DATABASE_URL").unwrap_or_else(|_| "tcp://localhost:9000?compression=lz4".into());
    tmp.replace("clickhouse://", "tcp://")
}

#[tokio::test]
async fn test_ping() {
    // This test is meant as a sanity check
    // to ensure the docker provisioning process worked
    let pool = Pool::new(database_url());
    println!("{:?}", pool);
    let mut client = pool.get_handle().await.unwrap();
    client.ping().await.unwrap();
}

#[tokio::test]
async fn test_query() {
    #[derive(Debug, Clone, PartialEq)]
    pub struct RowResult {
        pub month_name: String,
    }

    // This test is meant as a sanity check
    // to ensure the SQL ingestion worked
    let pool = Pool::new(database_url());
    let sql = "SELECT month_name FROM tesseract_webshop_time;";
    let mut client = pool.get_handle().await.unwrap();
    let block = client.query(&sql).fetch_all().await.unwrap();

    let schema_vec: Vec<RowResult> = block.rows()
        .map(|row| {
            RowResult {
                month_name: row.get("month_name").expect("missing month_name"),
            }
        }).collect();

    assert_eq!(
        schema_vec.len(),
        12
    );
}
