use actix_multipart_extract::{File, Multipart, MultipartForm};
use actix_web::{post, web::Data, HttpRequest, HttpResponse, Responder};
use image::ImageFormat;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{auth::get_user_address_from_request, database::Database, Image, Metadata, Settings};

#[derive(Deserialize, MultipartForm, Debug)]
pub struct Upload {
    #[multipart(max_size = 5MB)]
    image: File,
    metadata: Metadata,
}

#[derive(Serialize)]
pub struct UploadResponse {
    image: Image,
}

#[tracing::instrument]
#[post("/images")]
pub async fn upload_image(
    req: HttpRequest,
    bucket: Data<Bucket>,
    database: Data<Database>,
    settings: Data<Settings>,
    upload: Multipart<Upload>,
) -> impl Responder {
    let request_user_address = match get_user_address_from_request(&req) {
        Ok(address) => address,
        Err(bad_request_response) => return bad_request_response,
    };

    let (image, metadata) = (&upload.image, &upload.metadata);

    if !metadata
        .user_address
        .eq_ignore_ascii_case(&request_user_address)
    {
        return HttpResponse::Forbidden().body("forbidden");
    }

    if image.content_type != "image/png" {
        return HttpResponse::BadRequest().body("invalid content type");
    }

    // parse image and generate thumbnail
    match image::load_from_memory_with_format(&image.bytes, ImageFormat::Png) {
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
    if bucket.put_object(&image_id, &image.bytes).await.is_err() {
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
