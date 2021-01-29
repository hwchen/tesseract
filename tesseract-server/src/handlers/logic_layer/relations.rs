use std::collections::HashMap;
use std::str;

use actix_web::{
    HttpRequest,
    HttpResponse,
    Path,
    Result as ActixResult,
};
use failure::{Error, format_err};
use lazy_static::lazy_static;
use log::*;
use serde_qs as qs;
use serde_derive::Deserialize;
use url::Url;

use tesseract_core::names::{Property, LevelName};
use tesseract_core::format::{format_records, FormatType};
use tesseract_core::{DataFrame, Column, ColumnData};
use tesseract_core::schema::{Cube, DimensionType};
use crate::app::AppState;
use crate::logic_layer::{LogicLayerConfig, CubeCache};
use super::super::util::{verify_authorization, format_to_content_type};
use crate::handlers::logic_layer::{query_geoservice, GeoserviceQuery};


/// Handles default aggregation when a format is not specified.
/// Default format is jsonrecords.
pub fn logic_layer_relations_default_handler(
    (req, _cube): (HttpRequest<AppState>, Path<()>)
) -> ActixResult<HttpResponse>
{
    logic_layer_relations(req, "jsonrecords".to_owned())
}


/// Handles aggregation when a format is specified.
pub fn logic_layer_relations_handler(
    (req, cube_format): (HttpRequest<AppState>, Path<String>)
) -> ActixResult<HttpResponse>
{
    logic_layer_relations(req, cube_format.to_owned())
}


#[derive(Debug, Clone, Deserialize)]
pub struct LogicLayerRelationQueryOpt {
    pub cube: String,
    #[serde(flatten)]
    pub cuts: HashMap<String, String>,
    debug: Option<bool>,
}


pub fn logic_layer_relations(
    req: HttpRequest<AppState>,
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
    let schema = req.state().schema.read().unwrap();
    let _debug = req.state().debug;

    lazy_static! {
        static ref QS_NON_STRICT: qs::Config = qs::Config::new(5, false);
    }

    let agg_query = match QS_NON_STRICT.deserialize_str::<LogicLayerRelationQueryOpt>(query) {
        Ok(q) => q,
        Err(err) => return Ok(HttpResponse::NotFound().json(err.to_string()))
    };

    let logic_layer_config: Option<LogicLayerConfig> = match &req.state().logic_layer_config {
        Some(llc) => Some(llc.read().unwrap().clone()),
        None => None
    };

    let cube_name = match logic_layer_config.clone() {
        Some(llc) => {
            match llc.substitute_cube_name(agg_query.cube.clone()) {
                Ok(cn) => cn,
                Err(_) => agg_query.cube.clone()
            }
        },
        None => agg_query.cube.clone()
    };

    let cube = match schema.get_cube_by_name(&cube_name) {
        Ok(c) => c,
        Err(err) => return Ok(HttpResponse::NotFound().json(err.to_string()))
    };

    if let Err(err) = verify_authorization(&req, cube.min_auth_level) {
        return Ok(err);
    }

    let cache = req.state().cache.read().unwrap();

    let cube_cache = match cache.find_cube_info(&cube_name) {
        Some(cube_cache) => cube_cache,
        None => return Ok(HttpResponse::NotFound().json("Unable to access cube cache".to_string()))
    };

    let cuts_map = agg_query.cuts;

    let level_map = &cube_cache.level_map;
    let property_map = &cube_cache.property_map;
    let geoservice_url = &req.state().env_vars.geoservice_url;

    let dimensions_map: Vec<Vec<String>> = match get_relations(&cuts_map, &cube, &cube_cache, &level_map, &property_map, &geoservice_url) {
        Ok(dm) => dm,
        Err(err) => return Ok(HttpResponse::NotFound().json(err.to_string())),
    };

    let final_headers: Vec<String> = ["level".to_string(), "id".to_string(), "relation".to_string(), "value".to_string()].to_vec();
    let mut final_columns: Vec<Column> = vec![];

    let mut col_0: Vec<String> = Vec::new();
    let mut col_1: Vec<String> = Vec::new();
    let mut col_2: Vec<String> = Vec::new();
    let mut col_3: Vec<String> = Vec::new();

    for row in dimensions_map {
        col_0.push(row.get(0).unwrap().to_string());
        col_1.push(row.get(1).unwrap().to_string());
        col_2.push(row.get(2).unwrap().to_string());
        col_3.push(row.get(3).unwrap().to_string());
    }

    final_columns.push(Column {
        name: "level".to_string(),
        column_data: ColumnData::Text(col_0)
    });
    final_columns.push(Column {
        name: "id".to_string(),
        column_data: ColumnData::Text(col_1)
    });
    final_columns.push(Column {
        name: "relation".to_string(),
        column_data: ColumnData::Text(col_2)
    });
    final_columns.push(Column {
        name: "value".to_string(),
        column_data: ColumnData::Text(col_3)
    });

    let final_df = DataFrame { columns: final_columns };

    let content_type = format_to_content_type(&format);

    match format_records(&final_headers, final_df, format, None, false) {
        Ok(res) => {
            Ok(HttpResponse::Ok()
                .set(content_type)
                .body(res))
        },
        Err(err) => Ok(HttpResponse::NotFound().json(err.to_string())),
    }
}


pub fn get_relations(
    cuts_map: &HashMap<String, String>,
    cube: &Cube,
    cube_cache: &CubeCache,
    level_map: &HashMap<String, LevelName>,
    _property_map: &HashMap<String, Property>,
    geoservice_url: &Option<Url>
) -> Result<Vec<Vec<String>>, Error> {

    if cuts_map.len() == 0 {
        return Err(format_err!("Please provide at least one cut"));
    }

    let mut relations: Vec<Vec<String>> = vec![];

    let mut level_matches: Vec<LevelName> = vec![];

    for (cut_key, cut_values) in cuts_map.iter() {
        if cut_values.is_empty(){
            continue;
        }
        let element:Vec<String> = cut_values.split(":").map(|s| s.to_string()).collect();
        let cut_key_values:Vec<String> = match element.get(0){
            Some(ckv) => ckv.split(",").map(|s| s.to_string()).collect(),
            None => continue,
        };
        let operations:Vec<String> = match element.get(1) {
            Some(op) => op.split(",").map(|s| s.to_string()).collect(),
            None => continue,
        };
        for cut in cut_key_values {
            for op in &operations {
                let mut level_name = match cube_cache.dimension_caches.get(cut_key) {
                    Some(dimension_cache) => {
                        match dimension_cache.id_map.get(&cut) {
                            Some(level_name) => {
                                if level_name.len() > 1 {
                                    return Err(format_err!("{} matches multiple levels in this dimension.", cut))
                                }
                                match level_name.get(0) {
                                    Some(ln) => ln.clone(),
                                    None => return Err(format_err!("{} matches no levels in this dimension.", cut))
                                }
                            },
                            None => continue
                        }
                    },
                    None => {
                        match level_map.get(cut_key) {
                            Some(level_name) => {
                                level_matches.push(level_name.clone());
                                level_name.clone()
                            },
                            None => continue,
                        }
                    }
                };

                if op.to_string() == "children".to_string() {
                    let child_level = match cube.get_child_level(&level_name)? {
                        Some(child_level) => child_level,
                        None => continue  // This level has no child
                    };

                    let _child_level_name = LevelName {
                        dimension: level_name.dimension.clone(),
                        hierarchy: level_name.hierarchy.clone(),
                        level: child_level.name.clone()
                    };


                    // Get children IDs from the cache
                    let level_cache = match cube_cache.level_caches.get(&level_name) {
                        Some(level_cache) => level_cache,
                        None => return Err(format_err!("Could not find cached entries for {}.", level_name.level))
                    };

                    let children_ids = match &level_cache.children_map {
                        Some(children_map) => {
                            match children_map.get(&cut) {
                                Some(children_ids) => children_ids.clone(),
                                None => continue
                            }
                        },
                        None => continue
                    };

                    for children_id in children_ids.iter() {
                        relations.push([cut_key.to_string(), cut.to_string(), "child".to_string(), children_id.to_string()].to_vec())
                    }

                }
                else if op.to_string() == "parents".to_string() {
                    let mut parent_entries: Vec<Vec<String>> = vec![];

                    let parent_levels = cube.get_level_parents(&level_name)?;

                    if parent_levels.is_empty() {
                        // This level has no parents
                        continue;
                    }

                    let mut search_id = cut.clone();

                    for parent_level in (parent_levels.iter()).rev() {
                        let parent_level_name = LevelName {
                            dimension: level_name.dimension.clone(),
                            hierarchy: level_name.hierarchy.clone(),
                            level: parent_level.name.clone()
                        };

                        // Get parent IDs from the cache
                        let level_cache = match cube_cache.level_caches.get(&level_name) {
                            Some(level_cache) => level_cache,
                            None => return Err(format_err!("Could not find cached entries for {}.", level_name.level))
                        };

                        let parent_id = match &level_cache.parent_map {
                            Some(parent_map) => {
                                match parent_map.get(&search_id) {
                                    Some(parent_id) => parent_id.clone(),
                                    None => continue
                                }
                            },
                            None => continue
                        };

                        parent_entries.push([cut_key.to_string(), cut.to_string(), "parent".to_string(), parent_id.to_string()].to_vec());

                        // Update current level_name for the next iteration
                        level_name = parent_level_name.clone();

                        // The search_id in the next iteration will be the current parent
                        search_id = parent_id;
                    }

                    // Reverse the parent_entries vector so that parent levels
                    // are returned as they appear in the hierarchy
                    parent_entries.reverse();

                    relations.extend(parent_entries);
                }
                else if op.to_string() == "neighbors".to_string() {
                    // Find dimension for the level name
                    let dimension = cube.get_dimension(&level_name)
                        .ok_or_else(|| format_err!("Could not find dimension for {}.", level_name.level))?;

                    match dimension.dim_type {
                        DimensionType::Geo => {
                            match geoservice_url {
                                Some(geoservice_url) => {
                                    let mut neighbors_ids: Vec<String> = vec![];

                                    let geoservice_response = query_geoservice(
                                        geoservice_url, &GeoserviceQuery::Neighbors, &cut
                                    )?;

                                    for res in &geoservice_response {
                                        neighbors_ids.push(res.geoid.clone());
                                    }

                                    for neighbor_id in neighbors_ids.iter() {
                                        relations.push([cut_key.to_string(), cut.to_string(), "neighbor".to_string(), neighbor_id.to_string()].to_vec());
                                    }

                                },
                                None => return Err(format_err!("Unable to perform geoservice request: A Geoservice URL has not been provided."))
                            };
                        },
                        _ => {
                            let level_cache = match cube_cache.level_caches.get(&level_name) {
                                Some(level_cache) => level_cache,
                                None => return Err(format_err!("Could not find cached entries for {}.", level_name.level))
                            };

                            let neighbors_ids = match level_cache.neighbors_map.get(&cut) {
                                Some(neighbors_ids) => neighbors_ids.clone(),
                                None => continue
                            };

                            for neighbor_id in neighbors_ids.iter() {
                                relations.push([cut_key.to_string(), cut.to_string(), "neighbor".to_string(), neighbor_id.to_string()].to_vec());
                            }
                        }
                    }
                }
                else { return Err(format_err!("Unrecognized operation: `{}`.", op));
                }
            }
        }
    }

    Ok(relations)
}
