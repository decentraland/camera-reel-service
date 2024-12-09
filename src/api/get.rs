use actix_web::{
    get,
    web::{Data, Path, Redirect},
    FromRequest, HttpRequest, HttpResponse, Responder,
};
use actix_web_lab::extract::Query;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    api::{auth::AuthUser, GalleryImage, Image, ResponseError},
    database::Database,
    Environment, Settings,
};

#[tracing::instrument(skip(settings))]
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

#[tracing::instrument(skip(database))]
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
    #[serde(default = "default_compact")]
    compact: bool,
}

fn default_offset() -> u64 {
    0
}

fn default_limit() -> u64 {
    20
}

fn default_compact() -> bool {
    false
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDataResponse {
    pub current_images: u64,
    pub max_images: u64,
}

#[tracing::instrument(skip(database, settings))]
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
    let mut only_public_images: bool = false;

    match AuthUser::extract(&request).await {
        Ok(AuthUser { address }) if address == user_address => {}
        _ => {
            only_public_images = true;
        }
    }

    let Ok(images_count) = database
        .get_user_images_count(&user_address, only_public_images)
        .await
    else {
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

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetGalleryImagesResponse {
    pub images: Vec<GalleryImage>,
    #[serde(flatten)]
    pub user_data: UserDataResponse,
}

#[tracing::instrument(skip(database, settings))]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    params(
        GetImagesQuery
    ),
    responses(
        (status = 200, description = "List images for a given user", body = GetImagesResponse),
        (status = 210, description = "List gallery images for a given user if `compact=true` (status code is 200, but was not possible to list multiple responses for one status code)", body = GetGalleryImagesResponse),
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
    let mut only_public_images: bool = false;

    match AuthUser::extract(&request).await {
        Ok(AuthUser { address }) if address == user_address => {}
        _ => {
            only_public_images = true;
        }
    }

    let GetImagesQuery {
        offset,
        limit,
        compact,
    } = query_params.into_inner();

    let Ok(images_count) = database
        .get_user_images_count(&user_address, only_public_images)
        .await
    else {
        return HttpResponse::NotFound().json(ResponseError::new("user not found"));
    };

    let Ok(images) = database
        .get_user_images(
            &user_address,
            offset as i64,
            limit as i64,
            only_public_images,
        )
        .await
    else {
        return HttpResponse::NotFound().json(ResponseError::new("user not found"));
    };

    let user_data = UserDataResponse {
        current_images: images_count,
        max_images: settings.max_images_per_user,
    };

    if compact {
        let images = images
            .into_iter()
            .map(GalleryImage::from)
            .collect::<Vec<GalleryImage>>();
        return HttpResponse::Ok().json(GetGalleryImagesResponse { images, user_data });
    } else {
        let images = images.into_iter().map(Image::from).collect::<Vec<Image>>();
        return HttpResponse::Ok().json(GetImagesResponse { images, user_data });
    };
}

#[derive(Deserialize, Debug, IntoParams)]
struct GetPlaceImagesQuery {
    #[serde(default = "default_offset")]
    offset: u64,
    #[serde(default = "default_limit")]
    limit: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDataResponse {
    pub current_images: u64,
    pub max_images: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetPlaceImagesResponse {
    pub images: Vec<GalleryImage>,
    #[serde(flatten)]
    pub place_data: PlaceDataResponse,
}

#[tracing::instrument(skip(database, settings))]
#[utoipa::path(
    tag = "images",
    context_path = "/api", 
    params(
        GetPlaceImagesQuery
    ),
    responses(
        (status = 200, description = "List images for a given place", body = GetPlaceImagesResponse),
        (status = 404, description = "Not found")
    )
)]
#[get("/places/{place_id}/images")]
async fn get_place_images(
    place_id: Path<String>,
    query_params: Query<GetPlaceImagesQuery>,
    request: HttpRequest,
    settings: Data<Settings>,
    database: Data<Database>,
) -> impl Responder {
    let GetPlaceImagesQuery { offset, limit } = query_params.into_inner();

    let Ok(images_count) = database.get_place_images_count(&place_id).await else {
        return HttpResponse::NotFound().json(ResponseError::new("place not found"));
    };

    let Ok(images) = database
        .get_place_images(&place_id, offset as i64, limit as i64)
        .await
    else {
        return HttpResponse::NotFound().json(ResponseError::new("place not found"));
    };

    let place_data = PlaceDataResponse {
        current_images: images_count,
        max_images: settings.max_images_per_user,
    };

    let images = images
        .into_iter()
        .map(GalleryImage::from)
        .collect::<Vec<GalleryImage>>();

    return HttpResponse::Ok().json(GetPlaceImagesResponse { images, place_data });
}
