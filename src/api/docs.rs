use super::delete::*;
use super::get::*;
use super::upload::*;
use super::*;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Camera Reel Service", description = "Camera Reel API"),
    paths(delete_image, get_image, get_metadata, get_user_images, upload_image),
    components(schemas(Image, Metadata, Scene, Location, User, Upload)),
    tags((name = "images", description = "Images management endpoints.")),
)]
pub struct ApiDoc;

pub fn generate_docs() -> SwaggerUi {
    SwaggerUi::new("/api/docs/ui/{_:.*}").url("/api/docs/openapi.json", ApiDoc::openapi())
}
