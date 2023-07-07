use actix_web_lab::__reexports::serde_json;
use camera_reel_service::Metadata;

use crate::common::create_test_server;

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

    let image_bytes = include_bytes!("./resources/image.png").to_vec();
    let image_file_part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name("image.png")
        .mime_str("image/png")
        .unwrap();

    let metadata = Metadata::default();
    let metadata_json = serde_json::to_vec(&metadata).unwrap();

    let metadata_part = reqwest::multipart::Part::bytes(metadata_json)
        .file_name("metadata.json")
        .mime_str("application/json")
        .unwrap();

    let form = reqwest::multipart::Form::new();
    let form = form
        .part("image", image_file_part)
        .part("metadata", metadata_part);

    let response = reqwest::Client::new()
        .post(&format!("http://{}/api/images", address))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
}
