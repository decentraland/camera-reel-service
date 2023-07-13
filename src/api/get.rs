use actix_web::{
    get,
    web::{Data, Path, Redirect},
    HttpResponse, Responder,
};
use actix_web_lab::extract::Query;
use serde::Deserialize;

use crate::{api::auth::AuthUserAddress, database::Database, Image, Settings};

#[tracing::instrument]
#[get("/images/{image_id}")]
async fn get_image(settings: Data<Settings>, image_id: Path<String>) -> impl Responder {
    Redirect::to(format!("{}/{}", settings.bucket_url, image_id))
}

#[tracing::instrument]
#[get("/images/{image_id}/metadata")]
async fn get_metadata(database: Data<Database>, image_id: Path<String>) -> impl Responder {
    let image_id = image_id.into_inner();
    match database.get_image(&image_id).await {
        Ok(image) => {
            let image: Image = image.into();
            HttpResponse::Ok().json(image)
        }
        Err(e) => {
            tracing::debug!("Image not found: {e:?}");
            HttpResponse::NotFound().body("image not found")
        }
    }
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
    user_address: AuthUserAddress,
    query_params: Query<GetImagesQuery>,
    database: Data<Database>,
) -> impl Responder {
    let AuthUserAddress { user_address } = user_address;
    let GetImagesQuery { offset, limit } = query_params.into_inner();

    let Ok(images) = database.get_user_images(&user_address, offset as i64, limit as i64).await else {
        return HttpResponse::NotFound().body("user not found");
    };
    let images = images.into_iter().map(Image::from).collect::<Vec<_>>();
    HttpResponse::Ok().json(images)
}
