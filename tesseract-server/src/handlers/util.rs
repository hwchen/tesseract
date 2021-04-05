use anyhow::{bail, format_err, Error};
use std::collections::HashMap;
use actix_web::{
    web,
    HttpRequest,
    HttpResponse,
};
use actix_web::http::header::ContentType;
use log::*;
use mime;
use r2d2_redis::{r2d2, redis, RedisConnectionManager};

use tesseract_core::format::FormatType;
use tesseract_core::schema::Cube;
use tesseract_core::schema::metadata::SourceMetadata;

use crate::app::AppState;

use tesseract_core::names::Cut;
use crate::logic_layer::CubeCache;
use crate::auth::{validate_web_token, extract_token, user_auth_level};

pub(crate) fn format_to_content_type(format_type: &FormatType) -> ContentType {
    match format_type {
        FormatType::Csv => ContentType(mime::TEXT_CSV_UTF_8),
        FormatType::JsonRecords => ContentType(mime::APPLICATION_JSON),
        FormatType::JsonArrays => ContentType(mime::APPLICATION_JSON),
    }
}

// Generates the source data/annotation of the cube for which the query is executed
pub fn generate_source_data(cube: &Cube) -> SourceMetadata {
    let cube_name = &cube.name;
    let mut measures = Vec::new();
    for measure in cube.measures.iter() {
        measures.push(measure.name.clone());
    }
    let annotations = match cube.annotations.clone(){
        Some(annotations) => {
            let mut anotate_hashmap = HashMap::new();
            for annotation in annotations.iter(){
                anotate_hashmap.insert(annotation.name.to_string(), annotation.text.to_string());
            }
            Some(anotate_hashmap)
        },
        None => None
    };
    SourceMetadata {
        name: cube_name.clone(),
        measures: measures.clone(),
        annotations: annotations.clone(),
    }
}

pub fn get_user_auth_level(req: &HttpRequest, state: &web::Data<AppState>) -> Option<i32> {
    let jwt_secret = &state.env_vars.jwt_secret;
    let user_token = extract_token(req);
    user_auth_level(jwt_secret, &user_token)
}

pub fn verify_authorization(req: &HttpRequest, state: &web::Data<AppState>, min_auth_level: i32) -> Result<(), HttpResponse> {
    let jwt_secret = &state.env_vars.jwt_secret;
    let user_token = extract_token(req);
    if !validate_web_token(jwt_secret, &user_token, min_auth_level) {
        return Err(HttpResponse::Unauthorized().json("This cube is not public".to_string()));
    }

    Ok(())
}


#[macro_export]
macro_rules! ok_or_400 {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                return Ok(HttpResponse::BadRequest().json(err.to_string()));
            }
        }
    };
}


#[macro_export]
macro_rules! ok_or_404 {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                return Ok(HttpResponse::NotFound().json(err.to_string()));
            }
        }
    };
}


#[macro_export]
macro_rules! some_or_404 {
    ($expr:expr, $note:expr) => {
        match $expr {
            Some(val) => val,
            None => {
                return Ok(HttpResponse::NotFound().json($note.to_string()));
            }
        }
    };
}

#[macro_export]
macro_rules! ok_or_500 {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                return Ok(HttpResponse::InternalServerError().json(err.to_string()));
            }
        }
    };
}

pub fn validate_members(cuts: &[Cut], cube_cache: &CubeCache) -> Result<(), Error> {
    for cut in cuts {
        // get level cache
        let member_cache = cube_cache.members_for_level(&cut.level_name)
            .ok_or_else(|| format_err!("Level not found in cache"))?;
        for member in &cut.members {
            if !member_cache.contains(member) {
                bail!("Cut member not found");
            }
        }
    }
    Ok(())
}


///// Gets the Redis cache key for a given query.
///// The sorting of query param keys is an attempt to increase cache hits.
//pub fn get_redis_cache_key(prefix: &str, req: &HttpRequest, state: web::Data<AppState>, cube: &str, format: &FormatType) -> String {
//    let mut qry = req.query().clone();
//    qry.remove("x-tesseract-jwt-token");
//
//    let mut qry_keys: Vec<(String, String)> = qry.into_iter().collect();
//    qry_keys.sort_by(|x, y| {x.0.cmp(&y.0)});
//
//    let qry_strings: Vec<String> = qry_keys.iter()
//        .map(|x| {
//            format!("{}={}", x.0, x.1)
//        })
//        .collect();
//
//    let format_str = match format {
//        FormatType::Csv => "csv",
//        FormatType::JsonArrays => "jsonarrays",
//        FormatType::JsonRecords => "jsonrecords",
//    };
//
//    format!("{}/{}/{}/{}", prefix, cube, format_str, qry_strings.join("&"))
//}
//
//
///// Checks if the current query is already cached in Redis.
//pub async fn check_redis_cache(
//        format: &FormatType,
//        redis_pool: &Option<r2d2::Pool<RedisConnectionManager>>,
//        redis_cache_key: &str
//) -> Option<HttpResponse> {
//    if let Some(rpool) = redis_pool {
//        let conn_result = rpool.get();
//
//        if let Ok(mut conn) = conn_result {
//            let redis_cache_result = redis::cmd("GET").arg(redis_cache_key).query(&mut *conn);
//
//            if let Ok(result_str) = redis_cache_result {
//                let result_str: &String = &result_str;
//                let content_type = format_to_content_type(&format);
//                let response = HttpResponse::Ok()
//                    .content_type(content_type)
//                    .body(result_str);
//
//                return Some(Box::new(future::result(Ok(response))));
//            }
//        } else {
//            debug!("Failed to get redis pool handle!");
//        }
//        // Cache miss!
//    }
//
//    None
//}


///// Inserts a new entry into the Redis cache.
//pub fn insert_into_redis_cache(
//    res: &str,
//    redis_pool: &Option<r2d2::Pool<RedisConnectionManager>>,
//    redis_cache_key: &str
//) {
//    if let Some(rpool) = redis_pool {
//        if let Ok(mut conn) = rpool.get() {
//            let rs: redis::RedisResult<String> = redis::cmd("SET").arg(redis_cache_key).arg(res).query(&mut *conn);
//            if rs.is_err() {
//                debug!("Error occurred when trying to save key: {}", redis_cache_key);
//            }
//        }
//    }
//}
