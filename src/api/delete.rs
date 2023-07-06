use actix_web::{
    delete,
    web::{Data, Path},
    HttpRequest, HttpResponse, Responder,
};
use s3::Bucket;

use crate::{auth::get_user_address_from_request, database::Database};

#[tracing::instrument]
#[delete("/images/{image_id}")]
pub async fn delete_image(
    bucket: Data<Bucket>,
    database: Data<Database>,
    request: HttpRequest,
    image_id: Path<String>,
) -> impl Responder {
    let user_address = match database.get_image(&image_id).await {
        Ok(image) => image.user_address,
        Err(_) => return HttpResponse::NotFound().body("image not found"),
    };

    let request_user_address = match get_user_address_from_request(&request) {
        Ok(address) => address,
        Err(bad_request_response) => return bad_request_response,
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
