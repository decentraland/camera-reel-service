use actix_web::{
    get,
    web::{Data, Path, Redirect},
    FromRequest, HttpRequest, HttpResponse, Responder,
};
use actix_web_lab::extract::Query;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    api::{auth::AuthUser, Image, ResponseError},
    database::Database,
    Environment, Settings,
};

#[tracing::instrument]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    responses(
        (status = 200, description = "Get image", body = Image),
    )
)]
#[get("/images/{image_id}")]
async fn get_image(settings: Data<Settings>, image_id: Path<String>) -> impl Responder {
    Redirect::to(format!("{}/{}", settings.bucket_url, image_id))
}

#[tracing::instrument]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    responses(
        (status = 200, description = "Get image metadata", body = Image),
        (status = 404, description = "Not found")
    )
)]
#[get("/images/{image_id}/metadata")]
async fn get_metadata(database: Data<Database>, image_id: Path<String>) -> impl Responder {
    let image_id = image_id.into_inner();
    match database.get_image(&image_id).await {
        Ok(image) => {
            let image: Image = image.into();
            HttpResponse::Ok().json(image)
        }
        Err(sqlx::Error::ColumnDecode { source, .. }) => {
            tracing::debug!("Couldn't decode image metadata: {source:?}");
            HttpResponse::InternalServerError().json(ResponseError::new("couldn't decode image"))
        }
        Err(e) => {
            tracing::debug!("Image not found: {e:?}");
            HttpResponse::NotFound().json(ResponseError::new("image not found"))
        }
    }
}

#[derive(Deserialize, Debug, IntoParams)]
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

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDataResponse {
    pub current_images: u64,
    pub max_images: u64,
}

#[tracing::instrument]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    responses(
        (status = 200, description = "Get user data", body = UserDataResponse),
        (status = 404, description = "Not found")
    )
)]
#[get("/users/{user_address}")]
async fn get_user_data(
    user_address: Path<String>,
    query_params: Query<GetImagesQuery>,
    request: HttpRequest,
    settings: Data<Settings>,
    database: Data<Database>,
) -> impl Responder {
    let user_address = user_address.into_inner();

    if matches!(settings.env, Environment::Prod) {
        match AuthUser::extract(&request).await {
            Ok(AuthUser { address }) if address == user_address => {}
            _ => {
                return HttpResponse::Unauthorized().json(ResponseError::new("unauthorized"));
            }
        }
    }

    let Ok(images_count) = database.get_user_images_count(&user_address).await else {
        return HttpResponse::NotFound().json(ResponseError::new("user not found"));
    };

    let user_data = UserDataResponse {
        max_images: settings.max_images_per_user,
        current_images: images_count,
    };

    HttpResponse::Ok().json(user_data)
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetImagesResponse {
    pub images: Vec<Image>,
    #[serde(flatten)]
    pub user_data: UserDataResponse,
}

#[tracing::instrument]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    params(
        GetImagesQuery
    ),
    responses(
        (status = 200, description = "List images for a given user", body = GetImagesResponse),
        (status = 404, description = "Not found")
    )
)]
#[get("/users/{user_address}/images")]
async fn get_user_images(
    user_address: Path<String>,
    query_params: Query<GetImagesQuery>,
    request: HttpRequest,
    settings: Data<Settings>,
    database: Data<Database>,
) -> impl Responder {
    let user_address = user_address.into_inner();

    if matches!(settings.env, Environment::Prod) {
        match AuthUser::extract(&request).await {
            Ok(AuthUser { address }) if address == user_address => {}
            _ => {
                return HttpResponse::Unauthorized().json(ResponseError::new("unauthorized"));
            }
        }
    }

    let GetImagesQuery { offset, limit } = query_params.into_inner();

    let Ok(images_count) = database.get_user_images_count(&user_address).await else {
        return HttpResponse::NotFound().json(ResponseError::new("user not found"));
    };

    let Ok(images) = database.get_user_images(&user_address, offset as i64, limit as i64).await else {
        return HttpResponse::NotFound().json(ResponseError::new("user not found"));
    };

    let images = images.into_iter().map(Image::from).collect::<Vec<_>>();
    let user_data = UserDataResponse {
        current_images: images_count,
        max_images: settings.max_images_per_user,
    };
    HttpResponse::Ok().json(GetImagesResponse { images, user_data })
}
