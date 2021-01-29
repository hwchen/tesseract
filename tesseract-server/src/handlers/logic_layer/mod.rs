mod aggregate;
mod geoservice;
mod metadata;
mod relations;

pub use self::aggregate::logic_layer_handler;
pub use self::aggregate::logic_layer_default_handler;
pub use self::geoservice::GeoserviceQuery;
pub use self::geoservice::GeoServiceResponseJson;
pub use self::geoservice::query_geoservice;
pub use self::metadata::logic_layer_members_handler;
pub use self::metadata::logic_layer_members_default_handler;
pub use self::relations::logic_layer_relations_handler;
pub use self::relations::logic_layer_relations_default_handler;

use actix_web::{HttpRequest, HttpResponse, Path, ResponseError};
use crate::app::AppState;
use crate::errors::ServerError;
use tesseract_core::CubeHasUniqueLevelsAndProperties;


pub fn logic_layer_non_unique_levels_default_handler(
    (req, _cube): (HttpRequest<AppState>, Path<()>),
    ) -> HttpResponse
{
    if req.state().debug {
        // must be true, but have to de-structure again after doing it before in app.rs;
        if let CubeHasUniqueLevelsAndProperties::False { cube, name } = &req.state().has_unique_levels_properties {
            ServerError::LogicLayerDuplicateNames { cube: cube.clone(), name: name.clone() }.error_response()
        } else {
            unreachable!();
        }
    } else {
        ServerError::ErrorCode { code: "555".to_owned() }.error_response()
    }
}


pub fn logic_layer_non_unique_levels_handler(
    (req, _cube): (HttpRequest<AppState>, Path<String>),
    ) -> HttpResponse
{
    if req.state().debug {
        // must be true, but have to de-structure again after doing it before in app.rs;
        if let CubeHasUniqueLevelsAndProperties::False { cube, name } = &req.state().has_unique_levels_properties {
            ServerError::LogicLayerDuplicateNames { cube: cube.clone(), name: name.clone() }.error_response()
        } else {
            unreachable!();
        }
    } else {
        ServerError::ErrorCode { code: "555".to_owned() }.error_response()
    }
}


pub fn logic_layer_relations_non_unique_levels_default_handler(
    (req, _cube): (HttpRequest<AppState>, Path<()>),
    ) -> HttpResponse
{
    if req.state().debug {
        // must be true, but have to de-structure again after doing it before in app.rs;
        if let CubeHasUniqueLevelsAndProperties::False { cube, name } = &req.state().has_unique_levels_properties {
            ServerError::LogicLayerDuplicateNames { cube: cube.clone(), name: name.clone() }.error_response()
        } else {
            unreachable!();
        }
    } else {
        ServerError::ErrorCode { code: "555".to_owned() }.error_response()
    }
}


pub fn logic_layer_relations_non_unique_levels_handler(
    (req, _cube): (HttpRequest<AppState>, Path<String>),
    ) -> HttpResponse
{
    if req.state().debug {
        // must be true, but have to de-structure again after doing it before in app.rs;
        if let CubeHasUniqueLevelsAndProperties::False { cube, name } = &req.state().has_unique_levels_properties {
            ServerError::LogicLayerDuplicateNames { cube: cube.clone(), name: name.clone() }.error_response()
        } else {
            unreachable!();
        }
    } else {
        ServerError::ErrorCode { code: "555".to_owned() }.error_response()
    }
}
