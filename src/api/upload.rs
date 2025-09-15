use std::io::Cursor;

use actix_multipart::form::{bytes::Bytes, text::Text, MultipartForm};
use actix_web::{post, web::Data, HttpResponse, Responder};
use actix_web_lab::__reexports::serde_json;
use image::guess_format;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sqlx::types::{chrono, Uuid};
use utoipa::ToSchema;

use crate::{
    api::Image,
    api::{auth::AuthUser, get::UserDataResponse, ForbiddenError, Metadata, ResponseError},
    database::Database,
    sns::{Event, EventSubtype, EventType, SNSPublisher},
    Settings,
};
use std::collections::HashMap;

#[derive(MultipartForm, Debug, ToSchema)]
pub struct Upload {
    #[multipart(limit = "5MiB")]
    #[schema(value_type = String, format = Binary)]
    image: Bytes,
    #[schema(value_type = String, format = Binary)]
    metadata: Bytes,
    #[schema(value_type = bool)]
    is_public: Option<Text<bool>>,
}
#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    pub image: Image,
    #[serde(flatten)]
    pub user_data: UserDataResponse,
}

#[tracing::instrument(skip(upload, bucket, database, settings, sns_publisher))]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    request_body(content = Upload, description = "Image file and metadata in JSON format.", content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Uploaded image with its metadata", body = UploadResponse),
        (status = 400, description = "Bad Request", body = ResponseError),
        (status = 403, description = "Forbidden", body = ForbiddenError),
        (status = 500, description = "Internal Server Error", body = ResponseError),
    )
)]
#[post("/images")]
pub async fn upload_image(
    auth_user: AuthUser,
    bucket: Data<Bucket>,
    database: Data<Database>,
    settings: Data<Settings>,
    sns_publisher: Data<SNSPublisher>,
    upload: MultipartForm<Upload>,
) -> impl Responder {
    let images_count = database
        .get_user_images_count(&auth_user.address, false)
        .await
        .unwrap_or(0);
    if images_count >= settings.max_images_per_user {
        let message = format!(
            "you have reached the limit of {} max images",
            settings.max_images_per_user
        );

        return HttpResponse::Forbidden().json(ForbiddenError::new(&message));
    }

    let (image_bytes, metadata_bytes, is_public) = (
        &upload.image.data,
        &upload.metadata.data,
        upload.is_public.as_ref().map_or(false, |val| val.0),
    );

    let metadata: Metadata = match serde_json::from_slice(metadata_bytes) {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::error!("failed to parse metadata: {}", error);
            return HttpResponse::BadRequest().json(ResponseError::new("invalid metadata"));
        }
    };

    if metadata.user_address != auth_user.address {
        return HttpResponse::BadRequest().json(ResponseError::new("invalid user address"));
    }

    let Some(content_type) = upload
        .image
        .content_type
        .as_ref()
        .map(|content_type| content_type.to_string())
    else {
        return HttpResponse::BadRequest().json(ResponseError::new("invalid content type"));
    };

    match content_type.as_str() {
        "image/png" | "image/jpeg" => {}
        _ => {
            return HttpResponse::BadRequest().json(ResponseError::new("unsupported content type"));
        }
    }

    let Ok(format) = guess_format(image_bytes) else {
        return HttpResponse::BadRequest().json(ResponseError::new("invalid image format"));
    };

    let thumbnail = match image::load_from_memory_with_format(image_bytes, format) {
        Ok(image) => {
            let thumbnail = image.thumbnail(640, 360);
            let mut buffer = Cursor::new(vec![]);
            if let Err(error) = thumbnail.write_to(&mut buffer, format) {
                tracing::error!("couldn't generate thumbnail: {}", error);
                return HttpResponse::BadRequest()
                    .json(ResponseError::new("couldn't create thumbnail"));
            }
            buffer
        }
        Err(error) => {
            tracing::error!("failed to parse image: {}", error);
            return HttpResponse::BadRequest().json(ResponseError::new("invalid image"));
        }
    };

    let image_id = Uuid::new_v4().to_string();
    let image_file_name = upload
        .image
        .file_name
        .clone()
        .unwrap_or("image.png".to_string());

    // check that the image_file_name variable does not has any / or \ characters
    if image_file_name.contains('/') || image_file_name.contains('\\') {
        return HttpResponse::BadRequest().json(ResponseError::new("invalid file name"));
    }

    let image_name = format!("{image_id}-{image_file_name}");
    let thumbnail_name = format!("{image_id}-thumbnail-{image_file_name}");

    if let Err(error) = bucket
        .put_object_with_content_type(image_name.clone(), image_bytes, content_type.as_str())
        .await
    {
        tracing::error!("failed to upload image: {}", error);
        return HttpResponse::InternalServerError()
            .json(ResponseError::new("failed to upload image"));
    }

    if let Err(error) = bucket
        .put_object_with_content_type(
            thumbnail_name.clone(),
            thumbnail.get_ref(),
            content_type.as_str(),
        )
        .await
    {
        tracing::error!("failed to upload thumbnail image: {}", error);
        return HttpResponse::InternalServerError()
            .json(ResponseError::new("failed to upload image"));
    }

    let http_url = &settings.api_url;

    let image = Image {
        id: image_id.clone(),
        url: format!("{http_url}/api/images/{image_name}"),
        thumbnail_url: format!("{http_url}/api/images/{thumbnail_name}"),
        is_public,
        metadata: metadata.clone(),
    };

    if let Err(error) = database.insert_image(&image).await {
        tracing::error!("failed to store image metadata: {}", error);
        return HttpResponse::InternalServerError()
            .json(ResponseError::new("failed to store image metadata"));
    };

    // Publish SNS event
    let mut event_metadata = HashMap::new();
    event_metadata.insert(
        "sceneId".to_string(),
        serde_json::json!(metadata.scene.name),
    );
    event_metadata.insert("realm".to_string(), serde_json::json!(metadata.realm));
    event_metadata.insert(
        "userAddress".to_string(),
        serde_json::json!(metadata.user_address.to_lowercase().to_string()),
    );
    event_metadata.insert("isPublic".to_string(), serde_json::json!(is_public));
    event_metadata.insert("photoId".to_string(), serde_json::json!(image_id));

    // Convert visible_people to the required format
    let users: Vec<serde_json::Value> = metadata
        .visible_people
        .iter()
        .map(|user| {
            serde_json::json!({
                "address": user.user_address,
                "isEmoting": user.is_emoting.unwrap_or(false)
            })
        })
        .collect();
    event_metadata.insert("users".to_string(), serde_json::json!(users));

    let sns_event = Event {
        event_type: EventType::Camera,
        sub_type: EventSubtype::PhotoTaken,
        key: image_id.clone(),
        timestamp: chrono::Utc::now().timestamp() as u64,
        metadata: event_metadata,
    };

    if let Err(error) = sns_publisher.publish(&sns_event).await {
        tracing::error!("failed to publish SNS event: {}", error);
        // Don't return error here as the image was successfully uploaded
    }

    let user_data = UserDataResponse {
        current_images: images_count + 1,
        max_images: settings.max_images_per_user,
    };
    let response = UploadResponse { image, user_data };
    HttpResponse::Ok().json(response)
}
