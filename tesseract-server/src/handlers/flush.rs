use serde_derive::{Serialize, Deserialize};

use lazy_static::lazy_static;
use log::*;
use serde_qs as qs;

use actix_web::{
    web,
    HttpRequest,
    HttpResponse,
    Result as ActixResult,
};

use crate::app::{AppState, SchemaSource};
use crate::schema_config;


#[derive(Debug, Deserialize, Serialize)]
pub struct FlushQueryOpt {
    pub secret: String,
}

pub async fn flush_handler(req: HttpRequest, state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let query = req.query_string();

    lazy_static!{
        static ref QS_NON_STRICT: qs::Config = qs::Config::new(5, false);
    }

    let query_res = QS_NON_STRICT.deserialize_str::<FlushQueryOpt>(&query);
    let query = match query_res {
        Ok(q) => q,
        Err(err) => {
            return Ok(HttpResponse::BadRequest().json(err.to_string()));
        },
    };

    let db_secret = match &state.env_vars.flush_secret {
        Some(db_secret) => db_secret,
        None => { return Ok(HttpResponse::Unauthorized().finish()); }
    };

    if query.secret == *db_secret {
        info!("Flush internal state");

        // Read schema again
        // NOTE: This logic will change once we start supporting remote schemas
        let schema_path = match &state.env_vars.schema_source {
            SchemaSource::LocalSchema { ref filepath } => filepath,
            SchemaSource::RemoteSchema { ref endpoint } => endpoint,
        };

        let schema = match schema_config::read_schema(&schema_path) {
            Ok(val) => val,
            Err(err) => {
                error!("{}", err);
                return Ok(HttpResponse::InternalServerError().finish());
            },
        };

        // Update shared schema
        let mut w = state.schema.write().unwrap();
        *w = schema.clone();

        // TODO: Uncomment when issue with SystemRunner is solved
//        // Re-populate cache with the new schema
//        let cache = match populate_cache(schema, state.backend.clone()) {
//            Ok(cache) => cache,
//            Err(err) => {
//                error!("{}", err);
//                return Ok(HttpResponse::InternalServerError().finish());
//            },
//        };
//
//        // Update shared cache
//        let mut w = state.cache.write().unwrap();
//        *w = cache;

        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}
