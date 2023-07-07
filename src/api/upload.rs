use actix_multipart::form::{bytes::Bytes, json::Json, MultipartForm};
use actix_web::{post, web::Data, HttpResponse, Responder};
use actix_web_lab::__reexports::serde_json;
use dcl_crypto::{AuthChain, Authenticator};
use image::guess_format;
use ipfs_hasher::IpfsHasher;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{database::Database, Image, Metadata, Settings};

#[derive(MultipartForm, Debug)]
pub struct Upload {
    #[multipart(limit = "5MiB")]
    image: Bytes,
    metadata: Bytes,
    authchain: Option<Json<AuthChain>>,
}

#[derive(Deserialize, Serialize)]
pub struct UploadResponse {
    pub image: Image,
}

#[tracing::instrument(skip(upload))]
#[post("/images")]
pub async fn upload_image(
    bucket: Data<Bucket>,
    database: Data<Database>,
    settings: Data<Settings>,
    upload: MultipartForm<Upload>,
) -> impl Responder {
    let (image_bytes, metadata_bytes) = (&upload.image.data, &upload.metadata.data);

    let metadata = match parse_metadata(metadata_bytes) {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::error!("failed to parse metadata: {}", error);
            return HttpResponse::BadRequest().body("invalid metadata");
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
                return HttpResponse::BadRequest().body("invalid signature");
            }
        }
    }

    let Some(content_type) = upload.image
        .content_type
        .as_ref()
        .map(|content_type| content_type.to_string()) else {
        return HttpResponse::BadRequest().body("invalid content type");
    };

    match content_type.as_str() {
        "image/png" | "image/jpeg" => {}
        _ => {
            return HttpResponse::BadRequest().body("unsupported content type");
        }
    }

    let Ok(format) = guess_format(image_bytes) else {
        return HttpResponse::BadRequest().body("invalid image format");
    };

    match image::load_from_memory_with_format(image_bytes, format) {
        Ok(_image) => {
            // TODO: should we generate a thumbnail?
            // TODO: let thumbnail = image.thumbnail(100, 100);
            // TODO: store thumbnail in s3?
        }
        Err(error) => {
            tracing::error!("failed to parse image: {}", error);
            return HttpResponse::BadRequest().body("invalid image");
        }
    }

    let image_id = Uuid::new_v4().to_string();
    if let Err(error) = bucket.put_object(&image_id, image_bytes).await {
        tracing::error!("failed to upload image: {}", error);
        return HttpResponse::InternalServerError().body("failed to upload image");
    }

    let http_url = &settings.api_url;

    let image = Image {
        id: image_id.clone(),
        url: format!("{http_url}/images/{image_id}"),
        metadata: metadata.clone(),
    };

    if let Err(error) = database.insert_image(&image).await {
        tracing::error!("failed to store image metadata: {}", error);
        return HttpResponse::InternalServerError().body("failed to store image metadata");
    };

    let response = UploadResponse { image };
    HttpResponse::Ok().json(response)
}

fn parse_metadata(metadata: &[u8]) -> Result<Metadata, serde_json::Error> {
    let metadata: Metadata = serde_json::from_slice(metadata)?;

    Ok(metadata)
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

    let ipfs_hasher = IpfsHasher::default();
    let image_hash = ipfs_hasher.compute(image_bytes);
    let metadata_hash = ipfs_hasher.compute(metadata_bytes);
    let payload = format!("{}-{}", image_hash, metadata_hash);

    let authenticator = Authenticator::new();
    match authenticator.verify_signature(auth_chain, &payload).await {
        Ok(address) => {
            if address.to_string().to_lowercase() != user_address.to_lowercase() {
                tracing::debug!(
                    "expected address is {} but metadata has address {}",
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
