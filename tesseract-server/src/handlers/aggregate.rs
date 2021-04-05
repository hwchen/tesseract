use actix_web::{
    web,
    HttpRequest,
    HttpResponse,
    Result as ActixResult,
};
use anyhow::Error;

use lazy_static::lazy_static;
use log::*;
use serde_derive::{Serialize, Deserialize};
use serde_qs as qs;
use std::convert::{TryFrom, TryInto};
use tesseract_core::format::{format_records, FormatType};
use tesseract_core::Query as TsQuery;

use crate::handlers::util::validate_members;

use crate::app::AppState;
use crate::errors::ServerError;
use super::util::{
    verify_authorization,
    format_to_content_type, generate_source_data,
    //get_redis_cache_key, check_redis_cache, insert_into_redis_cache
};

/// Handles default aggregation when a format is not specified.
/// Default format is CSV.
pub async fn aggregate_default_handler(
    req: HttpRequest,
    state: web::Data<AppState>,
    cube: web::Path<String>,
    ) -> ActixResult<HttpResponse>
{
    let cube_format = (cube.into_inner(), "csv".to_owned());
    do_aggregate(req, state, cube_format).await
}


/// Handles aggregation when a format is specified.
pub async fn aggregate_handler(
    req: HttpRequest,
    state: web::Data<AppState>,
    cube_format: web::Path<(String, String)>,
    ) -> ActixResult<HttpResponse>
{
    do_aggregate(req, state, cube_format.into_inner()).await
}


/// Performs data aggregation.
pub async fn do_aggregate(
    req: HttpRequest,
    state: web::Data<AppState>,
    cube_format: (String, String),
    ) -> ActixResult<HttpResponse>
{
    let (cube, format) = cube_format;

    // Get cube object to check for API key
    let schema = &state.schema.read().unwrap().clone();
    let cube_obj = ok_or_404!(schema.get_cube_by_name(&cube));

    verify_authorization(&req, &state, cube_obj.min_auth_level)?;

    let format = format.parse::<FormatType>();
    let format = ok_or_404!(format);

    info!("cube: {}, format: {:?}", cube, format);

    let query = req.query_string();
    lazy_static!{
        static ref QS_NON_STRICT: qs::Config = qs::Config::new(5, false);
    }
    let agg_query_res = QS_NON_STRICT.deserialize_str::<AggregateQueryOpt>(&query);
    let agg_query = ok_or_404!(agg_query_res);

    info!("query opts:{:?}", agg_query);

    // Check if this query is already cached
    // TODO refactor
    //let redis_pool = state.redis_pool.clone();
    //let redis_cache_key = get_redis_cache_key("core", &req, &cube, &format);

    //if let Some(res) = check_redis_cache(&format, &redis_pool, &redis_cache_key) {
    //    return res;
    //}

    // Gets the Source Data
    let source_data = Some(generate_source_data(&cube_obj));

    // Turn AggregateQueryOpt into Query
    let ts_query: Result<TsQuery, _> = agg_query.try_into();
    let ts_query = ok_or_404!(ts_query);

    // sql injection mitigation on query:
    // - Check that cut members exist in members cache
    // this is in braces to explicitly the scope in which
    // req is borrowed, since req is moved later in the `map_err`
    {
        let cache = state.cache.read().unwrap();
        let cube_cache = some_or_404!(cache.find_cube_info(&cube), format!("Cube {} not found", cube));
        ok_or_404!(validate_members(&ts_query.cuts, &cube_cache));
    }

    let query_ir_headers = schema.sql_query(&cube, &ts_query, None);
    let (query_ir, headers) = ok_or_404!(query_ir_headers);

    let sql = state
        .backend
        .generate_sql(query_ir);

    info!("Sql query: {}", sql);
    info!("Headers: {:?}", headers);

    let df = ok_or_500!(
        state.backend.exec_sql(sql).await
            .map_err(move |e| {
                if state.debug {
                    ServerError::Db { cause: e.to_string() }
                } else {
                    ServerError::Db { cause: "Internal Server Error 1010".to_owned() }
                }
            })
    );

    let content_type = format_to_content_type(&format);

    match format_records(&headers, df, format, source_data, false) {
        Ok(res) => {
            // TODO turn redis cache back on
            // Try to insert this result in the Redis cache, if available
            //insert_into_redis_cache(&res, &redis_pool, &redis_cache_key);

            Ok(HttpResponse::Ok()
                .content_type(content_type)
                .body(res))
        },
        Err(err) => Ok(HttpResponse::NotFound().json(err.to_string())),
    }
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AggregateQueryOpt {
    drilldowns: Option<Vec<String>>,
    cuts: Option<Vec<String>>,
    measures: Option<Vec<String>>,
    properties: Option<Vec<String>>,
    filters: Option<Vec<String>>,
    captions: Option<Vec<String>>,
    parents: Option<bool>,
    top: Option<String>,
    top_where: Option<String>,
    sort: Option<String>,
    limit: Option<String>,
    growth: Option<String>,
    rca: Option<String>,
    rate: Option<String>,
    debug: Option<bool>,
    exclude_default_members: Option<bool>,
//    distinct: Option<bool>,
//    nonempty: Option<bool>,
    sparse: Option<bool>,
}

impl TryFrom<AggregateQueryOpt> for TsQuery {
    type Error = Error;

    fn try_from(agg_query_opt: AggregateQueryOpt) -> Result<Self, Self::Error> {
        let drilldowns: Result<Vec<_>, _> = agg_query_opt.drilldowns
            .map(|ds| {
                ds.iter().map(|d| d.parse()).collect()
            })
            .unwrap_or(Ok(vec![]));

        let cuts: Result<Vec<_>, _> = agg_query_opt.cuts
            .map(|cs| {
                cs.iter().map(|c| c.parse()).collect()
            })
            .unwrap_or(Ok(vec![]));

        let measures: Result<Vec<_>, _> = agg_query_opt.measures
            .map(|ms| {
                ms.iter().map(|m| m.parse()).collect()
            })
            .unwrap_or(Ok(vec![]));

        let properties: Result<Vec<_>, _> = agg_query_opt.properties
            .map(|ms| {
                ms.iter().map(|m| m.parse()).collect()
            })
            .unwrap_or(Ok(vec![]));

        let filters: Result<Vec<_>, _> = agg_query_opt.filters
            .map(|fs| {
                fs.iter().map(|f| f.parse()).collect()
            })
            .unwrap_or(Ok(vec![]));

        let captions: Result<Vec<_>, _> = agg_query_opt.captions
            .map(|cs| {
                cs.iter().map(|c| c.parse()).collect()
            })
            .unwrap_or(Ok(vec![]));

        let drilldowns = drilldowns?;
        let cuts = cuts?;
        let measures = measures?;
        let properties = properties?;
        let filters = filters?;
        let captions = captions?;

        let parents = agg_query_opt.parents.unwrap_or(false);

        let top = agg_query_opt.top
            .map(|t| t.parse())
            .transpose()?;
        let top_where = agg_query_opt.top_where
            .map(|t| t.parse())
            .transpose()?;
        let sort = agg_query_opt.sort
            .map(|s| s.parse())
            .transpose()?;
        let limit = agg_query_opt.limit
            .map(|l| l.parse())
            .transpose()?;

        let growth = agg_query_opt.growth
            .map(|g| g.parse())
            .transpose()?;

        let rca = agg_query_opt.rca
            .map(|r| r.parse())
            .transpose()?;

        let rate = agg_query_opt.rate
            .map(|r| r.parse())
            .transpose()?;

        let debug = agg_query_opt.debug.unwrap_or(false);
        let sparse = agg_query_opt.sparse.unwrap_or(false);
        let exclude_default_members = agg_query_opt.exclude_default_members.unwrap_or(false);

        // TODO: deserialize rate
        Ok(TsQuery {
            drilldowns,
            cuts,
            measures,
            parents,
            properties,
            filters,
            captions,
            top,
            top_where,
            sort,
            limit,
            rca,
            growth,
            debug,
            rate,
            sparse,
            exclude_default_members,
        })
    }
}

