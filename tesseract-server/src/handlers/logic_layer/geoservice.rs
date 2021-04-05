use std::str;

use actix_web::client::Client;
use failure::{Error, format_err};
use serde_derive::Deserialize;
use url::Url;


#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GeoServiceResponseJson {
    pub geoid: String,
    pub level: String,
}


pub enum GeoserviceQuery {
    Neighbors,
    Children,
    Parents,
    Intersects,
    Distance,
}


/// Queries geoservice for geo cuts resolution.
pub async fn query_geoservice(
    base_url: &Url,
    geoservice_query: &GeoserviceQuery,
    geo_id: &str
) -> Result<Vec<GeoServiceResponseJson>, Error> {
    let join_str = match geoservice_query {
        GeoserviceQuery::Neighbors => format!("neighbors/{}", geo_id),
        GeoserviceQuery::Children => format!("relations/children/{}", geo_id),
        GeoserviceQuery::Parents => format!("relations/parents/{}", geo_id),
        _ => return Err(format_err!("This type of geoservice query is not yet supported"))
    };

    let query_url = base_url.join(&join_str).unwrap();

    // TODO put client in AppData
    let client = Client::default();
    let resp: Result<Vec<GeoServiceResponseJson>, Result<(), Error>> = client.get(query_url.as_str())
        .insert_header(("User-Agent", "Actix-web"))
        .send()
        .await?;

    let body = resp.body();
    let data: Vec<GeoServiceResponseJson> = serde_json::from_str(body)?;

    Ok(data)
}
