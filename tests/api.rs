use camera_reel_service::api::get::GetImagesResponse;
use common::upload_test_image;

use crate::common::{create_test_identity, create_test_server, get_signed_headers};

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

    upload_test_image("image.png", &address.to_string()).await;
}

#[actix_web::test]
async fn test_get_multiple_images() {
    let server = create_test_server().await;
    let address = server.addr();

    let user_address = "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string();

    for i in 0..5 {
        upload_test_image(&format!("image-{i}.png"), &address.to_string()).await;
    }

    let images_response = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/users/{}/images",
            address, user_address
        ))
        .send()
        .await
        .unwrap()
        .json::<GetImagesResponse>()
        .await
        .unwrap();

    assert_eq!(images_response.current_images, 5);
}

#[actix_web::test]
async fn test_delete_image() {
    let server = create_test_server().await;
    let address = server.addr();

    let id = upload_test_image("image.png", &address.to_string()).await;
    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let path = format!("/api/images/{id}");

    let headers = get_signed_headers(create_test_identity(), "delete", &path, "{}");

    let response = reqwest::Client::new()
        .delete(&format!("http://{}{}", address, path))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}
