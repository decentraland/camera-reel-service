use actix_cors::Cors;
use actix_web::web::{scope, ServiceConfig};

use crate::{database::DBImage, Image};

use self::{
    delete::delete_image,
    get::{get_image, get_metadata, get_user_images},
    upload::upload_image,
};

pub mod auth;
pub mod delete;
pub mod get;
pub mod upload;

pub fn services(config: &mut ServiceConfig) {
    let cors = Cors::default()
        .allow_any_origin()
        .allow_any_header()
        .expose_any_header()
        .allowed_methods(vec!["GET", "POST", "DELETE"])
        .max_age(300);

    config.service(
        scope("/api")
            .service(upload_image)
            .service(delete_image)
            .service(get_image)
            .service(get_metadata)
            .service(get_user_images)
            .wrap(cors),
    );
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
