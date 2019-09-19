use actix_web::{
    AsyncResponder,
    FutureResponse,
    HttpRequest,
    HttpResponse,
    Path,
    Result as ActixResult
};

use futures::future::{self, Future};
use lazy_static::lazy_static;
use log::*;
use serde_derive::{Deserialize, Serialize};
use serde_qs as qs;
use tesseract_core::format::{format_records, FormatType};
use tesseract_core::names::LevelName;
use tesseract_core::schema::metadata::{SchemaMetadata, CubeMetadata};

use crate::app::AppState;
use crate::logic_layer::LogicLayerConfigMeta;

pub fn metadata_handler(
    (req, cube): (HttpRequest<AppState>, Path<String>)
    ) -> ActixResult<HttpResponse>
{
    info!("Metadata for cube: {}", cube);

    // currently, we do not check that cube names are distinct
    // TODO fix this

    let cube_meta = req.state()
        .schema
        .read()
        .unwrap()
        .cube_metadata(&cube);

    let ll_meta = req.state()
        .logic_layer_config
        .as_ref()
        .map(|ll| {
            let ll = &*(ll.read().unwrap());
            let res = ll.into();
            res
        });

    let meta = cube_meta.map(|c_meta| {
        CubeAllMetadata {
            core: c_meta,
            logic_layer: ll_meta,
        }
    });

    match meta {
        Some(res) => Ok(HttpResponse::Ok().json(res)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

// TODO do LL info for all metadata
pub fn metadata_all_handler(
    req: HttpRequest<AppState>
    ) -> ActixResult<HttpResponse>
{
    info!("Metadata for all");

    Ok(HttpResponse::Ok().json(req.state().schema.read().unwrap().metadata()))
}

pub fn members_default_handler(
    (req, cube): (HttpRequest<AppState>, Path<String>)
    ) -> FutureResponse<HttpResponse>
{
    let cube_format = (cube.into_inner(), "csv".to_owned());
    do_members(req, cube_format)
}

pub fn members_handler(
    (req, cube_format): (HttpRequest<AppState>, Path<(String, String)>)
    ) -> FutureResponse<HttpResponse>
{
    do_members(req, cube_format.into_inner())
}

pub fn do_members(
    req: HttpRequest<AppState>,
    cube_format: (String, String),
    ) -> FutureResponse<HttpResponse>
{
    let (cube, format) = cube_format;

    let format = format.parse::<FormatType>();
    let format = match format {
        Ok(f) => f,
        Err(err) => {
            return Box::new(
                future::result(
                    Ok(HttpResponse::NotFound().json(err.to_string()))
                )
            );
        },
    };

    let query = req.query_string();
    lazy_static!{
        static ref QS_NON_STRICT: qs::Config = qs::Config::new(5, false);
    }
    let query_res = QS_NON_STRICT.deserialize_str::<MembersQueryOpt>(&query);
    let query = match query_res {
        Ok(q) => q,
        Err(err) => {
            return Box::new(
                future::result(
                    Ok(HttpResponse::BadRequest().json(err.to_string()))
                )
            );
        },
    };

    let level: LevelName = match query.level.parse() {
        Ok(q) => q,
        Err(err) => {
            return Box::new(
                future::result(
                    Ok(HttpResponse::BadRequest().json(err.to_string()))
                )
            );
        },
    };

    info!("Members for cube: {}, level: {}", cube, level);

    let members_sql_and_headers = req.state().schema.read().unwrap()
        .members_sql(&cube, &level);
    let (members_sql, header) = match members_sql_and_headers {
        Ok(s) => s,
        Err(err) => {
            return Box::new(
                future::result(
                    Ok(HttpResponse::BadRequest().json(err.to_string()))
                )
            );
        },
    };

    req.state()
        .backend
        .exec_sql(members_sql)
        .from_err()
        .and_then(move |df| {
            match format_records(&header, df, format) {
                Ok(res) => Ok(HttpResponse::Ok().body(res)),
                Err(err) => Ok(HttpResponse::NotFound().json(err.to_string())),
            }
        })
        .responder()
}

#[derive(Debug, Deserialize)]
struct MembersQueryOpt {
    level: String,
}

#[derive(Debug, Serialize)]
struct SchemaAllMetadata {
    core: SchemaMetadata,
    logic_layer: Option<LogicLayerConfigMeta>,
}

#[derive(Debug, Serialize)]
struct CubeAllMetadata {
    core: CubeMetadata,
    logic_layer: Option<LogicLayerConfigMeta>,
}
