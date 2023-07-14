use camera_reel_service::{database::Database, run, Context, Settings};
use clap::Parser;
use s3::{creds::Credentials, Bucket, Region};

const LOCAL_S3: &str = "http://localhost:9000";
#[derive(Parser, Debug)]
pub struct Arguments {
    #[clap(short, long, env, default_value = "3000")]
    port: u16,

    #[clap(long, env, default_value_t = String::from("http://localhost:3000"))]
    api_url: String,

    #[clap(long, short, env, default_value_t = String::from("postgres://postgres:postgres@localhost:5432/camera_reel"))]
    database_url: String,

    #[clap(long, env, default_value_t = String::from("us-east"))]
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
    let mut args = Arguments::parse();

    let database = Database::from_url(&args.database_url).await?;

    let region = if args.s3_url == LOCAL_S3 {
        std::env::set_var("AWS_ACCESS_KEY_ID", "minioadmin");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "minioadmin");

        let region = Region::Custom {
            region: args.aws_region,
            endpoint: args.s3_url.to_owned(),
        };
        args.s3_url = format!("{}/{}", args.s3_url, args.s3_bucket_name);
        region
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

    let s3_url = if !args.s3_url.ends_with('/') {
        args.s3_url.to_string()
    } else {
        let mut s3_url = args.s3_url.to_string();
        s3_url.remove(args.s3_url.len());
        s3_url
    };

    let settings = Settings {
        port: args.port,
        bucket_url: format!("{}", s3_url),
        api_url: args.api_url,
        authentication: args.enable_authentication,
    };

    let context = Context {
        settings,
        database,
        bucket,
    };

    Ok(run(context).await.map_err(|e| {
        tracing::debug!("app finished with error: {:?}", e);
        e
    })?)
}
