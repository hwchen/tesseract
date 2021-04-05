use actix_web::HttpResponse;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    // the display is shown in the logs as an explanation of the error
    #[error("db internal error")]
    Db {
        cause: String,
    },
    // TODO route this through Error in handler, so that it will log. Right now, nothing gets
    // logged because it's just a httpresponse in the logic layer non-unique handler
    // In addition, even if there's and ErrorCode variant, it should still be able to log a cause,
    // as is currently done on Db failures, but without hardcoding the error value into the handler
    // because that should be handled here in the error module.
    #[error("Logic Layer duplicate name: {:?} in cube {:?}. Level/property name must be unique.", name, cube)]
    LogicLayerDuplicateNames {
        cube: String,
        name: String,
    },

    #[error("Internal Server Error {}", code)]
    ErrorCode {
        code: String,
    }
}

impl actix_web::error::ResponseError for ServerError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServerError::Db { cause } => HttpResponse::InternalServerError().body(cause.clone()),
            ServerError::LogicLayerDuplicateNames { .. } => HttpResponse::InternalServerError().body(self.to_string()),
            ServerError::ErrorCode { .. } => HttpResponse::InternalServerError().body(self.to_string()),
        }
    }
}


