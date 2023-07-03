use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

pub struct Context {
    pub port: u16,
}

pub async fn run(context: Context) -> std::io::Result<()> {
    initialize_tracing();

    let port = context.port;
    let server = HttpServer::new(move || {
        let logger = TracingLogger::default();
        App::new().service(live).wrap(logger)
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
