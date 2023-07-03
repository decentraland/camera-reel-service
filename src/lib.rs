use actix_web::{get, App, HttpResponse, HttpServer, Responder};

pub struct Context {
    pub port: u16,
}

pub async fn run(context: Context) -> std::io::Result<()> {
    let server =
        HttpServer::new(move || App::new().service(live)).bind(("127.0.0.1", context.port))?;

    server.run().await
}

#[get("/health/live")]
async fn live() -> impl Responder {
    HttpResponse::Ok().json("alive")
}
