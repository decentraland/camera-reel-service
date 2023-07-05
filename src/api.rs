use actix_multipart_extract::{File, Multipart, MultipartForm};
use actix_web::{
    delete, get, post,
    web::{self, Data, Redirect, ServiceConfig},
    HttpRequest, HttpResponse, Responder,
};
use image::ImageFormat;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{
    auth::{self, get_user_address_from_request},
    database::Database,
    Image, Metadata, Settings,
};

pub fn services(config: &mut ServiceConfig) {
    config.service(
        web::scope("/api")
            .service(upload_image)
            .service(delete_image)
            .service(get_image)
            .service(get_metadata)
            .service(get_user_images)
            .wrap(auth::dcl_auth_middleware([
                "POST:/api/images",
                "DELETE:/api/images",
                "GET:/api/users/{user_address}/images",
            ])),
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

    if !upload
        .metadata
        .photographer
        .eq_ignore_ascii_case(&request_user_address)
    {
        return HttpResponse::Forbidden().body("forbidden");
    }

    let image = &upload.image;
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

    let http_url = std::env::var("HTTP_URL").unwrap_or_else(|_| "http://localhost:5000".into());
    let metadata = &upload.metadata;

    let image = Image {
        id: image_id.clone(),
        metadata: metadata.clone(),
        url: format!("{http_url}/images/{image_id}"),
    };

    if let Err(error) = database.insert_image(&image).await {
        tracing::error!("failed to store image metadata: {}", error);
        return HttpResponse::InternalServerError().body("failed to store image metadata");
    };

    let response = UploadResponse { image_id };
    HttpResponse::Ok().json(response)
}

#[tracing::instrument]
#[delete("/images/{image_id}")]
async fn delete_image(
    req: HttpRequest,
    bucket: Data<Bucket>,
    database: Data<Database>,
    image_id: web::Path<String>,
) -> impl Responder {
    let user_address = match database.get_image_photographer(&image_id).await {
        Ok(image) => image,
        Err(_) => return HttpResponse::NotFound().body("image not found"),
    };

    let request_user_address = match get_user_address_from_request(&req) {
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
#[get("/users/{user_address}/images")]
async fn get_user_images(
    req: HttpRequest,
    database: Data<Database>,
    user_address: web::Path<String>,
) -> impl Responder {
    let request_user_address = match get_user_address_from_request(&req) {
        Ok(address) => address,
        Err(bad_request_response) => return bad_request_response,
    };

    let user_address = user_address.into_inner();
    if !user_address.eq_ignore_ascii_case(&request_user_address) {
        return HttpResponse::Forbidden().body("forbidden");
    }
    let Ok(images) = database.get_user_images(&user_address).await else {
        return HttpResponse::NotFound().body("user not found");
    };
    HttpResponse::Ok().json(images)
}
