use actix_web::{
    HttpRequest,
    HttpResponse,
    Result as ActixResult,
};
use serde_derive::Serialize;
use structopt::clap::crate_version;


/// Returns server status and Tesseract version.
pub async fn index_handler(_req: HttpRequest) -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(
        &Status {
            status: "ok".to_owned(),
            // TODO set this as the Cargo.toml version, after structopt added
            tesseract_version: crate_version!().to_owned(),
        }
    ))
}

/// Holds the contents of an `index_handler` handler response before serialization.
#[derive(Debug, Serialize)]
struct Status {
    status: String,
    tesseract_version: String,
}
