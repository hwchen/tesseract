use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_derive::{Serialize, Deserialize};
use std::collections::HashMap;
use actix_web::{web, HttpRequest};

pub const X_TESSERACT_JWT_TOKEN: &str = "x-tesseract-jwt-token";
use tesseract_core::DEFAULT_ALLOWED_ACCESS;


#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    status: String,
    exp: usize,
    auth_level: Option<i32>,
}

pub fn extract_token(req: &HttpRequest) -> String {
    let qry = web::Query::<HashMap<String, String>>::from_query(req.query_string()).expect("temporary unwrap");

    let token = {
        let qp_token = qry.get(X_TESSERACT_JWT_TOKEN);
        match qp_token {
            None => {
                // If we don't match in query params, try headers
                // The next lines below are little ugly. Basically,
                // we need to catch for two potential errors:
                // 1. the key might not be present (phase1)
                // 2. the key might not parse to a string properly (phase2)
                let phase1 = req.headers().get(X_TESSERACT_JWT_TOKEN);
                match phase1 {
                    Some(val) => {
                        let phase2 = val.to_str();
                        match phase2 {
                            Ok(v) => v,
                            _ => ""
                        }
                    },
                    _ => "",
                }
            },
            Some(token) => token,
        }
    };
    token.to_string()
}

// None = auth not set on server, -1 = bad auth level
pub fn user_auth_level(jwt_secret: &Option<String>, raw_token: &str) -> Option<i32> {
    match jwt_secret {
        Some(key) => {
            let decoding_key = DecodingKey::from_base64_secret(&key).ok()?;
            let validation = Validation::default();
            match decode::<Claims>(&raw_token, &decoding_key, &validation) {
                Ok(c) => {
                    let claims: Claims = c.claims;
                    if claims.auth_level.is_some() && claims.status == "valid" {
                        claims.auth_level
                    } else {
                        Some(DEFAULT_ALLOWED_ACCESS)
                    }
                },
                Err(_err) => Some(DEFAULT_ALLOWED_ACCESS), // If any error occurs, validate to default access level
            }
        },
        None => None
    }
}

pub fn validate_web_token(jwt_secret: &Option<String>, raw_token: &str, min_auth_level: i32) -> bool {
    // if no token is provided, allowed access where min auth is 0
    if raw_token == "" && min_auth_level == DEFAULT_ALLOWED_ACCESS {
        return true;
    }
    match jwt_secret {
        Some(key) => {
            let decoding_key = match DecodingKey::from_base64_secret(&key) {
                Ok(dk) => dk,
                Err(_) => return false,
            };
            let validation = Validation::default();
            match decode::<Claims>(&raw_token, &decoding_key, &validation) {
                Ok(c) => {
                    let claims: Claims = c.claims;
                    let part1 = claims.status == "valid"; // TODO allow this value to be configurable
                    let part2 = match claims.auth_level {
                        Some(lvl) => lvl >= min_auth_level,
                        None => false // if no auth_level is set, do not validate
                    };
                    part1 && part2
                },
                Err(_err) => false, // If any error occurs, do not validate
            }
        },
        None => true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_jwt_auth_good() {
        let jwt_secret = Some("hello-secret-123".to_string());
        let result = validate_web_token(&jwt_secret, "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJPbmxpbmUgSldUIEJ1aWxkZXIiLCJpYXQiOjE1ODA0MjA2NzQsImV4cCI6MTczODE4NzA4OSwiYXVkIjoid3d3LmV4YW1wbGUuY29tIiwic3ViIjoianJvY2tldEBleGFtcGxlLmNvbSIsInN0YXR1cyI6InZhbGlkIiwiYXV0aF9sZXZlbCI6Mn0.IUkh8-y9LIcNcxpmCJyLK09SY9LDm8P0ekJcL4OZKNI", DEFAULT_ALLOWED_ACCESS);
        assert_eq!(result, true);
    }

    #[test]
    fn test_jwt_auth_bad1() {
        let jwt_secret = Some("hello-secret-123".to_string());
        let result = validate_web_token(&jwt_secret, "eyJpc3MiOiJPbmxpbmUgSldUIEJ1aWxkZXIiLCJpYXQiOjE1Nzk4ODI4NDcsImV4cCI6MjUyNjU2NzY3MywiYXVkIjoid3d3LmV4YW1wbGUuY29tIiwic3ViIjoianJvY2tldEBleGFtcGxlLmNvbSIsInN0YXR1cyI6InZhbGlkIn0.GSMVTKG3RrWOCfoDpGmJcYakspKsmjkZw7Le9lPJwtw", DEFAULT_ALLOWED_ACCESS);
        assert_eq!(result, false);
    }

    #[test]
    fn test_jwt_auth_allow_by_default() {
        let jwt_secret = Some("hello-secret-123".to_string());
        let result = validate_web_token(&jwt_secret, "", DEFAULT_ALLOWED_ACCESS);
        assert_eq!(result, true);
    }

    #[test]
    fn test_jwt_auth_do_not_allow_high_by_default() {
        let jwt_secret = Some("hello-secret-123".to_string());
        let result = validate_web_token(&jwt_secret, "", 100);
        assert_eq!(result, false);
    }

    #[test]
    fn test_jwt_auth_good2() {
        // if token is none, all requests are OK
        let jwt_secret = None;
        let result = validate_web_token(&jwt_secret, "", DEFAULT_ALLOWED_ACCESS);
        assert_eq!(result, true);
    }

    #[test]
    fn test_jwt_auth_level_overflow_bad() {
        let jwt_secret = Some("hello-secret-123".to_string());
        let result = validate_web_token(&jwt_secret, "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJPbmxpbmUgSldUIEJ1aWxkZXIiLCJpYXQiOjE1ODAyNDg2OTAsImV4cCI6MTU4MDI1MjgyNiwiYXVkIjoid3d3LmV4YW1wbGUuY29tIiwic3ViIjoianJvY2tldEBleGFtcGxlLmNvbSIsInN0YXR1cyI6InZhbGlkIiwiYXV0aF9sZXZlbCI6NWUrNDUsImp0aSI6IjIzYmIwMDg5LWQ2NDctNDNlOC04YjdiLWIxOGU4N2ViMjljZCJ9.-d7k5i6oGopyBzSbiD9rl9FyYQUR_hwy4tvYzgfMb1M", DEFAULT_ALLOWED_ACCESS);
        assert_eq!(result, false);
    }

    #[test]
    fn test_auth_level_too_low_bad() {
        let jwt_secret = Some("hello-secret-123".to_string());
        let result = validate_web_token(&jwt_secret, "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJPbmxpbmUgSldUIEJ1aWxkZXIiLCJpYXQiOjE1ODA0MjA2NzQsImV4cCI6MTczODE4NzA4OSwiYXVkIjoid3d3LmV4YW1wbGUuY29tIiwic3ViIjoianJvY2tldEBleGFtcGxlLmNvbSIsInN0YXR1cyI6InZhbGlkIiwiYXV0aF9sZXZlbCI6Mn0.IUkh8-y9LIcNcxpmCJyLK09SY9LDm8P0ekJcL4OZKNI", 100);
        assert_eq!(result, false);
    }

    #[test]
    fn test_no_auth_level_in_jwt_is_bad() {
        let jwt_secret = Some("hello-secret-123".to_string());
        let result = validate_web_token(&jwt_secret, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwiaWF0IjoxNTE2MjM5MDIyLCJleHAiOjE5MTYyMzkwMjIsInN0YXR1cyI6InZhbGlkIn0.8kc8kYiPe2PSzGuEvDQJNw0eJicHloPhJK6FYJL95pI", 0);
        assert_eq!(result, false);
    }
}
