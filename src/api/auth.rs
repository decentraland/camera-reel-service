use actix_web::{
    dev::Payload, error::ErrorUnauthorized, http::header::HeaderMap, Error, FromRequest,
    HttpRequest,
};
use dcl_crypto_middleware_rs::signed_fetch::{verify, AuthMiddlewareError, VerificationOptions};
use serde::Deserialize;
use std::{collections::HashMap, future::Future, pin::Pin};

#[derive(Deserialize, Debug, Default, Clone)]
pub struct AuthUser {
    pub address: String,
}

impl FromRequest for AuthUser {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        let request = request.clone();
        Box::pin(async move {
            if let Ok(user_address) =
                verification(request.headers(), request.method().as_str(), request.path()).await
            {
                Ok(AuthUser {
                    address: user_address,
                })
            } else {
                Err(ErrorUnauthorized("Unathorized"))
            }
        })
    }
}

async fn verification(
    headers: &HeaderMap,
    method: &str,
    path: &str,
) -> Result<String, AuthMiddlewareError> {
    let headers = headers
        .iter()
        .map(|(key, val)| (key.to_string(), val.to_str().unwrap_or("").to_string()))
        .collect::<HashMap<String, String>>();

    verify(method, path, headers, VerificationOptions::default())
        .await
        .map(|address| address.to_string())
}
