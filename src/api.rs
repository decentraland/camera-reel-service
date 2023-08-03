use actix_cors::Cors;
use actix_web::web::{scope, ServiceConfig};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::database::DBImage;

use self::{
    delete::delete_image,
    docs::generate_docs,
    get::{get_image, get_metadata, get_user_images},
    upload::upload_image,
};

pub mod auth;
pub mod delete;
mod docs;
pub mod get;
pub mod middlewares;
pub mod upload;

pub fn services(config: &mut ServiceConfig) {
    let cors = Cors::default()
        .allow_any_origin()
        .allow_any_header()
        .expose_any_header()
        .allowed_methods(vec!["GET", "POST", "DELETE"])
        .max_age(300);

    let docs = generate_docs();

    config.service(docs).service(
        scope("/api")
            .service(upload_image)
            .service(delete_image)
            .service(get_image)
            .service(get_metadata)
            .service(get_user_images)
            .wrap(cors),
    );
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub metadata: Metadata,
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
}

impl From<DBImage> for Image {
    fn from(value: DBImage) -> Self {
        Self {
            id: value.id.to_string(),
            url: value.url,
            thumbnail_url: value.thumbnail_url,
            metadata: value.metadata.0,
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
}

impl ForbiddenError {
    pub fn new(message: &str) -> Self {
        Self {
            reason: ForbiddenReason::MaxLimitReached,
            message: message.to_string(),
        }
    }
}
