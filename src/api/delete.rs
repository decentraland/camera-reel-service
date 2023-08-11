use actix_web::{
    delete,
    web::{Data, Path},
    HttpResponse, Responder,
};
use s3::Bucket;

use crate::{
    api::{auth::AuthUser, ResponseError},
    database::Database,
    Settings,
};

use super::get::UserDataResponse;

#[tracing::instrument(skip(bucket, database))]
#[utoipa::path(
    tag = "images",
    context_path = "/api",
    responses(
        (status = 200, description = "Image deleted", body = UserDataResponse),
        (status = NOT_FOUND, description = "Image was not found"),
        (status = FORBIDDEN, description = "Forbidden"),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to delete image"),
    ),
    params(
        ("image_id" = u64, Path, description = "Image database id to delete"),
    )
)]
#[delete("/images/{image_id}")]
pub async fn delete_image(
    user_address: AuthUser,
    image_id: Path<String>,
    bucket: Data<Bucket>,
    database: Data<Database>,
    settings: Data<Settings>,
) -> impl Responder {
    let AuthUser {
        address: request_user_address,
    } = user_address;

    let image = match database.get_image(&image_id).await {
        Ok(image) => image,
        Err(_) => return HttpResponse::NotFound().json(ResponseError::new("image not found")),
    };

    if !image
        .user_address
        .eq_ignore_ascii_case(&request_user_address)
    {
        return HttpResponse::Forbidden().json(ResponseError::new("forbidden"));
    }

    let image_id = image_id.into_inner();
    if let Err(error) = database.delete_image(&image_id).await {
        tracing::error!("failed to delete image metadata: {}", error);
        return HttpResponse::InternalServerError()
            .json(ResponseError::new("failed to delete image"));
    };

    match image.url.split('/').last() {
        Some(image_file_name) => {
            if let Err(error) = bucket.delete_object(image_file_name).await {
                tracing::error!("failed to delete image from bucket: {}", error);
                return HttpResponse::InternalServerError()
                    .json(ResponseError::new("failed to delete image"));
            };
        }
        None => {
            tracing::debug!("No image to delete");
        }
    }

    match image.thumbnail_url.split('/').last() {
        Some(thumbnail_file_name) => {
            if let Err(error) = bucket.delete_object(thumbnail_file_name).await {
                tracing::error!("failed to delete thumbnail image from bucket: {}", error);
                return HttpResponse::InternalServerError()
                    .json(ResponseError::new("failed to delete thumbnail image"));
            };
        }
        None => {
            tracing::debug!("No thumbnail image to delete");
        }
    }

    let current_images = database
        .get_user_images_count(&image.user_address)
        .await
        .unwrap_or(0);

    HttpResponse::Ok().json(UserDataResponse {
        max_images: settings.max_images_per_user,
        current_images,
    })
}
