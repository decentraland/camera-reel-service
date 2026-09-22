use actix_cors::Cors;
use actix_web::web::{scope, ServiceConfig};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::database::DBImage;

use self::{
    delete::delete_image,
    docs::generate_docs,
    get::{
        get_image, get_metadata, get_multiple_places_images, get_place_images, get_user_data,
        get_user_images, get_wearable_images,
    },
    update::update_image_visibility,
    upload::upload_image,
};

pub mod auth;
pub mod delete;
mod docs;
pub mod get;
pub mod middlewares;
pub mod update;
pub mod upload;

pub fn services(config: &mut ServiceConfig) {
    let cors = Cors::default()
        .allow_any_origin()
        .allow_any_header()
        .expose_any_header()
        .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE"])
        .max_age(300);

    let docs = generate_docs();

    config.service(docs).service(
        scope("/api")
            .service(upload_image)
            .service(delete_image)
            .service(get_image)
            .service(update_image_visibility)
            .service(get_metadata)
            .service(get_user_images)
            .service(get_user_data)
            .service(get_place_images)
            .service(get_wearable_images)
            .service(get_multiple_places_images)
            .wrap(cors),
    );
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub is_public: bool,
    pub metadata: Metadata,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub is_public: bool,
    pub date_time: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImageWithPlace {
    pub id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub is_public: bool,
    pub date_time: String,
    pub place_id: String,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub user_name: String,
    pub user_address: String,
    pub date_time: String,
    pub realm: String,
    pub scene: Scene,
    pub visible_people: Vec<User>,
    pub place_id: String,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub name: String,
    pub location: Location,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub x: String,
    pub y: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub user_name: String,
    pub user_address: String,
    pub wearables: Vec<String>,
    #[serde(default)]
    pub is_guest: bool,
    #[serde(default)]
    pub is_emoting: Option<bool>,
    /// Skipped rather than written as null so a photo taken by a client that does not report it keeps
    /// the shape it has always had.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_rect: Option<ScreenRect>,
}

/// Where a visible person stands in the photo: normalized to the image, with the origin at its
/// top-left corner. Measured by the explorer when the shot is taken, which is the only moment the
/// avatar's bounds and the camera are both known.
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, ToSchema)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<DBImage> for Image {
    fn from(value: DBImage) -> Self {
        Self {
            id: value.id.to_string(),
            url: value.url,
            thumbnail_url: value.thumbnail_url,
            is_public: value.is_public,
            metadata: value.metadata.0,
        }
    }
}

impl From<DBImage> for GalleryImage {
    fn from(value: DBImage) -> Self {
        Self {
            id: value.id.to_string(),
            url: value.url,
            thumbnail_url: value.thumbnail_url,
            is_public: value.is_public,
            date_time: value.metadata.0.date_time,
        }
    }
}

impl From<DBImage> for GalleryImageWithPlace {
    fn from(value: DBImage) -> Self {
        Self {
            id: value.id.to_string(),
            url: value.url,
            thumbnail_url: value.thumbnail_url,
            is_public: value.is_public,
            date_time: value.metadata.0.date_time,
            place_id: value.metadata.0.place_id,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ForbiddenReason {
    MaxLimitReached,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForbiddenError {
    reason: ForbiddenReason,
    message: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponseError {
    message: String,
}

impl ResponseError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    pub fn get_message(&self) -> &String {
        &self.message
    }
}

impl ForbiddenError {
    pub fn new(message: &str) -> Self {
        Self {
            reason: ForbiddenReason::MaxLimitReached,
            message: message.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERSON_WITH_RECT: &str = r#"{
        "userName": "someone",
        "userAddress": "0x1",
        "wearables": [],
        "isGuest": false,
        "isEmoting": false,
        "screenRect": { "x": 0.25, "y": 0.1, "width": 0.2, "height": 0.6 }
    }"#;

    const PERSON_WITHOUT_RECT: &str = r#"{
        "userName": "someone",
        "userAddress": "0x1",
        "wearables": [],
        "isGuest": false,
        "isEmoting": false
    }"#;

    #[test]
    fn keeps_the_screen_rect_a_photo_was_uploaded_with() {
        let user: User = serde_json::from_str(PERSON_WITH_RECT).unwrap();

        assert_eq!(
            user.screen_rect,
            Some(ScreenRect {
                x: 0.25,
                y: 0.1,
                width: 0.2,
                height: 0.6,
            })
        );

        // The metadata is stored by re-serializing this struct, so what survives the round trip is what
        // reaches the database.
        let stored = serde_json::to_string(&user).unwrap();
        assert!(stored.contains(r#""screenRect":{"x":0.25,"y":0.1,"width":0.2,"height":0.6}"#));
    }

    #[test]
    fn accepts_a_photo_without_a_screen_rect_and_stores_no_null_for_it() {
        let user: User = serde_json::from_str(PERSON_WITHOUT_RECT).unwrap();

        assert_eq!(user.screen_rect, None);

        let stored = serde_json::to_string(&user).unwrap();
        assert!(!stored.contains("screenRect"));
    }
}
