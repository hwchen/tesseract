//! DB options: For now, only one db at a time, and only
//! clickhouse or mysql
//! They're set to conflict with each other in cli opts
//!
//! Also, casting to trait object:
//! https://stackoverflow.com/questions/38294911/how-do-i-cast-a-literal-value-to-a-trait-object
//!
//! Also, it needs to be safe to send between threads, so add trait bounds
//! Send + Sync.
//! https://users.rust-lang.org/t/sending-trait-objects-between-threads/2374
//!
//! Also, it needs to be clonable to move into the closure that is
//! used to initialize actix-web, so there's a litle boilerplate
//! to implement https://users.rust-lang.org/t/solved-is-it-possible-to-clone-a-boxed-trait-object/1714/4

use anyhow::{Error, format_err};
use std::fmt;
use std::str::FromStr;

use tesseract_clickhouse::Clickhouse;
use tesseract_core::Backend;
use tesseract_mysql::MySql;
use tesseract_postgres::Postgres;

/// from a full url e.g. clickhouse://127.0.0.1:9000 returns
/// the db client, url, and database type.
///
/// Clickhouse is the default if no prefix, e.g. 127.0.0.1:9000
pub fn get_db(db_url_full: &str) -> Result<(Box<dyn Backend + Send + Sync>, String, Database), Error> {
    let db_type_url: Vec<_> = db_url_full.split("://").collect();

    let db_url = if db_type_url.len() == 1 {
        db_type_url[0]
    } else {
        db_type_url[1]
    };

    let db_type = if db_type_url.len() > 1 {
        db_type_url[0].parse()?
    } else {
        Database::Clickhouse
    };

    let db = match db_type {
        Database::Clickhouse => {
            Box::new(Clickhouse::from_url(&db_url)?) as
                Box<dyn Backend + Send + Sync>
        },
        Database::MySql => {
            Box::new(MySql::from_addr(&db_url_full)?) as
                Box<dyn Backend + Send + Sync>
        },
        Database::Postgres => {
            Box::new(Postgres::from_addr(&db_url_full)?) as
                Box<dyn Backend + Send + Sync>
        },
    };

    // Remove password when there's a user:password@host in the url
    // for display purposes only
    let db_url = match &db_url.split('@').collect::<Vec<_>>()[..] {
        [user_pass, url] => {
            match &user_pass.split(':').collect::<Vec<_>>()[..] {
                [user, _pass] => {
                    format!("{}:*@{}", user, url)
                },
                _ => db_url.to_owned(),
            }
        },
        _ => db_url.to_owned(),
    };

    Ok((db, db_url, db_type))
}

#[derive(Debug, Clone)]
pub enum Database {
    Clickhouse,
    MySql,
    Postgres,
}

impl FromStr for Database {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "clickhouse" => Ok(Database::Clickhouse),
            "mysql" => Ok(Database::MySql),
            "postgres" => Ok(Database::Postgres),
            _ => Err(format_err!("database {} not supported or not parsed", s)),
        }
    }
}

impl fmt::Display for Database {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Database::Clickhouse => write!(f, "Clickhouse"),
            Database::MySql => write!(f, "MySql"),
            Database::Postgres => write!(f, "Postgres"),
        }
    }
}

