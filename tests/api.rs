use actix_web_lab::__reexports::serde_json;
use camera_reel_service::api::{
    get::{
        GetGalleryImagesResponse, GetImagesResponse, GetMultiplePlacesImagesResponse,
        GetPlaceImagesResponse, GetWearableImagesResponse, UserDataResponse,
    },
    Image, Metadata, ResponseError, ScreenRect, User,
};
use common::upload_raw_metadata;
use common::upload_test_failing_image;
use common::upload_test_image;
use common::upload_test_image_with_people;
use common::{get_place_id, upload_public_test_image};
use sqlx::types::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::{
    create_other_identity, create_test_identity, create_test_server,
    create_test_server_with_places_url, get_signed_headers, poll_sqs_for_message_with_filter,
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

const WORN_ITEM: &str = "0x0bf152a83a6fc55066c2b664b164ca2916ad38f5-2";

/// A photo of somebody wearing the given URN, written straight to the database: the endpoint is about
/// reading, and an upload would only add a multipart round trip between the fixture and the query.
///
/// The metadata is built from JSON rather than from a struct literal, so a field added to the schema
/// later does not break a test that has nothing to do with it.
async fn insert_photo_of_the_item(context: &common::TestContext, is_public: bool, wearable: &str) {
    let metadata: Metadata = serde_json::from_value(serde_json::json!({
        "userName": "someone",
        "userAddress": "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5",
        "dateTime": "1789615158",
        "realm": "main",
        "placeId": Uuid::new_v4().to_string(),
        "scene": { "name": "Somewhere", "location": { "x": "0", "y": "0" } },
        "visiblePeople": [{
            "userName": "someone",
            "userAddress": "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5",
            "wearables": [wearable],
            "isGuest": false,
            "isEmoting": false
        }]
    }))
    .unwrap();

    let image = Image {
        id: Uuid::new_v4().to_string(),
        url: "https://camera-reel.decentraland.org/image.png".to_string(),
        thumbnail_url: "https://camera-reel.decentraland.org/image-thumbnail.png".to_string(),
        is_public,
        metadata,
    };

    context.database.insert_image(&image).await.unwrap();
}

async fn get_wearable_images(address: &str, item: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(&format!("http://{}/api/wearables/{}/images", address, item))
        .send()
        .await
        .unwrap()
}

#[actix_web::test]
async fn test_get_wearable_images() {
    let (server, context) = create_test_server().await;
    let address = server.addr().to_string();

    // Two copies of the same item, a different item, and the same item on a private photo.
    insert_photo_of_the_item(
        &context,
        true,
        "urn:decentraland:matic:collections-v2:0x0bf152a83a6fc55066c2b664b164ca2916ad38f5:2:105312291668557186697918027683670432318895095400549111254310977559",
    )
    .await;
    insert_photo_of_the_item(
        &context,
        true,
        "urn:decentraland:matic:collections-v2:0x0BF152A83A6FC55066C2B664B164CA2916AD38F5:2:7",
    )
    .await;
    insert_photo_of_the_item(
        &context,
        true,
        "urn:decentraland:matic:collections-v2:0x0bf152a83a6fc55066c2b664b164ca2916ad38f5:3:7",
    )
    .await;
    insert_photo_of_the_item(
        &context,
        false,
        "urn:decentraland:matic:collections-v2:0x0bf152a83a6fc55066c2b664b164ca2916ad38f5:2:9",
    )
    .await;

    let response = get_wearable_images(&address, WORN_ITEM).await;
    assert!(response.status().is_success());

    let response: GetWearableImagesResponse = response.json().await.unwrap();

    // The two public photos of that item, whatever token of it each avatar owns, and never the private
    // one or the photo of the item next to it.
    assert_eq!(response.max_images, 2);
    assert_eq!(response.images.len(), 2);
    assert!(response
        .images
        .iter()
        .all(|image| image.metadata.visible_people[0].wearables[0]
            .to_lowercase()
            .contains("0x0bf152a83a6fc55066c2b664b164ca2916ad38f5:2:")));
}

#[actix_web::test]
async fn test_get_wearable_images_of_an_item_nobody_wears() {
    let (server, _) = create_test_server().await;
    let address = server.addr().to_string();

    let response =
        get_wearable_images(&address, "0x0bf152a83a6fc55066c2b664b164ca2916ad38f5-9").await;

    assert!(response.status().is_success());

    let response: GetWearableImagesResponse = response.json().await.unwrap();

    assert_eq!(response.max_images, 0);
    assert!(response.images.is_empty());
}

#[actix_web::test]
async fn test_get_wearable_images_refuses_a_malformed_item() {
    let (server, _) = create_test_server().await;
    let address = server.addr().to_string();

    let response = get_wearable_images(&address, "not-an-item").await;

    assert_eq!(response.status(), 400);
}

/// Metadata for one person in the shot, with the rect given as raw JSON.
fn metadata_with_screen_rect(screen_rect: serde_json::Value) -> serde_json::Value {
    let mut metadata = serde_json::to_value(Metadata {
        user_address: "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string(),
        place_id: Uuid::new_v4().to_string(),
        realm: "https://realm.org/v1".to_string(),
        visible_people: vec![User {
            user_name: "someone".to_string(),
            user_address: "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string(),
            wearables: vec![],
            is_guest: false,
            is_emoting: Some(false),
            screen_rect: None,
        }],
        ..Default::default()
    })
    .unwrap();
    metadata["visiblePeople"][0]["screenRect"] = screen_rect;
    metadata
}

#[actix_web::test]
async fn test_upload_refuses_a_screen_rect_that_does_not_fit_an_f32() {
    let (server, _) = create_test_server().await;
    let address = server.addr();

    // Parsed as infinity, it would be stored as null and break every feed that reads the row back.
    let (status, message) = upload_raw_metadata(
        &address.to_string(),
        metadata_with_screen_rect(
            serde_json::json!({ "x": 1e39, "y": 0.1, "width": 0.2, "height": 0.6 }),
        ),
    )
    .await;

    assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(message, "invalid metadata");
}

#[actix_web::test]
async fn test_upload_refuses_a_screen_rect_outside_the_image() {
    let (server, _) = create_test_server().await;
    let address = server.addr();

    let (status, message) = upload_raw_metadata(
        &address.to_string(),
        metadata_with_screen_rect(
            serde_json::json!({ "x": 0.9, "y": 0.1, "width": 1000.0, "height": 0.6 }),
        ),
    )
    .await;

    assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(message, "invalid metadata");
}

#[actix_web::test]
async fn test_visible_person_screen_rect_survives_storage() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = Uuid::new_v4().to_string();

    let screen_rect = ScreenRect {
        x: 0.25,
        y: 0.1,
        width: 0.2,
        height: 0.6,
    };

    let id = upload_test_image_with_people(
        "image.png",
        &address.to_string(),
        &place_id,
        vec![User {
            user_name: "someone".to_string(),
            user_address: "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string(),
            wearables: vec![],
            is_guest: false,
            is_emoting: Some(false),
            screen_rect: Some(screen_rect),
        }],
    )
    .await;

    // The metadata is stored by re-serializing it, so reading it back is what proves the field is kept
    // rather than dropped on the way in.
    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let image: Image = response.json().await.unwrap();

    assert_eq!(image.metadata.visible_people.len(), 1);
    assert_eq!(
        image.metadata.visible_people[0].screen_rect,
        Some(screen_rect)
    );
}

#[actix_web::test]
async fn test_delete_image() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = Uuid::new_v4().to_string();

    let id = upload_test_image("image.png", &address.to_string(), &place_id).await;

    // Metadata is public for every image, so no auth is required to read it.
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

    // Check if visibility was updated. The image is now private, but its metadata stays
    // publicly readable (privacy only hides it from the user's public gallery listing).
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
async fn test_get_public_image_metadata_without_auth_succeeds() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = get_place_id();

    let id = upload_public_test_image("public-meta.png", &address.to_string(), &place_id).await;

    // Public images expose their metadata to anyone, no auth required.
    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let image = response.json::<Image>().await.unwrap();
    assert_eq!(image.is_public, true);
}

#[actix_web::test]
async fn test_get_private_image_metadata_without_auth_succeeds() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = get_place_id();

    let id = upload_test_image("private-noauth.png", &address.to_string(), &place_id).await;

    // A private image is still viewable by anyone with the link (privacy only hides it
    // from the user's public gallery), so its metadata is served without auth. This is
    // what powers sharing a photo to social networks.
    let response = reqwest::Client::new()
        .get(&format!("http://{}/api/images/{}/metadata", address, id))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let image = response.json::<Image>().await.unwrap();
    assert_eq!(image.is_public, false);
}

#[actix_web::test]
async fn test_get_private_image_metadata_as_owner_succeeds() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = get_place_id();

    let id = upload_test_image("private-owner.png", &address.to_string(), &place_id).await;

    // The owner can read their own private image metadata when authenticated.
    let path = format!("/api/images/{id}/metadata");
    let headers = get_signed_headers(create_test_identity(), "get", &path, "");
    let response = reqwest::Client::new()
        .get(&format!("http://{}{}", address, path))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let image = response.json::<Image>().await.unwrap();
    assert_eq!(image.is_public, false);
}

#[actix_web::test]
async fn test_get_private_image_metadata_as_non_owner_succeeds() {
    let (server, _) = create_test_server().await;
    let address = server.addr();
    let place_id = get_place_id();

    let id = upload_test_image("private-other.png", &address.to_string(), &place_id).await;

    // A different authenticated user can also read the metadata: a photo is shareable
    // regardless of who requests it. Authentication never changes the outcome here.
    let path = format!("/api/images/{id}/metadata");
    let headers = get_signed_headers(create_other_identity(), "get", &path, "");
    let response = reqwest::Client::new()
        .get(&format!("http://{}{}", address, path))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let image = response.json::<Image>().await.unwrap();
    assert_eq!(image.is_public, false);
}

#[actix_web::test]
async fn test_post_multiple_places_images_too_many_ids() {
    let (server, _) = create_test_server().await;
    let address = server.addr();

    // 101 IDs exceeds the cap and must be rejected before any DB work.
    let places_ids: Vec<String> = (0..101).map(|_| Uuid::new_v4().to_string()).collect();
    let request_body = serde_json::json!({ "placesIds": places_ids });

    let response = reqwest::Client::new()
        .post(&format!("http://{}/api/places/images", address))
        .json(&request_body)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = response.json::<ResponseError>().await.unwrap();
    assert!(body.get_message().contains("too many place IDs"));
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
        .respond_with(ResponseTemplate::new(200).set_body_json(places_response(0, vec![])))
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
            ResponseTemplate::new(200).set_body_json(places_response(1, vec![place_id.as_str()])),
        )
        .mount(&mock_server)
        .await;

    let (server, _) = create_test_server_with_places_url(&mock_server.uri()).await;
    let address = server.addr();

    // Upload 5 public images
    for i in 0..5 {
        upload_public_test_image(&format!("eth-pg-{i}.png"), &address.to_string(), &place_id).await;
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
            ResponseTemplate::new(200).set_body_json(places_response(1, vec![place_id.as_str()])),
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
        .respond_with(ResponseTemplate::new(200).set_body_json(places_response(0, vec![])))
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
