use std::fs;
use std::error::Error;

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use once_cell::sync::OnceCell;
use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

static JWT_POLICY: OnceCell<(Validation, DecodingKey)> = OnceCell::new();

pub fn setup() -> Result<(), Box<dyn Error>> {
    let key = if let Ok(key) = fs::read("./jwt_key.pem") {
        tracing::info!("Using JWT validation.");
        key
    } else {
        tracing::info!("NOT using JWT validation, since file `jwt_key.pem` does not exist.");
        return Ok(());
    };

    let key = DecodingKey::from_ec_pem(&key)?;

    // See https://docs.rs/jsonwebtoken/8.1.1/jsonwebtoken/struct.Validation.html for more info about JWT validation policy
    let mut validation_policy = Validation::new(Algorithm::ES256);
    validation_policy.validate_exp = true; // validation "exp" (expiry time) field
    let _ = JWT_POLICY.set((validation_policy, key));

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    pub userid: usize,
    pub username: String,

    // Expiration time (as UTC timestamp)
    pub exp: usize,

}

#[derive(Debug, Default, Clone)]
pub struct AuthExtension {
    pub claims: Option<JwtClaims>,
}

impl AuthExtension {
    #[allow(unused)]
    pub fn is_logged(&self) -> bool {
        self.claims.is_some()
    }

    #[allow(unused)]
    pub fn require_logged(&self) -> Result<&JwtClaims, StatusCode> {
        self.claims
            .as_ref()
            .ok_or_else(|| StatusCode::UNAUTHORIZED)
    }

    pub fn userid(&self) -> Option<usize> {
        self.claims.as_ref().map(|c| c.userid)
    }

    pub fn username(&self) -> Option<String> {
        self.claims.as_ref().map(|c| c.username.clone())
    }
}

/// This interceptor will extend the request with `AuthExtension` as an
/// extension.
pub async fn auth_interceptor<B>(mut req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    let (policy, key) = if let Some(p) = JWT_POLICY.get() {
        p
    } else {
        // JWT verification is disabled
        let response = next.run(req).await;
        return Ok(response);
    };

    let t = if let Some(t) = req.headers().get("accesstoken") {
        t
    } else {
        req.extensions_mut().insert(AuthExtension::default());
        let response = next.run(req).await;
        return Ok(response);
    };

    let t = t
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let token = jsonwebtoken::decode::<JwtClaims>(t, key, policy)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    req.extensions_mut().insert(AuthExtension {
        claims: Some(token.claims),
    });

    let response = next.run(req).await;
    return Ok(response);
}