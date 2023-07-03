use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use database::Database;
use serde::{Deserialize, Serialize};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

mod api;
pub mod database;

pub struct Context {
    pub port: u16,
    pub database: Database,
}

pub async fn run(context: Context) -> std::io::Result<()> {
    initialize_tracing();

    let port = context.port;
    let server = HttpServer::new(move || {
        let logger = TracingLogger::default();
        App::new()
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
pub struct Image {
    pub id: String,
    pub url: String,
    pub metadata: Metadata,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Metadata {
    pub photographer: String,
    pub tags: Vec<String>,
    pub users: Vec<String>,
    pub wearables: Vec<String>,
    pub location: (i32, i32),
    pub timestamp: i64,
}
