use actix_web_lab::__reexports::serde_json;
use camera_reel_service::Metadata;
use sha256::digest;

use crate::common::{create_test_identity, create_test_server};

mod common;

#[actix_web::test]
async fn test_live() {
    let server = create_test_server().await;
    let address = server.addr();

    let response = reqwest::Client::new()
        .get(&format!("http://{}/health/live", address))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
}

#[actix_web::test]
async fn test_upload_image() {
    let server = create_test_server().await;
    let address = server.addr();

    let identity = create_test_identity();

    // prepare image
    let image_bytes = include_bytes!("./resources/image.png").to_vec();
    let image_hash = digest(&image_bytes);
    let image_file_part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name("image.png")
        .mime_str("image/png")
        .unwrap();

    // prepare image metadata
    let metadata = Metadata {
        user_address: "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string(),
        ..Default::default()
    };
    let metadata_json = serde_json::to_vec(&metadata).unwrap();
    let metadata_hash = digest(&metadata_json);
    let metadata_part = reqwest::multipart::Part::bytes(metadata_json)
        .file_name("metadata.json")
        .mime_str("application/json")
        .unwrap();

    // prepare authchain
    let payload = format!("{image_hash}-{metadata_hash}");
    let authchain = identity.sign_payload(payload);
    let auth_chain_json = serde_json::to_vec(&authchain).unwrap();
    let authchain_part = reqwest::multipart::Part::bytes(auth_chain_json)
        .mime_str("application/json")
        .unwrap();

    // fill form
    let form = reqwest::multipart::Form::new();
    let form = form
        .part("image", image_file_part)
        .part("metadata", metadata_part)
        .part("authchain", authchain_part);

    let response = reqwest::Client::new()
        .post(&format!("http://{}/api/images", address))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
}
