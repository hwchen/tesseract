use actix_web::{
    web,
    HttpRequest,
    HttpResponse,
};
use failure::Error;
use futures::future::{self, Future};
use lazy_static::lazy_static;
use log::*;
use serde_derive::{Serialize, Deserialize};
use serde_qs as qs;

use crate::app::AppState;
use crate::logic_layer::LogicLayerConfig;

use tesseract_core::format::{format_records, FormatType};
use tesseract_core::names::LevelName;

use super::super::util::{
    boxed_error_string, boxed_error_http_response,
    verify_authorization, format_to_content_type
};


/// Handles default members query when a format is not specified.
/// Default format is CSV.
pub async fn logic_layer_members_default_handler(
    req: HttpRequest,
    state: web::Data<AppState>,
    _cube: web::Path<()>
) -> HttpResponse
{
    get_members(req, state, "jsonrecords".to_owned()).await
}


/// Handles members query when a format is specified.
pub async fn logic_layer_members_handler(
    req: HttpRequest,
    state: web::Data<AppState>,
    cube_format: web::Path<String>
) -> HttpResponse
{
    get_members(req, state, cube_format.to_owned()).await
}


/// Performs members query.
pub async fn get_members(
    req: HttpRequest,
    state: web::Data<AppState>,
    format: web::Path<String>
) -> HttpResponse
{
    let format = ok_or_404!(format.parse::<FormatType>());

    info!("Format: {:?}", format);

    let query = req.query_string();
    let schema = state.schema.read().unwrap();
    let _debug = state.debug;

    let logic_layer_config: Option<LogicLayerConfig> = match &state.logic_layer_config {
        Some(llc) => Some(llc.read().unwrap().clone()),
        None => None
    };

    lazy_static!{
        static ref QS_NON_STRICT: qs::Config = qs::Config::new(5, false);
    }

    let members_query_res = QS_NON_STRICT.deserialize_str::<MembersQueryOpt>(query);
    let members_query = ok_or_404!(members_query_res);

    let mut cube_name = members_query.cube.clone();
    let mut level_name: Option<LevelName> = None;

    // Get cube object to check for API key
    let cube_obj = ok_or_404!(schema.get_cube_by_name(&cube_name));

    if let Err(err) = verify_authorization(&req, cube_obj.min_auth_level) {
        return boxed_error_http_response(err);
    }

    if let Some(logic_layer_config) = &logic_layer_config {
        if let Some(aliases) = &logic_layer_config.aliases {
            if let Some(cubes) = &aliases.cubes {
                for cube in cubes {
                    // Cube name may change due to config substitutions
                    for alternative in &cube.alternatives {
                        if alternative == &cube_name {
                            cube_name = cube.name.clone();
                        }
                    }

                    if &cube.name == &cube_name {
                        if let Some(levels) = &cube.levels {
                            for level_prop_config in levels {
                                if level_prop_config.unique_name == members_query.level {
                                    let parsed_level_name_res: Result<LevelName, Error> = level_prop_config.current_name.clone().parse();
                                    match parsed_level_name_res {
                                        Ok(parsed_level_name) => level_name = Some(parsed_level_name),
                                        Err(_) => ()
                                    }

                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // If level name is not yet set, try to set it from a cube object by a direct match
    let level_name = match level_name {
        Some(level_name) => Some(level_name),
        None => {
            let mut level_name: Option<LevelName> = None;

            for cube in &schema.cubes {
                if &cube.name == &cube_name {
                    for dimension in &cube.dimensions {
                        for hierarchy in &dimension.hierarchies {
                            for level in &hierarchy.levels {
                                if &level.name == &members_query.level {
                                    level_name = Some(LevelName {
                                        dimension: dimension.name.clone(),
                                        hierarchy: hierarchy.name.clone(),
                                        level: level.name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            level_name
        }
    };

    let level_name = match level_name {
        Some(level_name) => level_name,
        None => return boxed_error_string("Unable to find a level with the name provided".to_string())
    };

    debug!("{:?}", cube_name);
    debug!("{:?}", level_name);

    let members_sql_and_headers = match members_query.locale {
        Some(locale) => schema.members_locale_sql(&cube_name, &level_name, &locale),
        None => schema.members_sql(&cube_name, &level_name)
    };

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

    debug!("{:?}", members_sql);
    debug!("{:?}", header);

    state
        .backend
        .exec_sql(members_sql)
        .from_err()
        .and_then(move |df| {
            let content_type = format_to_content_type(&format);

            match format_records(&header, df, format, None, false) {
                Ok(res) => Ok(HttpResponse::Ok().set(content_type).body(res)),
                Err(err) => Ok(HttpResponse::NotFound().json(err.to_string())),
            }
        })
        .responder()
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MembersQueryOpt {
    pub cube: String,
    pub level: String,
    pub locale: Option<String>,
}
