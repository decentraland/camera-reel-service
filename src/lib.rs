use actix_web::{get, web::Data, App, HttpResponse, HttpServer, Responder};
use database::Database;
use s3::Bucket;
use tracing::subscriber::set_global_default;
use tracing_actix_web::TracingLogger;
use tracing_log::LogTracer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
use tracing_tree::HierarchicalLayer;

use crate::api::middlewares;

pub mod api;
pub mod database;

#[derive(Debug)]
pub enum Environment {
    Dev,
    Prod,
}

#[derive(Debug)]
pub struct Settings {
    pub port: u16,
    pub api_url: String,
    pub bucket_url: String,
    pub max_images_per_user: u64,
    pub env: Environment,
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
    let metrics_token = std::env::var("WKC_METRICS_BEARER_TOKEN").unwrap_or("".to_string());

    let server = HttpServer::new(move || {
        let logger = TracingLogger::default();
        App::new()
            .app_data(settings.clone())
            .app_data(bucket.clone())
            .app_data(database.clone())
            .service(live)
            .configure(api::services)
            .wrap(logger)
            .wrap(middlewares::metrics())
            .wrap(middlewares::metrics_token(&metrics_token))
    })
    .bind(("0.0.0.0", port))?;

    tracing::debug!("listening on port: {port}");

    server.run().await
}

fn initialize_tracing() {
    // Redirect all `log`'s events to our subscriber
    LogTracer::init().expect("Failed to set logger");

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("camera-reel-service=debug"));
    let formatting_layer = HierarchicalLayer::new(2);
    let subscriber = Registry::default().with(env_filter).with(formatting_layer);

    set_global_default(subscriber).expect("Failed to set subscriber");
}

#[tracing::instrument]
#[get("/health/live")]
async fn live() -> impl Responder {
    HttpResponse::Ok().json("alive")
}
