use actix_multipart_extract::{File, Multipart, MultipartForm};
use actix_web::{
    delete, get, post,
    web::{self, Data, Redirect, ServiceConfig},
    HttpResponse, Responder,
};
use image::ImageFormat;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{database::Database, Image, Metadata, Settings};

pub fn services(config: &mut ServiceConfig) {
    config.service(
        web::scope("/api")
            .service(upload_image)
            .service(delete_image)
            .service(get_image)
            .service(get_metadata)
            .service(get_user_images),
    );
}

#[derive(Deserialize, MultipartForm, Debug)]
pub struct Upload {
    #[multipart(max_size = 5MB)]
    image: File,
    metadata: Metadata,
}

#[derive(Serialize)]
pub struct UploadResponse {
    image_id: String,
}

#[tracing::instrument]
#[post("/images")]
async fn upload_image(
    bucket: Data<Bucket>,
    database: Data<Database>,
    settings: Data<Settings>,
    upload: Multipart<Upload>,
) -> impl Responder {
    let image = &upload.image;
    if image.content_type != "image/png" {
        return HttpResponse::BadRequest().body("invalid content type");
    }

    // TODO: validate photographer

    // parse image and generate thumbnail
    match image::load_from_memory_with_format(&image.bytes, ImageFormat::Png) {
        Ok(_image) => {
            // let thumbnail = image.thumbnail(100, 100);
            // store thumbnail in s3?
        }
        Err(_) => {
            return HttpResponse::BadRequest().body("invalid image");
        }
    }

    let image_id = Uuid::new_v4().to_string();
    if bucket.put_object(&image_id, &image.bytes).await.is_err() {
        return HttpResponse::InternalServerError().body("failed to upload image");
    }

    let http_url = std::env::var("HTTP_URL").unwrap_or_else(|_| "http://localhost:5000".into());
    let metadata = &upload.metadata;

    let image = Image {
        id: image_id.clone(),
        metadata: metadata.clone(),
        url: format!("{http_url}/images/{image_id}"),
    };
    if database.insert_image(&image).await.is_err() {
        return HttpResponse::InternalServerError().body("failed to store image metadata");
    };

    let response = UploadResponse { image_id };
    HttpResponse::Ok().json(response)
}

#[tracing::instrument]
#[delete("/images/{image_id}")]
async fn delete_image(
    bucket: Data<Bucket>,
    database: Data<Database>,
    image_id: web::Path<String>,
) -> impl Responder {
    let image_id = image_id.into_inner();
    // TODO: authenticate, only owner can delete the image
    if database.delete_image(&image_id).await.is_err() {
        return HttpResponse::InternalServerError().body("failed to delete image");
    };
    if bucket.delete_object(image_id).await.is_err() {
        return HttpResponse::InternalServerError().body("failed to delete image");
    };

    HttpResponse::Ok().body("image deleted")
}

#[tracing::instrument]
#[get("/images/{image_id}")]
async fn get_image(settings: Data<Settings>, image_id: web::Path<String>) -> impl Responder {
    Redirect::to(format!("{}/{}", settings.bucket_url, image_id))
}

#[tracing::instrument]
#[get("/images/{image_id}/metadata")]
async fn get_metadata(database: Data<Database>, image_id: web::Path<String>) -> impl Responder {
    let image_id = image_id.into_inner();
    let Ok(image) = database.get_image(&image_id).await else {
        return HttpResponse::NotFound().body("image not found");
    };

    HttpResponse::Ok().json(image)
}

#[tracing::instrument]
#[get("/user/{user_address}/images")]
async fn get_user_images(
    database: Data<Database>,
    user_address: web::Path<String>,
) -> impl Responder {
    let user_address = user_address.into_inner();
    let Ok(images) = database.get_user_images(&user_address).await else {
        return HttpResponse::NotFound().body("user not found");
    };
    HttpResponse::Ok().json(images)
}
