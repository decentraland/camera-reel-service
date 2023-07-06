use actix_web::{
    get,
    web::{Data, Path, Redirect},
    HttpRequest, HttpResponse, Responder,
};
use actix_web_lab::extract::Query;
use serde::Deserialize;

use crate::{auth::get_user_address_from_request, database::Database, Image, Settings};

#[tracing::instrument]
#[get("/images/{image_id}")]
async fn get_image(settings: Data<Settings>, image_id: Path<String>) -> impl Responder {
    Redirect::to(format!("{}/{}", settings.bucket_url, image_id))
}

#[tracing::instrument]
#[get("/images/{image_id}/metadata")]
async fn get_metadata(database: Data<Database>, image_id: Path<String>) -> impl Responder {
    let image_id = image_id.into_inner();
    let Ok(image) = database.get_image(&image_id).await else {
        return HttpResponse::NotFound().body("image not found");
    };

    let image: Image = image.into();

    HttpResponse::Ok().json(image)
}

#[derive(Deserialize, Debug)]
struct GetImagesQuery {
    #[serde(default = "default_offset")]
    offset: u64,
    #[serde(default = "default_limit")]
    limit: u64,
}

fn default_offset() -> u64 {
    0
}

fn default_limit() -> u64 {
    20
}

#[tracing::instrument]
#[get("/users/me/images")]
async fn get_user_images(
    req: HttpRequest,
    query_params: Query<GetImagesQuery>,
    database: Data<Database>,
) -> impl Responder {
    let user_address = match get_user_address_from_request(&req) {
        Ok(address) => address,
        Err(bad_request_response) => return bad_request_response,
    };
    let GetImagesQuery { offset, limit } = query_params.into_inner();

    let Ok(images) = database.get_user_images(&user_address, offset, limit).await else {
        return HttpResponse::NotFound().body("user not found");
    };
    let images = images.into_iter().map(Image::from).collect::<Vec<_>>();
    HttpResponse::Ok().json(images)
}
