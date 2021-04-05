use std::str;

use actix_web::{
    web,
    HttpRequest,
    HttpResponse,
    Result as ActixResult,
};
use anyhow::Error;
use lazy_static::lazy_static;
use log::*;
use serde_qs as qs;
use serde_derive::Deserialize;

use tesseract_core::format::{format_records, FormatType};
use tesseract_core::{DataFrame, Column, ColumnData};
use tesseract_core::schema::{Cube, Level};
use crate::app::AppState;
use crate::handlers::util::{verify_authorization, format_to_content_type};


/// Handles default aggregation when a format is not specified.
/// Default format is jsonrecords.
pub async fn diagnosis_default_handler(
    req: HttpRequest,
    state: web::Data<AppState>,
    _cube: web::Path<()>,
) -> ActixResult<HttpResponse>
{
    perform_diagnosis(req, state, "jsonrecords".to_owned()).await
}


/// Handles aggregation when a format is specified.
pub async fn diagnosis_handler(
    req: HttpRequest,
    state: web::Data<AppState>,
    cube_format: web::Path<String>,
) -> ActixResult<HttpResponse>
{
    perform_diagnosis(req, state, cube_format.to_owned()).await
}


#[derive(Debug, Clone, Deserialize)]
pub struct DiagnosisQueryOpt {
    pub cube: Option<String>,
}


pub async fn perform_diagnosis(
    req: HttpRequest,
    state: web::Data<AppState>,
    format: String,
) -> ActixResult<HttpResponse>
{
    let format = format.parse::<FormatType>();
    let format = match format {
        Ok(f) => f,
        Err(err) =>return Ok(HttpResponse::NotFound().json(err.to_string())),
    };

    info!("Format: {:?}", format);

    let query = req.query_string();
    let schema = state.schema.read().unwrap();
    let _debug = state.debug;

    lazy_static! {
        static ref QS_NON_STRICT: qs::Config = qs::Config::new(5, false);
    }

    let query_opt = match QS_NON_STRICT.deserialize_str::<DiagnosisQueryOpt>(query) {
        Ok(q) => q,
        Err(err) => return Ok(HttpResponse::NotFound().json(err.to_string()))
    };

    // If a cube name was provided, we try to match that,
    // otherwise we will diagnose all cubes this user has access to
    match query_opt.cube {
        Some(cube_name) => {
            match schema.get_cube_by_name(&cube_name) {
                Ok(cube) => {
                    if let Err(err) = verify_authorization(&req, &state, cube.min_auth_level) {
                        return Ok(err);
                    }

                    let (error_types, error_messages) = diagnose_cube(&req, &state, cube).await;

                    format_diagnosis_response(error_types, error_messages, format, None)
                },
                Err(err) => return Ok(HttpResponse::NotFound().json(err.to_string()))
            }
        },
        None => {
            let mut error_cubes: Vec<String> = vec![];
            let mut error_types: Vec<String> = vec![];
            let mut error_messages: Vec<String> = vec![];

            for cube in &schema.cubes {
                if let Err(err) = verify_authorization(&req, &state, cube.min_auth_level) {
                    continue;
                }

                let (new_error_types, new_error_messages) = diagnose_cube(&req, &state, &cube).await;

                // Add these to the overall list
                if new_error_types.len() != 0 {
                    let mut new_error_cubes: Vec<String> = vec![];

                    for i in 0..new_error_types.len() {
                        new_error_cubes.push(cube.name.clone());
                    }

                    error_cubes.extend(new_error_cubes);
                    error_types.extend(new_error_types);
                    error_messages.extend(new_error_messages);
                }
            }

            format_diagnosis_response(error_types, error_messages, format, Some(error_cubes))
        }
    }
}


async fn diagnose_cube(req: &HttpRequest, state: &web::Data<AppState>, cube: &Cube) -> (Vec<String>, Vec<String>) {
    let mut error_types: Vec<String> = vec![];
    let mut error_messages: Vec<String> = vec![];

    for dimension in &cube.dimensions {
        for hierarchy in &dimension.hierarchies {
            let last_level: &Level = &hierarchy.levels[hierarchy.levels.len() - 1];

            if let Some(ref foreign_key) = dimension.foreign_key {
                if let Some(ref dimension_table) = hierarchy.table {
                    // Check for `MissingDimensionIDs`
                    // TODO: Deal with the case where there is an inline table.
                    let sql_str: String = format!(
                        "SELECT DISTINCT {} FROM {} WHERE {} NOT IN (SELECT {} FROM {})",
                        foreign_key,
                        cube.table.name,
                        foreign_key,
                        last_level.key_column,
                        dimension_table.name,
                    );

                    match get_res_df(&req, &state, sql_str).await {
                        Ok(res_df) => {
                            match res_df.columns.get(0) {
                                Some(column) => {
                                    // This is used here as a hack to extract the data inside
                                    // the enum as a vector instead of using a match on every
                                    // single data type supported.
                                    let column_data = column.stringify_column_data();

                                    if column_data.len() > 0 {
                                        error_types.push("MissingDimensionIDs".to_string());
                                        error_messages.push(
                                            format!(
                                                "The following IDs for [{}].[{}].[{}] are not present in its dimension table: {}.",
                                                dimension.name,
                                                hierarchy.name,
                                                last_level.name,
                                                column_data.join(", ")
                                            )
                                        );
                                    }
                                },
                                None => ()
                            }
                        },
                        Err(_) => ()
                    }

                    // Check for `NonUniqueDimensionIDs`
                    // TODO: Deal with the case where there is an inline table.
                    let sql_str: String = format!(
                        "SELECT {} FROM (select {}, count(*) as unique_count FROM {} GROUP BY {}) where unique_count > 1",
                        last_level.key_column,
                        last_level.key_column,
                        dimension_table.name,
                        last_level.key_column,
                    );

                    match get_res_df(&req, &state, sql_str).await {
                        Ok(res_df) => {
                            match res_df.columns.get(0) {
                                Some(column) => {
                                    let column_data = column.stringify_column_data();

                                    if column_data.len() > 0 {
                                        error_types.push("NonUniqueDimensionIDs".to_string());
                                        error_messages.push(
                                            format!(
                                                "There are duplicate entries for the following IDs in the {} dimension table: {}.",
                                                dimension_table.name,
                                                column_data.join(", ")
                                            )
                                        );
                                    }
                                },
                                None => ()
                            }
                        },
                        Err(_) => ()
                    }
                }
            }
        }
    }

    (error_types, error_messages)
}


fn format_diagnosis_response(
        error_types: Vec<String>,
        error_messages: Vec<String>,
        format: FormatType,
        error_cubes: Option<Vec<String>>
) -> ActixResult<HttpResponse> {
    if error_messages.len() == 0 {
        Ok(HttpResponse::Ok().json("Success.".to_string()))
    } else {
        let headers = if error_cubes.is_some() {
            vec!["cube".to_string(), "type".to_string(), "message".to_string()]
        } else {
            vec!["type".to_string(), "message".to_string()]
        };

        let df = match error_cubes {
            Some(error_cubes) => {
                DataFrame { columns: vec![
                    Column {
                        name: "cube".to_string(),
                        column_data: ColumnData::Text(error_cubes)
                    },
                    Column {
                        name: "type".to_string(),
                        column_data: ColumnData::Text(error_types)
                    },
                    Column {
                        name: "message".to_string(),
                        column_data: ColumnData::Text(error_messages)
                    }
                ] }
            },
            None => {
                DataFrame { columns: vec![
                    Column {
                        name: "type".to_string(),
                        column_data: ColumnData::Text(error_types)
                    },
                    Column {
                        name: "message".to_string(),
                        column_data: ColumnData::Text(error_messages)
                    }
                ] }
            }
        };

        let content_type = format_to_content_type(&format);

        match format_records(&headers, df, format, None, true) {
            Ok(res) => {
                Ok(HttpResponse::ExpectationFailed()
                    .content_type(content_type)
                    .body(res))
            },
            Err(err) => Ok(HttpResponse::NotFound().json(err.to_string())),
        }
    }
}


async fn get_res_df(req: &HttpRequest, state: &web::Data<AppState>, sql_str: String) -> Result<DataFrame, Error> {
    state.backend.exec_sql(sql_str).await
}
