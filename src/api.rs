use actix_web::web::{scope, ServiceConfig};

use crate::{database::DBImage, Image};

use self::{
    delete::delete_image,
    get::{get_image, get_metadata, get_user_images},
    upload::upload_image,
};

mod auth;
mod delete;
mod get;
mod upload;

pub fn services(config: &mut ServiceConfig) {
    config.service(
        scope("/api")
            .service(upload_image)
            .service(delete_image)
            .service(get_image)
            .service(get_metadata)
            .service(get_user_images),
    );
}

impl From<DBImage> for Image {
    fn from(value: DBImage) -> Self {
        Self {
            id: value.id,
            url: value.url,
            metadata: value.metadata.0,
        }
    }
}
