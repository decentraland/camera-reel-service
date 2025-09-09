use actix_web::{
    patch,
    web::{Data, Json, Path},
    HttpResponse, Responder,
};
use actix_web_lab::__reexports::serde_json;
use serde::Deserialize;
use sqlx::types::chrono;
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::{
    api::{auth::AuthUser, ResponseError},
    database::Database,
    sns::{Event, EventSubtype, EventType, SNSPublisher},
};

#[derive(Deserialize, ToSchema)]
pub struct UpdateVisibility {
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
    sns_publisher: Data<SNSPublisher>,
    update: Json<UpdateVisibility>,
) -> impl Responder {
    let image_id = image_id.into_inner();

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

    if image.is_public == update.is_public {
        return HttpResponse::Ok().finish();
    }

    if let Err(error) = database
        .update_image_visibility(&image_id, &update.is_public)
        .await
    {
        tracing::error!("failed to update image metadata: {}", error);
        return HttpResponse::InternalServerError()
            .json(ResponseError::new("failed to update image metadata"));
    }

    // Publish SNS event for privacy settings change
    let mut event_metadata = HashMap::new();
    event_metadata.insert("photoId".to_string(), serde_json::json!(image_id));
    event_metadata.insert("isPublic".to_string(), serde_json::json!(update.is_public));

    let sns_event = Event {
        event_type: EventType::Camera,
        sub_type: EventSubtype::PhotoPrivacyChanged,
        key: image_id.clone(),
        timestamp: chrono::Utc::now().timestamp() as u64,
        metadata: event_metadata,
    };

    if let Err(error) = sns_publisher.publish(&sns_event).await {
        tracing::error!("failed to publish SNS event: {}", error);
        // Don't return error here as the update was successful
    }

    HttpResponse::Ok().finish()
}
