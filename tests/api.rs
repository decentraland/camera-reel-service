use actix_test::TestServer;
use actix_web_lab::__reexports::serde_json;
use camera_reel_service::api::{
    get::{GetImagesResponse, UserDataResponse},
    Image, ImageCompact,
};
use common::upload_test_failing_image;
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
async fn test_upload_failing_image() {
    let server = create_test_server().await;
    let address = server.addr();

    let response = upload_test_failing_image("any/image.png", &address.to_string()).await;
    assert!(response.contains("invalid file name"));
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
        .json::<GetImagesResponse<Image>>()
        .await
        .unwrap();

    assert_eq!(images_response.user_data.current_images, 5);
}

#[actix_web::test]
async fn test_get_multiple_images_compact() {
    let server = create_test_server().await;
    let address = server.addr();

    let user_address = "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string();

    for i in 0..5 {
        upload_test_image(&format!("image-{i}.png"), &address.to_string()).await;
    }

    let images_response = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/users/{}/images?compact=true",
            address, user_address
        ))
        .send()
        .await
        .unwrap()
        .json::<GetImagesResponse<ImageCompact>>()
        .await
        .unwrap();

    assert_eq!(images_response.user_data.current_images, 5);
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
    let response = response.json::<UserDataResponse>().await;
    println!("{:?}", response);
    assert!(response.is_ok());

    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[actix_web::test]
async fn test_update_image_visibility() {
    let server: TestServer = create_test_server().await;
    let address = server.addr();

    let id = upload_test_image("image.png", &address.to_string()).await;

    // Initial visibility is private by default
    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());

    let image = response.json::<Image>().await.unwrap();
    assert_eq!(image.is_public, false);

    // Update visibility to public
    let identity = create_test_identity();
    let path = &format!("/api/images/{}/visibility", id);
    let headers = get_signed_headers(identity, "patch", path, "");

    let response = reqwest::Client::new()
        .patch(&format!("http://{}{path}", address))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .json(&serde_json::json!({ "is_public": true }))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());

    // Check if visibility was updated
    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let image = response.json::<Image>().await.unwrap();
    assert_eq!(image.is_public, true);
}
