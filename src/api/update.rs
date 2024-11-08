use actix_web::{
    patch,
    web::{
        Data, Json, Path
    },
    HttpResponse, Responder
};
use serde::Deserialize;

use crate::{
    api::{auth::AuthUser, Metadata, ResponseError},
    database::Database
};

use super::{Scene, User};

#[derive(Deserialize)]
struct UpdateVisibility {
    is_public: bool,
}

#[utoipa::path(
    tag = "images",
    context_path = "/api",
    request_body(content = UpdateVisibility, description = "Update image visibility", content_type = "application/json"),
    responses(
        (status = 200, description = "Image visibility updated successfully"),
        (status = NOT_FOUND, description = "Image was not found"),
        (status = FORBIDDEN, description = "Forbidden"),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to update image visibility"),
    )
)]
#[patch("/images/{id}/visibility")]
pub async fn update_image_visibility(
    user_address: AuthUser,
    image_id: Path<String>,
    database: Data<Database>,
    update: Json<UpdateVisibility>,
) -> impl Responder {
    let image_id = id.into_inner();

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

    image.is_public = update.is_public;
    
    if let Err(error) = database.update_image_visibility(&image).await {
        tracing::error!("failed to update image metadata: {}", error);
        return HttpResponse::InternalServerError()
        .json(ResponseError::new("failed to update image metadata"));
    }
    
    HttpResponse::Ok().finish()
}
