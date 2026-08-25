use actix_web::{
    dev::Payload, error::ErrorUnauthorized, http::header::HeaderMap, Error, FromRequest,
    HttpRequest,
};
use dcl_crypto::authenticator::WithoutTransport;
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
            verification(request.headers(), request.method().as_str(), request.path())
                .await
                .map(|address| AuthUser { address })
                .map_err(|_| ErrorUnauthorized("Unathorized"))
        })
    }
}

/// Verification policy for every authenticated route.
///
/// `dcl-crypto-middleware-rs` 0.3.0 signs and verifies the metadata bytes exactly as
/// `x-identity-metadata` delivers them; before that it lowercased the whole joined payload, leaving
/// the metadata's casing outside the signature. Taking the fix is what lets this service verify
/// clients that have already migrated to the matching format (`decentraland-crypto-fetch` 3,
/// `@dcl/crypto-middleware` 6).
///
/// The legacy payload is still accepted as a fallback, tried only after the current format fails.
/// Every explorer that reaches this service still signs the old way, and they are separate client
/// releases that cannot be deployed alongside a server — so going strict-only would refuse all of
/// them at once. Remove this once unity, godot and bevy have shipped the new format.
///
/// The declared-key list is empty, and that is a statement rather than an omission: nothing here
/// authorizes on metadata. `verify` returns the address recovered from the signature and `AuthUser`
/// carries only that; no handler reads `x-identity-metadata` at all. With no field whose spelling
/// could change an authorization outcome, there is nothing for a key list to bind.
fn verification_options() -> VerificationOptions<WithoutTransport> {
    VerificationOptions::default().accept_legacy_payload(&[])
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

    verify(method, path, headers, verification_options())
        .await
        .map(|address| address.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::{HeaderName, HeaderValue};
    use dcl_crypto::Identity;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Metadata carrying uppercase, which is the only case where the two payload formats differ.
    /// All-lowercase metadata folds to itself and would verify identically under either, proving
    /// nothing about which format is accepted.
    const MIXED_CASE_METADATA: &str = r#"{"realmName":"Main"}"#;

    fn test_identity() -> Identity {
        Identity::from_json(
            r#"{
         "ephemeralIdentity": {
           "address": "0x84452bbFA4ca14B7828e2F3BBd106A2bD495CD34",
           "publicKey": "0x0420c548d960b06dac035d1daf826472eded46b8b9d123294f1199c56fa235c89f2515158b1e3be0874bfb15b42d1551db8c276787a654d0b8d7b4d4356e70fe42",
           "privateKey": "0xbc453a92d9baeb3d10294cbc1d48ef6738f718fd31b4eb8085efe7b311299399"
         },
         "expiration": "3021-10-16T22:32:29.626Z",
         "authChain": [
           {
             "type": "SIGNER",
             "payload": "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5",
             "signature": ""
           },
           {
             "type": "ECDSA_EPHEMERAL",
             "payload": "Decentraland Login\nEphemeral address: 0x84452bbFA4ca14B7828e2F3BBd106A2bD495CD34\nExpiration: 3021-10-16T22:32:29.626Z",
             "signature": "0x39dd4ddf131ad2435d56c81c994c4417daef5cf5998258027ef8a1401470876a1365a6b79810dc0c4a2e9352befb63a9e4701d67b38007d83ffc4cd2b7a38ad51b"
           }
         ]
        }"#,
        )
        .unwrap()
    }

    fn now_ms() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string()
    }

    /// The 6.x payload: method and path lowercased, metadata joined verbatim.
    fn current_payload(method: &str, path: &str, timestamp: &str, metadata: &str) -> String {
        format!(
            "{}:{}:{}:{}",
            method.to_lowercase(),
            path.to_lowercase(),
            timestamp,
            metadata
        )
    }

    /// The pre-0.3.0 payload: the whole joined string folded, metadata included.
    fn legacy_payload(method: &str, path: &str, timestamp: &str, metadata: &str) -> String {
        format!("{}:{}:{}:{}", method, path, timestamp, metadata).to_lowercase()
    }

    fn headers_for(
        identity: &Identity,
        payload: String,
        timestamp: &str,
        metadata: &str,
    ) -> HeaderMap {
        let chain = identity.sign_payload(payload);
        let links = serde_json::to_value(&chain).unwrap();

        let mut headers = HeaderMap::new();
        for (index, link) in links.as_array().unwrap().iter().enumerate() {
            headers.insert(
                HeaderName::try_from(format!("x-identity-auth-chain-{}", index)).unwrap(),
                HeaderValue::from_str(&serde_json::to_string(link).unwrap()).unwrap(),
            );
        }
        headers.insert(
            HeaderName::from_static("x-identity-timestamp"),
            HeaderValue::from_str(timestamp).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-identity-metadata"),
            HeaderValue::from_str(metadata).unwrap(),
        );

        headers
    }

    #[actix_web::test]
    async fn should_verify_a_request_signing_the_current_payload() {
        let identity = test_identity();
        let timestamp = now_ms();
        let headers = headers_for(
            &identity,
            current_payload("GET", "/api/images", &timestamp, MIXED_CASE_METADATA),
            &timestamp,
            MIXED_CASE_METADATA,
        );

        // What the migrated clients send. Before 0.3.0 this was refused, because the verifier
        // rebuilt a folded payload the client never signed.
        assert!(verification(&headers, "GET", "/api/images").await.is_ok());
    }

    #[actix_web::test]
    async fn should_still_verify_a_request_signing_the_legacy_payload() {
        let identity = test_identity();
        let timestamp = now_ms();
        let headers = headers_for(
            &identity,
            legacy_payload("GET", "/api/images", &timestamp, MIXED_CASE_METADATA),
            &timestamp,
            MIXED_CASE_METADATA,
        );

        // What every explorer still sends. `accept_legacy_payload` is what keeps them working, and
        // without it this is a 401 for the whole client fleet at once.
        assert!(verification(&headers, "GET", "/api/images").await.is_ok());
    }

    #[actix_web::test]
    async fn should_refuse_metadata_re_cased_after_signing() {
        let identity = test_identity();
        let timestamp = now_ms();
        let mut headers = headers_for(
            &identity,
            current_payload("GET", "/api/images", &timestamp, MIXED_CASE_METADATA),
            &timestamp,
            MIXED_CASE_METADATA,
        );

        // Accepting the legacy format widens which signatures verify; it must not make the metadata
        // header rewritable. This delivery matches neither payload: not the current one, whose bytes
        // it no longer equals, and not the folded one, which this chain never signed.
        headers.insert(
            HeaderName::from_static("x-identity-metadata"),
            HeaderValue::from_static(r#"{"RealmName":"Main"}"#),
        );

        assert!(verification(&headers, "GET", "/api/images").await.is_err());
    }
}
