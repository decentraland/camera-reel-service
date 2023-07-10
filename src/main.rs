use camera_reel_service::{database::Database, run, Context, Settings};
use clap::Parser;
use s3::{creds::Credentials, Bucket, Region};

const LOCAL_S3: &str = "http://localhost:9000";
#[derive(Parser, Debug)]
pub struct Arguments {
    #[clap(short, long, env, default_value = "5000")]
    port: u16,

    #[clap(long, short, env, default_value_t = String::from("http://localhost:5000"))]
    api_url: String,

    #[clap(long, short, env, default_value_t = String::from("postgres://postgres:postgres@localhost:5432/camera_reel"))]
    database_url: String,

    #[clap(long, short, env, default_value_t = String::from("us-east"))]
    aws_region: String,

    #[clap(long, short, env, default_value_t = String::from(LOCAL_S3))]
    s3_url: String,

    #[clap(long, env, default_value_t = String::from("camera-reel"))]
    s3_bucket_name: String,

    #[clap(long, env, default_value = "false")]
    enable_authentication: bool,
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();

    let database = Database::from_url(&args.database_url).await?;

    let region = if args.s3_url == LOCAL_S3 {
        Region::Custom {
            region: args.aws_region,
            endpoint: args.s3_url.to_owned(),
        }
    } else {
        args.aws_region.parse()?
    };

    let bucket = Bucket::new(
        &args.s3_bucket_name,
        region,
        // Loads credentials from ENV variables
        Credentials::default()?,
    )?
    .with_path_style();

    let settings = Settings {
        port: args.port,
        bucket_url: format!("{}/{}", args.s3_url, args.s3_bucket_name),
        api_url: args.api_url,
        authentication: args.enable_authentication,
    };

    let context = Context {
        settings,
        database,
        bucket,
    };

    Ok(run(context).await?)
}
