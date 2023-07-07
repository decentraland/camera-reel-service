use actix_multipart_extract::{File, Multipart, MultipartForm};
use actix_web::{post, web::Data, HttpResponse, Responder};
use actix_web_lab::__reexports::serde_json;
use image::guess_format;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{database::Database, Image, Metadata, Settings};

#[derive(Deserialize, MultipartForm, Debug)]
pub struct Upload {
    #[multipart(max_size = 5MB)]
    image: File,
    metadata: File,
}

#[derive(Deserialize, Serialize)]
pub struct UploadResponse {
    pub image: Image,
}

#[tracing::instrument]
#[post("/images")]
pub async fn upload_image(
    // user_address: AuthUserAddress,
    bucket: Data<Bucket>,
    database: Data<Database>,
    settings: Data<Settings>,
    upload: Multipart<Upload>,
) -> impl Responder {
    // let AuthUserAddress { user_address } = user_address;
    let (image, metadata) = (&upload.image, &upload.metadata);

    let metadata = match parse_metadata(metadata) {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::error!("failed to parse metadata: {}", error);
            return HttpResponse::BadRequest().body("invalid metadata");
        }
    };

    // if !metadata.user_address.eq_ignore_ascii_case(&user_address) {
    //     return HttpResponse::Forbidden().body("forbidden");
    // }

    if image.content_type != "image/png" && image.content_type != "image/jpeg" {
        return HttpResponse::BadRequest().body("invalid content type");
    }

    let Ok(format) = guess_format(&image.bytes) else {
        return HttpResponse::BadRequest().body("invalid image format");
    };
    match image::load_from_memory_with_format(&image.bytes, format) {
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
    if let Err(error) = bucket.put_object(&image_id, &image.bytes).await {
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

fn parse_metadata(metadata: &File) -> Result<Metadata, serde_json::Error> {
    let metadata: Metadata = serde_json::from_slice(&metadata.bytes)?;

    Ok(metadata)
}
