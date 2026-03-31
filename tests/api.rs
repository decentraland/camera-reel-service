use actix_web_lab::__reexports::serde_json;
use camera_reel_service::api::{
    get::{
        GetGalleryImagesResponse, GetImagesResponse, GetMultiplePlacesImagesResponse,
        GetPlaceImagesResponse, UserDataResponse,
    },
    Image, ResponseError,
};
use common::upload_test_failing_image;
use common::upload_test_image;
use common::{get_place_id, upload_public_test_image};
use sqlx::types::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::{
    create_test_identity, create_test_server, create_test_server_with_places_url,
    get_signed_headers, poll_sqs_for_message_with_filter,
};

mod common;

#[actix_web::test]
async fn test_live() {
    let (server, _) = create_test_server().await;
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
    let (server, test_context) = create_test_server().await;
    let address = server.addr();
    let place_id = get_place_id();

    let image_id = upload_test_image("image.png", &address.to_string(), &place_id).await;

    // Verify SNS event was published correctly (filter for photo-taken events)
    let sns_message = poll_sqs_for_message_with_filter(
        &test_context.sqs_client,
        &test_context.queue_url,
        10,
        Some("photo-taken"),
    )
    .await;
    assert!(
        sns_message.is_some(),
        "SNS message should have been received"
    );

    let message = sns_message.unwrap();

    // Verify the event structure
    assert_eq!(message["type"], "camera");
    assert_eq!(message["subType"], "photo-taken");
    assert_eq!(message["key"], image_id);

    // Verify metadata
    let metadata = &message["metadata"];
    assert_eq!(metadata["photoId"], image_id);
    assert_eq!(metadata["isPublic"], false); // upload_test_image creates private images
    assert_eq!(
        metadata["userAddress"],
        "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5"
    );
    assert_eq!(metadata["realm"], "https://realm.org/v1");
    assert_eq!(metadata["placeId"], place_id);

    // Verify timestamp is present and reasonable (within last 60 seconds)
    let timestamp = message["timestamp"].as_u64().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        timestamp <= now && timestamp >= now - 60,
        "Timestamp should be recent"
    );

    // Verify users array exists (should be empty for default metadata)
    assert!(metadata["users"].is_array());
}

#[actix_web::test]
async fn test_upload_failing_image() {
    let (server, _) = create_test_server().await;
    let address = server.addr();

    let response = upload_test_failing_image("any/image.png", &address.to_string()).await;
    assert!(response.contains("invalid file name"));
}

#[actix_web::test]
async fn test_get_multiple_images() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let user_address = "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string();
    let place_id = get_place_id();
    let identity = create_test_identity();

    for i in 0..5 {
        upload_test_image(&format!("image-{i}.png"), &address.to_string(), &place_id).await;
    }

    let path = &format!("/api/users/{}/images", user_address);
    let headers = get_signed_headers(identity, "get", path, "");

    let images_response = reqwest::Client::new()
        .get(&format!("http://{}{}", address, path))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap()
        .json::<GetImagesResponse>()
        .await
        .unwrap();

    assert_eq!(images_response.user_data.current_images, 5);
}

#[actix_web::test]
async fn test_get_multiple_only_public_images() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let not_my_user_address = "0x6949f9f239d1a0816ce5eb364a1f588ae9cc1bf4".to_string();
    let place_id = get_place_id();
    let identity = create_test_identity();

    for i in 0..5 {
        upload_test_image(&format!("image-{i}.png"), &address.to_string(), &place_id).await;
    }

    let path = &format!("/api/users/{}/images", not_my_user_address);
    let headers = get_signed_headers(identity, "get", path, "");

    let images_response = reqwest::Client::new()
        .get(&format!("http://{}{}", address, path))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap()
        .json::<GetImagesResponse>()
        .await
        .unwrap();

    assert_eq!(images_response.user_data.current_images, 0);
}

#[actix_web::test]
async fn test_get_multiple_images_compact() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let user_address = "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string();
    let place_id = get_place_id();
    let identity = create_test_identity();

    for i in 0..5 {
        upload_test_image(&format!("image-{i}.png"), &address.to_string(), &place_id).await;
    }

    let path = &format!("/api/users/{}/images", user_address);
    let headers = get_signed_headers(identity, "get", path, "");

    let images_response = reqwest::Client::new()
        .get(&format!("http://{}{}?compact=true", address, path))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap()
        .json::<GetGalleryImagesResponse>()
        .await
        .unwrap();

    assert_eq!(images_response.user_data.current_images, 5);
}

#[actix_web::test]
async fn test_delete_image() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = Uuid::new_v4().to_string();

    let id = upload_test_image("image.png", &address.to_string(), &place_id).await;
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
    let (server, test_context) = create_test_server().await;
    let address = server.addr();
    let place_id = Uuid::new_v4().to_string();

    let id = upload_public_test_image("image.png", &address.to_string(), &place_id).await;

    // Initial visibility is public (as uploaded with upload_public_test_image)
    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());

    let image = response.json::<Image>().await.unwrap();
    assert_eq!(image.is_public, true);

    // Update visibility to private
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
        .json(&serde_json::json!({ "is_public": false }))
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
    assert_eq!(image.is_public, false);

    // Verify SNS event was published correctly (filter for photo-privacy-changed events)
    let sns_message = poll_sqs_for_message_with_filter(
        &test_context.sqs_client,
        &test_context.queue_url,
        10,
        Some("photo-privacy-changed"),
    )
    .await;
    assert!(
        sns_message.is_some(),
        "SNS message should have been received"
    );

    let message = sns_message.unwrap();

    // Verify the event structure
    assert_eq!(message["type"], "camera");
    assert_eq!(message["subType"], "photo-privacy-changed");
    assert_eq!(message["key"], id);

    // Verify metadata
    let metadata = &message["metadata"];
    assert_eq!(metadata["photoId"], id);
    assert_eq!(metadata["isPublic"], false);
    assert_eq!(
        metadata["userAddress"],
        "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5"
    );

    // Verify timestamp is present and reasonable (within last 60 seconds)
    let timestamp = message["timestamp"].as_u64().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        timestamp <= now && timestamp >= now - 60,
        "Timestamp should be recent"
    );
}

#[actix_web::test]
async fn test_get_multiple_images_by_place() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = get_place_id();

    for i in 0..5 {
        upload_test_image(
            &format!("image-pr-{i}.png"),
            &address.to_string(),
            &place_id,
        )
        .await;
        upload_public_test_image(
            &format!("image-pu-{i}.png"),
            &address.to_string(),
            &place_id,
        )
        .await;
    }

    let images_response = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/places/{}/images",
            address, place_id
        ))
        .send()
        .await
        .unwrap()
        .json::<GetPlaceImagesResponse>()
        .await
        .unwrap();

    assert_eq!(images_response.place_data.max_images, 5);
}

#[actix_web::test]
async fn test_get_multiple_places_images() {
    let (server, _) = create_test_server().await;
    let address = server.addr();

    let place_id1 = get_place_id();
    let place_id2 = Uuid::new_v4().to_string();

    for i in 0..3 {
        upload_public_test_image(
            &format!("image-p1-{i}.png"),
            &address.to_string(),
            &place_id1,
        )
        .await;
        upload_public_test_image(
            &format!("image-p2-{i}.png"),
            &address.to_string(),
            &place_id2,
        )
        .await;
    }

    let request_body = serde_json::json!({
        "placesIds": [place_id1, place_id2]
    });

    let response = reqwest::Client::new()
        .post(&format!(
            "http://{}/api/places/images?offset=0&limit=20",
            address
        ))
        .json(&request_body)
        .send()
        .await
        .unwrap()
        .json::<GetMultiplePlacesImagesResponse>()
        .await
        .unwrap();

    assert_eq!(response.place_data.max_images, 6);
    assert_eq!(response.images.len(), 6);
}

fn places_response(total: usize, ids: Vec<&str>) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "total": total,
        "data": ids.into_iter().map(|id| serde_json::json!({"id": id})).collect::<Vec<_>>()
    })
}

#[actix_web::test]
async fn test_get_place_images_with_eth_world_name() {
    let mock_server = MockServer::start().await;

    let place_id_1 = Uuid::new_v4().to_string();
    let place_id_2 = Uuid::new_v4().to_string();

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .and(query_param("names", "test-world.eth"))
        .respond_with(ResponseTemplate::new(200).set_body_json(places_response(
            2,
            vec![place_id_1.as_str(), place_id_2.as_str()],
        )))
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    // Upload 3 images to place_id_1
    for i in 0..3 {
        upload_public_test_image(
            &format!("eth-p1-{i}.png"),
            &address.to_string(),
            &place_id_1,
        )
        .await;
    }

    // Upload 2 images to place_id_2
    for i in 0..2 {
        upload_public_test_image(
            &format!("eth-p2-{i}.png"),
            &address.to_string(),
            &place_id_2,
        )
        .await;
    }

    let response = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/places/test-world.eth/images",
            address
        ))
        .send()
        .await
        .unwrap()
        .json::<GetPlaceImagesResponse>()
        .await
        .unwrap();

    assert_eq!(response.place_data.max_images, 5);
    assert_eq!(response.images.len(), 5);
}

#[actix_web::test]
async fn test_get_place_images_eth_empty_places() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .and(query_param("names", "empty-world.eth"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(places_response(0, vec![])),
        )
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    let response = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/places/empty-world.eth/images",
            address
        ))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let body = response.json::<GetPlaceImagesResponse>().await.unwrap();
    assert_eq!(body.images.len(), 0);
    assert_eq!(body.place_data.max_images, 0);
}

#[actix_web::test]
async fn test_get_place_images_eth_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    let response = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/places/error-world.eth/images",
            address
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 502);

    let body = response.json::<ResponseError>().await.unwrap();
    assert!(body.get_message().contains("failed to resolve world name"));
}

#[actix_web::test]
async fn test_get_place_images_eth_with_pagination_params() {
    let mock_server = MockServer::start().await;

    let place_id = Uuid::new_v4().to_string();

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .and(query_param("names", "paginated-world.eth"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(places_response(1, vec![place_id.as_str()])),
        )
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    // Upload 5 public images
    for i in 0..5 {
        upload_public_test_image(
            &format!("eth-pg-{i}.png"),
            &address.to_string(),
            &place_id,
        )
        .await;
    }

    // Request with offset=2&limit=2
    let response = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/places/paginated-world.eth/images?offset=2&limit=2",
            address
        ))
        .send()
        .await
        .unwrap()
        .json::<GetPlaceImagesResponse>()
        .await
        .unwrap();

    assert_eq!(response.images.len(), 2);
    assert_eq!(response.place_data.max_images, 5);
}

#[actix_web::test]
async fn test_get_place_images_eth_caches_resolution() {
    let mock_server = MockServer::start().await;

    let place_id = Uuid::new_v4().to_string();

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .and(query_param("names", "cache-test.eth"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(places_response(1, vec![place_id.as_str()])),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    // First request
    let response1 = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/places/cache-test.eth/images",
            address
        ))
        .send()
        .await
        .unwrap();
    assert!(response1.status().is_success());

    // Second request — should use cache, mock expects exactly 1 hit
    let response2 = reqwest::Client::new()
        .get(&format!(
            "http://{}/api/places/cache-test.eth/images",
            address
        ))
        .send()
        .await
        .unwrap();
    assert!(response2.status().is_success());
    // wiremock .expect(1) will panic on drop if more than 1 request was made
}

#[actix_web::test]
async fn test_post_multiple_places_images_with_eth_world_name() {
    let mock_server = MockServer::start().await;

    let world_place_id_1 = Uuid::new_v4().to_string();
    let world_place_id_2 = Uuid::new_v4().to_string();
    let regular_place_id = Uuid::new_v4().to_string();

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .and(query_param("names", "multi-world.eth"))
        .respond_with(ResponseTemplate::new(200).set_body_json(places_response(
            2,
            vec![world_place_id_1.as_str(), world_place_id_2.as_str()],
        )))
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    // Upload 2 images to world scene 1
    for i in 0..2 {
        upload_public_test_image(
            &format!("mw-p1-{i}.png"),
            &address.to_string(),
            &world_place_id_1,
        )
        .await;
    }

    // Upload 3 images to world scene 2
    for i in 0..3 {
        upload_public_test_image(
            &format!("mw-p2-{i}.png"),
            &address.to_string(),
            &world_place_id_2,
        )
        .await;
    }

    // Upload 1 image to regular place
    upload_public_test_image("mw-rp-0.png", &address.to_string(), &regular_place_id).await;

    let request_body = serde_json::json!({
        "placesIds": [regular_place_id, "multi-world.eth"]
    });

    let response = reqwest::Client::new()
        .post(&format!(
            "http://{}/api/places/images?offset=0&limit=20",
            address
        ))
        .json(&request_body)
        .send()
        .await
        .unwrap()
        .json::<GetMultiplePlacesImagesResponse>()
        .await
        .unwrap();

    assert_eq!(response.place_data.max_images, 6);
    assert_eq!(response.images.len(), 6);
}

#[actix_web::test]
async fn test_post_multiple_places_images_eth_empty_world() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .and(query_param("names", "empty-multi.eth"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(places_response(0, vec![])),
        )
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    let request_body = serde_json::json!({
        "placesIds": ["empty-multi.eth"]
    });

    let response = reqwest::Client::new()
        .post(&format!(
            "http://{}/api/places/images?offset=0&limit=20",
            address
        ))
        .json(&request_body)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let body = response
        .json::<GetMultiplePlacesImagesResponse>()
        .await
        .unwrap();
    assert_eq!(body.images.len(), 0);
    assert_eq!(body.place_data.max_images, 0);
}

#[actix_web::test]
async fn test_post_multiple_places_images_eth_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/places"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    let request_body = serde_json::json!({
        "placesIds": ["error-multi.eth"]
    });

    let response = reqwest::Client::new()
        .post(&format!(
            "http://{}/api/places/images?offset=0&limit=20",
            address
        ))
        .json(&request_body)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 502);

    let body = response.json::<ResponseError>().await.unwrap();
    assert!(body.get_message().contains("failed to resolve world name"));
}
