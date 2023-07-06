use actix_web::{get, web::Data, App, HttpResponse, HttpServer, Responder};
use database::Database;
use s3::Bucket;
use serde::{Deserialize, Serialize};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

mod api;
pub mod database;

#[derive(Debug)]
pub struct Settings {
    pub port: u16,
    pub api_url: String,
    pub bucket_url: String,
}

pub struct Context {
    pub settings: Settings,
    pub database: Database,
    pub bucket: Bucket,
}

pub async fn run(context: Context) -> std::io::Result<()> {
    initialize_tracing();

    let port = context.settings.port;

    let settings = Data::new(context.settings);
    let bucket = Data::new(context.bucket);
    let database = Data::new(context.database);

    let server = HttpServer::new(move || {
        let logger = TracingLogger::default();
        App::new()
            .app_data(settings.clone())
            .app_data(bucket.clone())
            .app_data(database.clone())
            .service(live)
            .configure(api::services)
            .wrap(logger)
    })
    .bind(("127.0.0.1", port))?;

    tracing::debug!("listening on port: {port}");

    server.run().await
}

fn initialize_tracing() {
    let directives =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "camera-reel-service=debug".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(directives))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tracing::instrument]
#[get("/health/live")]
async fn live() -> impl Responder {
    HttpResponse::Ok().json("alive")
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub id: String,
    pub url: String,
    pub metadata: Metadata,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub user_name: String,
    pub user_address: String,
    pub date_time: i64,
    pub realm: String,
    pub scene: Scene,
    pub visible_people: Vec<User>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub name: String,
    pub location: Location,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub x: i32,
    pub y: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub user_name: String,
    pub user_address: String,
    pub wearables: Vec<String>,
}
