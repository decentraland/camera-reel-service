use actix_web::{
    get,
    web::{Data, Path, Redirect},
    HttpResponse, Responder,
};
use actix_web_lab::extract::Query;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

use crate::{
    api::{Image, ResponseError},
    database::Database,
    Settings,
};

#[tracing::instrument]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    responses(
        (status = 200, description = "Get image", body = Image),
    )
)]
#[get("/images/{image_id}")]
async fn get_image(settings: Data<Settings>, image_id: Path<String>) -> impl Responder {
    Redirect::to(format!("{}/{}", settings.bucket_url, image_id))
}

#[tracing::instrument]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    responses(
        (status = 200, description = "Get image metadata", body = Image),
        (status = 404, description = "Not found")
    )
)]
#[get("/images/{image_id}/metadata")]
async fn get_metadata(database: Data<Database>, image_id: Path<String>) -> impl Responder {
    let image_id = image_id.into_inner();
    match database.get_image(&image_id).await {
        Ok(image) => {
            let image: Image = image.into();
            HttpResponse::Ok().json(image)
        }
        Err(e) => {
            tracing::debug!("Image not found: {e:?}");
            HttpResponse::NotFound().json(ResponseError::new("image not found"))
        }
    }
}

#[derive(Deserialize, Debug, IntoParams)]
struct GetImagesQuery {
    #[serde(default = "default_offset")]
    offset: u64,
    #[serde(default = "default_limit")]
    limit: u64,
}

fn default_offset() -> u64 {
    0
}

fn default_limit() -> u64 {
    20
}

#[derive(Deserialize, Serialize)]
pub struct GetImagesResponse {
    pub images: Vec<Image>,
    pub current_images: u64,
    pub max_images: u64,
}

// Commenting this in favour of unauthorized endpoint for testing purposes
// Re-enable this one when is ready
//
// #[tracing::instrument]
// #[utoipa::path(get, context_path = "/api")]
// #[get("/users/me/images")]
// async fn get_user_images(
//     user_address: AuthUserAddress,
//     query_params: Query<GetImagesQuery>,
//     database: Data<Database>,
// ) -> impl Responder {
//     let AuthUserAddress { user_address } = user_address;
//     let GetImagesQuery { offset, limit } = query_params.into_inner();
//
//     let Ok(images) = database.get_user_images(&user_address, offset as i64, limit as i64).await else {
//         return HttpResponse::NotFound().body("user not found");
//     };
//     let images = images.into_iter().map(Image::from).collect::<Vec<_>>();
//     HttpResponse::Ok().json(images)
// }

#[tracing::instrument]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    params(
        GetImagesQuery
    ),
    responses(
        (status = 200, description = "List images metadatas for a given user", body = GetImagesResponse),
        (status = 404, description = "Not found")
    )
)]
#[get("/users/{user_address}/images")]
async fn get_user_images(
    user_address: Path<String>,
    query_params: Query<GetImagesQuery>,
    settings: Data<Settings>,
    database: Data<Database>,
) -> impl Responder {
    let user_address = user_address.into_inner();
    let GetImagesQuery { offset, limit } = query_params.into_inner();

    let Ok(images_count) = database.get_user_images_count(&user_address).await else {
        return HttpResponse::NotFound().json(ResponseError::new("user not found"));
    };

    let Ok(images) = database.get_user_images(&user_address, offset as i64, limit as i64).await else {
        return HttpResponse::NotFound().json(ResponseError::new("user not found"));
    };

    let images = images.into_iter().map(Image::from).collect::<Vec<_>>();
    HttpResponse::Ok().json(GetImagesResponse {
        images,
        current_images: images_count,
        max_images: settings.max_images_per_user,
    })
}
