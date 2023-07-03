use camera_reel_service::{run, Context};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Arguments {
    #[clap(short, long, default_value = "5000")]
    pub port: u16,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Arguments::parse();

    let context = Context { port: args.port };

    run(context).await
}
