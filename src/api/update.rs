use actix_web::{
    patch,
    web::{
        Data, Json, Path
    },
    HttpResponse, Responder
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    api::{auth::AuthUser, ResponseError},
    database::Database
};

#[derive(Deserialize, ToSchema)]
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
    let image_id = image_id.into_inner();

    println!("Updating image visibility: {}, {}", image_id, update.is_public);

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
    
    if let Err(error) = database.update_image_visibility(&image_id, &update.is_public).await {
        tracing::error!("failed to update image metadata: {}", error);
        return HttpResponse::InternalServerError()
        .json(ResponseError::new("failed to update image metadata"));
    }
    
    HttpResponse::Ok().finish()
}
