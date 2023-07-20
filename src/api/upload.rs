use std::io::Cursor;

use actix_multipart::form::{bytes::Bytes, json::Json, MultipartForm};
use actix_web::{post, web::Data, HttpResponse, Responder};
use actix_web_lab::__reexports::serde_json;
use dcl_crypto::{AuthChain, Authenticator};
use image::guess_format;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sha256::digest;
use sqlx::types::Uuid;
use utoipa::ToSchema;

use crate::{
    api::Image,
    api::{Metadata, ResponseError},
    database::Database,
    Settings,
};

#[derive(MultipartForm, Debug, ToSchema)]
pub struct Upload {
    #[multipart(limit = "5MiB")]
    #[schema(value_type = String, format = Binary)]
    image: Bytes,
    #[schema(value_type = String, format = Binary)]
    metadata: Bytes,
    #[schema(value_type = String)]
    authchain: Option<Json<AuthChain>>,
}

#[derive(Deserialize, Serialize)]
pub struct UploadResponse {
    #[serde(flatten)]
    pub image: Image,
}

#[tracing::instrument(skip(upload))]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    request_body(content = Upload, description = "Image file, metadata attached. If authentication is enabled, an AuthChain must be provided with a valid signature", content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Uploaded image with its metadata", body = Image),
        (status = 400, description = "Bad Request"),
        (status = 500, description = "Internal Server Error"),
    )
)]
#[post("/images")]
pub async fn upload_image(
    bucket: Data<Bucket>,
    database: Data<Database>,
    settings: Data<Settings>,
    upload: MultipartForm<Upload>,
) -> impl Responder {
    let (image_bytes, metadata_bytes) = (&upload.image.data, &upload.metadata.data);

    let metadata: Metadata = match serde_json::from_slice(metadata_bytes) {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::error!("failed to parse metadata: {}", error);
            return HttpResponse::BadRequest().json(ResponseError::new("invalid metadata"));
        }
    };

    if settings.authentication {
        match validate_signature_and_file_hashes(
            image_bytes,
            metadata_bytes,
            &upload.authchain,
            &metadata.user_address,
        )
        .await
        {
            AuthChainValidation::Ok => {}
            error => {
                tracing::error!("failed to validate signature: {:?}", error);
                return HttpResponse::BadRequest().json(ResponseError::new("invalid signature"));
            }
        }
    }

    let Some(content_type) = upload.image
        .content_type
        .as_ref()
        .map(|content_type| content_type.to_string()) else {
            return HttpResponse::BadRequest()
                .json(ResponseError::new("invalid content type"));

    };

    match content_type.as_str() {
        "image/png" | "image/jpeg" => {}
        _ => {
            return HttpResponse::BadRequest().json(ResponseError::new("unsupported content type"));
        }
    }

    let Ok(format) = guess_format(image_bytes) else {
        return HttpResponse::BadRequest()
            .json(ResponseError::new("invalid image format"));
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

    let image_name = format!("{image_id}-{image_file_name}");
    let thumbnail_name = format!("{image_id}-thumbnail-{image_file_name}");

    if let Err(error) = bucket.put_object(image_name.clone(), image_bytes).await {
        tracing::error!("failed to upload image: {}", error);
        return HttpResponse::InternalServerError()
            .json(ResponseError::new("failed to upload image"));
    }

    if let Err(error) = bucket
        .put_object(thumbnail_name.clone(), &thumbnail.get_ref())
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
        metadata: metadata.clone(),
    };

    if let Err(error) = database.insert_image(&image).await {
        tracing::error!("failed to store image metadata: {}", error);
        return HttpResponse::InternalServerError()
            .json(ResponseError::new("failed to store image metadata"));
    };

    let response = UploadResponse { image };
    HttpResponse::Ok().json(response)
}

#[derive(Debug)]
enum AuthChainValidation {
    Ok,
    AuthChainNotFound,
    InvalidSignature,
    InvalidAddress,
}

async fn validate_signature_and_file_hashes(
    image_bytes: &[u8],
    metadata_bytes: &[u8],
    authchain: &Option<Json<AuthChain>>,
    user_address: &str,
) -> AuthChainValidation {
    let Some(auth_chain) = authchain.as_ref() else {
        return AuthChainValidation::AuthChainNotFound;
    };

    let image_hash = digest(image_bytes);
    let metadata_hash = digest(metadata_bytes);
    let payload = format!("{}-{}", image_hash, metadata_hash);

    let authenticator = Authenticator::new();
    match authenticator.verify_signature(auth_chain, &payload).await {
        Ok(address) => {
            if address.to_string().to_lowercase() != user_address.to_lowercase() {
                tracing::debug!(
                    "expected address was {} but metadata address is {}",
                    address,
                    user_address
                );
                tracing::error!("invalid address");
                return AuthChainValidation::InvalidAddress;
            }
        }
        Err(error) => {
            tracing::error!("failed to verify signature: {}", error);
            return AuthChainValidation::InvalidSignature;
        }
    }

    AuthChainValidation::Ok
}
