use actix_web::{
    http::Method,
    middleware,
    App,
    http::NormalizePath,
};
use tesseract_core::{Backend, Schema, CubeHasUniqueLevelsAndProperties};
use crate::db_config::Database;
use crate::handlers::{
    aggregate_handler,
    aggregate_default_handler,
    aggregate_stream_handler,
    aggregate_stream_default_handler,
    diagnosis_handler,
    diagnosis_default_handler,
    logic_layer_default_handler,
    logic_layer_handler,
    logic_layer_non_unique_levels_handler,
    logic_layer_non_unique_levels_default_handler,
    logic_layer_members_handler,
    logic_layer_members_default_handler,
    flush_handler,
    schema_update_handler,
    schema_add_handler,
    schema_delete_handler,
    schema_info_handler,
    index_handler,
    metadata_handler,
    metadata_all_handler,
    members_handler,
    members_default_handler,
    logic_layer_relations_handler,
    logic_layer_relations_default_handler,
    logic_layer_relations_non_unique_levels_default_handler,
    logic_layer_relations_non_unique_levels_handler
};
use crate::logic_layer::{Cache, LogicLayerConfig};

use std::sync::{Arc, RwLock};
use url::Url;


/// Holds data about the source of a schema file.
#[derive(Debug, Clone)]
pub enum SchemaSource {
    LocalSchema { filepath: String },
    DbSchema { tablepath: String, },
    #[allow(dead_code)]
    RemoteSchema { endpoint: String },
}

/// Holds a struct of environment variables that will be accessed through the `AppState`.
#[derive(Debug, Clone)]
pub struct EnvVars {
    pub database_url: String,
    pub geoservice_url: Option<Url>,
    pub schema_source: SchemaSource,
    pub jwt_secret: Option<String>,
    pub flush_secret: Option<String>,
}

/// Holds [ActixWeb State](https://actix.rs/docs/application/).
pub struct AppState {
    pub debug: bool,
    pub backend: Box<dyn Backend + Sync + Send>,
    // TODO this is a hack, until a better interface is set up with the Backend Trait
    // to generate its own sql.
    pub db_type: Database,
    pub env_vars: EnvVars,
    pub schema: Arc<RwLock<Schema>>,
    pub cache: Arc<RwLock<Cache>>,
    pub logic_layer_config: Option<Arc<RwLock<LogicLayerConfig>>>,
    // TODO is there a way to access this that's not through state? Tried using closures to
    // capture, but the handlers need to implement Fn, not FnOnce (which happens once capturing
    // variables from environment
    pub has_unique_levels_properties: CubeHasUniqueLevelsAndProperties,
}

/// Creates an ActixWeb application with an `AppState`.
pub fn create_app(
        debug: bool,
        backend: Box<dyn Backend + Sync + Send>,
        db_type: Database,
        env_vars: EnvVars,
        schema: Arc<RwLock<Schema>>,
        cache: Arc<RwLock<Cache>>,
        logic_layer_config: Option<Arc<RwLock<LogicLayerConfig>>>,
        streaming_response: bool,
        has_unique_levels_properties: CubeHasUniqueLevelsAndProperties,
    ) -> App<AppState>
{
    let app = App::with_state(
            AppState {
                debug,
                backend,
                db_type,
                env_vars,
                schema,
                cache,
                logic_layer_config,
                has_unique_levels_properties: has_unique_levels_properties.clone(),
        })
        .middleware(middleware::Logger::default())
        .middleware(middleware::DefaultHeaders::new().header("Vary", "Accept-Encoding"))

        // Metadata
        .resource("/", |r| {
            r.method(Method::GET).with(index_handler)
        })
        .resource("/cubes", |r| {
            r.method(Method::GET).with(metadata_all_handler)
        })
        .resource("/cubes/{cube}", |r| {
            r.method(Method::GET).with(metadata_handler)
        })

        // Helpers
        .resource("/cubes/{cube}/members", |r| {
            r.method(Method::GET).with(members_default_handler)
        })
        .resource("/cubes/{cube}/members.{format}", |r| {
            r.method(Method::GET).with(members_handler)
        })

        // Data Quality Assurance
        .resource("/diagnosis", |r| {
            r.method(Method::GET).with(diagnosis_default_handler)
        })
        .resource("/diagnosis.{format}", |r| {
            r.method(Method::GET).with(diagnosis_handler)
        })

        .resource("/flush", |r| {
            r.method(Method::POST).with(flush_handler)
        })
        .resource("/schema/update", |r| {
            r.method(Method::POST).with(schema_update_handler)
        })
        .resource("/schema/add", |r| {
            r.method(Method::POST).with(schema_add_handler)
        })
        .resource("/schema/delete", |r| {
            r.method(Method::POST).with(schema_delete_handler)
        })
        .resource("/schema/list", |r| {
            r.method(Method::GET).with(schema_info_handler)
        })
        // Allow the API to accept /my-path or /my-path/ for all requests
        .default_resource(|r| r.h(NormalizePath::default()));

    let app = if streaming_response {
        app
            .resource("/cubes/{cube}/aggregate", |r| {
                r.method(Method::GET).with(aggregate_stream_default_handler)
            })
            .resource("/cubes/{cube}/aggregate.{format}", |r| {
                r.method(Method::GET).with(aggregate_stream_handler)
            })
    } else {
        app
            .resource("/cubes/{cube}/aggregate", |r| {
                r.method(Method::GET).with(aggregate_default_handler)
            })
            .resource("/cubes/{cube}/aggregate.{format}", |r| {
                r.method(Method::GET).with(aggregate_handler)
            })
    };

    match has_unique_levels_properties {
        CubeHasUniqueLevelsAndProperties::True => {
            // Logic Layer
            app
                .resource("/data", |r| {
                    r.method(Method::GET).with(logic_layer_default_handler)
                })
                .resource("/data.{format}", |r| {
                    r.method(Method::GET).with(logic_layer_handler)
                })
                .resource("/members", |r| {
                    r.method(Method::GET).with(logic_layer_members_default_handler)
                })
                .resource("/members.{format}", |r| {
                    r.method(Method::GET).with(logic_layer_members_handler)
                })
                .resource("/relations", |r| {
                    r.method(Method::GET).with(logic_layer_relations_default_handler)
                })
                .resource("/relations.{foramt}", |r| {
                    r.method(Method::GET).with(logic_layer_relations_handler)
                })
        },
        CubeHasUniqueLevelsAndProperties::False { .. } => {
            // No Logic Layer, give error instead
            app
                .resource("/data", |r| {
                    r.method(Method::GET).with(logic_layer_non_unique_levels_default_handler)
                })
                .resource("/data.{format}", |r| {
                    r.method(Method::GET).with(logic_layer_non_unique_levels_handler)
                })
                .resource("/members", |r| {
                    r.method(Method::GET).with(logic_layer_non_unique_levels_default_handler)
                })
                .resource("/members.{format}", |r| {
                    r.method(Method::GET).with(logic_layer_non_unique_levels_handler)
                })
                .resource("/relations", |r| {
                    r.method(Method::GET).with(logic_layer_relations_non_unique_levels_default_handler)
                })
                .resource("/relations.{foramt}", |r| {
                    r.method(Method::GET).with(logic_layer_relations_non_unique_levels_handler)
                })
        },
    }

}
