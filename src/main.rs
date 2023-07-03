use camera_reel_service::{database::Database, run, Context};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Arguments {
    #[clap(short, long, env, default_value = "5000")]
    port: u16,

    #[clap(long, short, env, default_value_t = String::from("postgres://postgres:postgres@localhost:5432/camera_reel"))]
    database_url: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Arguments::parse();

    let Ok(database) = Database::from_url(&args.database_url).await else {
        panic!("Unable to connect to database");
    };

    let context = Context {
        port: args.port,
        database,
    };

    run(context).await
}
