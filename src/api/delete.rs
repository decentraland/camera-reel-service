use actix_web::{
    delete,
    web::{Data, Path},
    HttpResponse, Responder,
};
use s3::Bucket;

use crate::{api::auth::AuthUserAddress, database::Database};

#[utoipa::path(
    tag = "images",
    context_path = "/api",
    responses(
        (status = 200, description = "Image deleted"),
        (status = NOT_FOUND, description = "Image was not found"),
        (status = FORBIDDEN, description = "Forbidden"),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to delete image"),
    ),
    params(
        ("image_id" = u64, Path, description = "Image database id to delete"),
    )
)]
#[tracing::instrument]
#[delete("/images/{image_id}")]
pub async fn delete_image(
    user_address: AuthUserAddress,
    bucket: Data<Bucket>,
    database: Data<Database>,
    image_id: Path<String>,
) -> impl Responder {
    let AuthUserAddress {
        user_address: request_user_address,
    } = user_address;

    let user_address = match database.get_image(&image_id).await {
        Ok(image) => image.user_address,
        Err(_) => return HttpResponse::NotFound().body("image not found"),
    };

    if !user_address.eq_ignore_ascii_case(&request_user_address) {
        return HttpResponse::Forbidden().body("forbidden");
    }

    let image_id = image_id.into_inner();
    if let Err(error) = database.delete_image(&image_id).await {
        tracing::error!("failed to delete image metadata: {}", error);
        return HttpResponse::InternalServerError().body("failed to delete image");
    };
    if let Err(error) = bucket.delete_object(image_id).await {
        tracing::error!("failed to delete image from bucket: {}", error);
        return HttpResponse::InternalServerError().body("failed to delete image");
    };

    HttpResponse::Ok().body("image deleted")
}
