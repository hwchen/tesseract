//! tesseract-core contains Schema;
//! Schema is stateless; it is constructed from the schema file.
//! Schema is held in the AppState struct to provide access from a route
//!
//! Each route instance will apply a tesseract_core::Query to tesseract_core::Schema to get sql.
//! The route instance then sends sql to a database and gets results back in a
//! tesseract_core::DataFrame. DataFrame is then applied to Schema to format result (jsonrecords
//! or csv).
//!
//!
//! Backend trait: exec_sql() takes in a sql string, outputs a DataFrame.
//! Because tesseract-core generates just sql (instead of taking a query and schema into a
//! `Backend`, it allows different kinds of backends to be used. Don't have to worry about
//! async/sync, which is the hardest difference to manage. Otherwise it would be easy to define
//! a `Backend` trait. (Or, can I do this and define the Backend trait as defining futures,
//! and have sync operations return a future? This might still be tricky dealing with actix,
//! it's overall easier to leave backend handling on the server side entirely).
//!
//! The database is able to be declared in the schema, each fact table and dim can be from
//! different databases. Supported: clickhouse, postgres, mysql, sqlite.

mod app;
mod db_config;
mod errors;
mod auth;
mod handlers;
mod logic_layer;
mod schema_config;

use actix_web::{
    middleware,
    web,
    App,
    HttpServer,
};
use anyhow::{format_err, Error};
use dotenv::dotenv;
use log::*;
use std::env;
use structopt::StructOpt;
use url::Url;

use std::sync::{Arc, RwLock};

use crate::app::{EnvVars, SchemaSource, config_app};
use r2d2_redis::{r2d2, RedisConnectionManager};

#[actix_web::main]
async fn main() -> Result<(), Error> {
    // Configuration

    pretty_env_logger::init();
    dotenv().ok();
    let opt = Opt::from_args();

    // debug is boolean, but env var is Result.
    // cli opt overrides env var if env_var is false
    let env_var_debug = env::var("TESSERACT_DEBUG")
        .map_err(|_| format_err!(""))
        .and_then(|d| {
             d.parse::<bool>()
            .map_err(|_| format_err!("could not parse bool from env_var TESSERACT_DEBUG"))
        });
    let debug = if !opt.debug {
        if let Ok(d) = env_var_debug {
            d
        } else {
            opt.debug // false
        }
    } else {
        opt.debug // true
    };

    // streaming http response (transfer encoding chunked)
    // cli is boolean, but env var is Result.
    // cli opt overrides env var if env_var is false
    // TODO this has the same logic as for debug. make util fn?
    let env_var_streaming_response = env::var("TESSERACT_STREAMING_RESPONSE")
        .map_err(|_| format_err!(""))
        .and_then(|d| {
             d.parse::<bool>()
            .map_err(|_| format_err!("could not parse bool from env_var TESSERACT_STREAMING_RESPONSE"))
        });
    let streaming_response = if !opt.streaming_response {
        if let Ok(d) = env_var_streaming_response {
            d
        } else {
            opt.streaming_response // false
        }
    } else {
        opt.streaming_response // true
    };

    // address
    let server_addr = opt.address.unwrap_or("127.0.0.1:7777".to_owned());

    // JSONWebToken Secret
    let jwt_secret = env::var("TESSERACT_JWT_SECRET").ok();

    // flush
    let flush_secret = env::var("TESSERACT_FLUSH_SECRET").ok();

    // Database
    let db_url_full = env::var("TESSERACT_DATABASE_URL")
        .or(opt.database_url.ok_or(format_err!("")))
        .map_err(|_| format_err!("database url not found; either TESSERACT_DATABASE_URL or cli option required"))?;

    let (db, db_url, db_type) = db_config::get_db(&db_url_full)?;
    let db_type_viz = db_type.clone();

    // Schema
    let schema_path = env::var("TESSERACT_SCHEMA_FILEPATH")
        .expect("TESSERACT_SCHEMA_FILEPATH not found");

    // Geoservice
    let geoservice_url = match env::var("TESSERACT_GEOSERVICE_URL") {
        Ok(geoservice_url) => {
            Some(Url::parse(&geoservice_url).unwrap())
        },
        Err(_) => {
            info!("Geoservice URL not provided");
            None
        }
    };

    // NOTE: Local schema is the only supported SchemaSource for now
    let schema_source = SchemaSource::LocalSchema { filepath: schema_path.clone() };

    let mut schema = schema_config::read_schema(&schema_path)?;
    schema.validate()?;
    let mut has_unique_levels_properties = schema.has_unique_levels_properties();
    let schema_arc = Arc::new(RwLock::new(schema.clone()));
    let jwt_status = if jwt_secret.is_some() {
        "ON"
    } else {
        "OFF"
    };
    // Env
    let env_vars = EnvVars {
        database_url: db_url.clone(),
        geoservice_url,
        schema_source,
        jwt_secret,
        flush_secret,
    };

    // Logic Layer Config
    let logic_layer_config = match env::var("TESSERACT_LOGIC_LAYER_CONFIG_FILEPATH") {
        Ok(config_path) => {
            match logic_layer::read_config(&config_path) {
                Ok(config_obj) => {
                    has_unique_levels_properties = config_obj.has_unique_levels_properties(&schema)?;
                    Some(config_obj)
                },
                Err(err) => return Err(err)
            }
        },
        Err(_) => None
    };

    // Populate internal cache
    let db_for_cache = db.clone(); // TODO remove clone
    let cache = logic_layer::populate_cache(
            schema.clone(), logic_layer_config.as_ref(), db_for_cache
        )
        .await.map_err(|err| format_err!("Cache population error: {}", err))?;

    // Create lock on logic layer config
    let logic_layer_config = match logic_layer_config {
        Some(ll_config) => Some(Arc::new(RwLock::new(ll_config.clone()))),
        None => None
    };

    let cache_arc = Arc::new(RwLock::new(cache));


    let redis_url = env::var("TESSERACT_REDIS_URL").ok();

    // Setup redis pool and settings if enabled by user
    let _redis_pool = match redis_url {
        Some(conn_str) => {
            let redis_connection_timeout = env::var("TESSERACT_REDIS_TIMEOUT").ok();
            let redis_max_size = env::var("TESSERACT_REDIS_MAX_SIZE").ok();

            let manager = RedisConnectionManager::new(conn_str).expect("Failed to connect to redis");
            let pool: r2d2::Pool<RedisConnectionManager> = r2d2::Pool::builder()
                .connection_timeout(if let Some(val) = redis_connection_timeout{
                    std::time::Duration::from_secs(val.parse::<u64>().expect("Invalid value for TESSERACT_REDIS_TIMEOUT"))
                } else {
                    std::time::Duration::from_secs(20) // default connection time out 10 seconds
                })
                .max_size(if let Some(rms_val) = redis_max_size{
                    rms_val.parse::<u32>().expect("Invalid value for TESSERACT_REDIS_MAX_SIZE")
                } else {
                    25 // default max size 25
                })
                .build(manager)
            .expect("Failed to connect to redis server. Is it running?");
            Some(pool)
        },
        None => None,
    };

    // Initialize Server
    HttpServer::new(move || {
        App::new()
            .configure(|cfg: &mut web::ServiceConfig| {
                config_app(
                    cfg,
                    debug,
                    db.clone(),
                    None, // redis_pool
                    db_type.clone(),
                    env_vars.clone(),
                    schema_arc.clone(),
                    cache_arc.clone(),
                    logic_layer_config.clone(),
                    streaming_response,
                    has_unique_levels_properties.clone(),
                )
            })
        .wrap(middleware::Logger::default())
        .wrap(middleware::DefaultHeaders::new().header("Vary", "Accept-Encoding"))
    })
    .bind(&server_addr)?
    .run()
    .await?;

    println!("Tesseract listening on: {}", server_addr);
    println!("Tesseract database:     {}, {}", db_url, db_type_viz);
    println!("Tesseract schema path:  {}", schema_path);

    println!("Tesseract JWT token protection: {}", jwt_status);

    if debug {
        println!("Tesseract debug mode: ON");
    }
    if streaming_response {
        println!("Tesseract streaming mode: ON");
    }

    Ok(())
}


/// CLI arguments helper.
#[derive(Debug, StructOpt)]
#[structopt(name="tesseract")]
struct Opt {
    #[structopt(short="a", long="addr")]
    address: Option<String>,

    #[structopt(long="db-url")]
    database_url: Option<String>,

    #[structopt(long="geoservice-url")]
    geoservice_url: Option<String>,

    #[structopt(long="debug")]
    debug: bool,

    #[structopt(long="streaming")]
    streaming_response: bool,
}
