use actix_multipart_extract::{File, Multipart, MultipartForm};
use actix_web::{
    delete, get, post,
    web::{self, ServiceConfig},
    HttpResponse, Responder,
};
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Metadata {
    pub photographer: String,
    pub tags: Vec<String>,
    pub users: Vec<String>,
    pub wearables: Vec<String>,
}

#[derive(Deserialize, MultipartForm, Debug)]
pub struct Upload {
    #[multipart(max_size = 5MB)]
    _image: File,
    _metadata: Metadata,
}

#[tracing::instrument]
#[post("/images")]
async fn upload_image(upload: Multipart<Upload>) -> impl Responder {
    HttpResponse::Ok().body("unimplemented")
}

#[tracing::instrument]
#[delete("/images/{image_id}")]
async fn delete_image() -> impl Responder {
    HttpResponse::Ok().body("unimplemented")
}

#[tracing::instrument]
#[get("/images/{image_id}")]
async fn get_image(image_id: web::Path<String>) -> impl Responder {
    HttpResponse::Ok().body("unimplemented")
}

#[tracing::instrument]
#[get("/images/{image_id}/metadata")]
async fn get_metadata(image_id: web::Path<String>) -> impl Responder {
    HttpResponse::Ok().body("unimplemented")
}

#[tracing::instrument]
#[get("/user/{user_address}/images")]
async fn get_user_images() -> impl Responder {
    HttpResponse::Ok().body("unimplemented")
}
