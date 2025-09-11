use camera_reel_service::sns::SNSPublisher;
use camera_reel_service::{database::Database, run, Context, Environment, Settings};
use clap::Parser;
use s3::{creds::Credentials, Bucket, Region};

const LOCAL_S3: &str = "http://localhost:4566";
#[derive(Parser, Debug)]
pub struct Arguments {
    #[clap(short, long, env, default_value = "3000")]
    port: u16,

    #[clap(long, env, default_value_t = String::from("http://localhost:3000"))]
    api_url: String,

    #[clap(long, short, env, default_value_t = String::from("postgres://postgres:postgres@localhost:5432/camera_reel"))]
    database_url: String,

    #[clap(long, env, default_value_t = String::from("us-east-1"))]
    aws_region: String,

    #[clap(long, short, env, default_value_t = String::from(LOCAL_S3))]
    s3_url: String,

    #[clap(long, env, default_value_t = String::from("camera-reel"))]
    s3_bucket_name: String,

    #[clap(long, env, default_value_t = 500)]
    max_images_per_user: u64,

    #[clap(long, env, default_value_t = String::from("arn:aws:sns:us-east-1:000000000000:events"))]
    aws_sns_arn: String,

    #[clap(long, env)]
    aws_sns_endpoint: Option<String>,
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Arguments::parse();

    let database = Database::from_url(&args.database_url).await?;

    let aws_region = args.aws_region.clone();
    let region = if args.s3_url == LOCAL_S3 {
        std::env::set_var("AWS_ACCESS_KEY_ID", "test");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");

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
        s3_url.remove(args.s3_url.len() - 1);
        s3_url
    };

    let settings = Settings {
        port: args.port,
        bucket_url: s3_url.to_string(),
        api_url: args.api_url,
        max_images_per_user: args.max_images_per_user,
        env: read_env(),
        aws_sns_arn: args.aws_sns_arn.clone(),
        aws_sns_endpoint: args.aws_sns_endpoint.clone(),
    };

    // Create SNS Publisher
    let sns_publisher =
        SNSPublisher::new(args.aws_sns_arn, args.aws_sns_endpoint, aws_region).await?;

    let context = Context {
        settings,
        database,
        bucket,
        sns_publisher,
    };

    Ok(run(context).await.map_err(|e| {
        tracing::debug!("app finished with error: {:?}", e);
        e
    })?)
}

fn read_env() -> Environment {
    match std::env::var("ENV") {
        Ok(env) if env == "prd" => Environment::Prod,
        _ => Environment::Dev,
    }
}
