use std::time::{SystemTime, UNIX_EPOCH};

use actix_test::TestServer;
use actix_web::{web::Data, App};
use actix_web_lab::__reexports::serde_json;
use camera_reel_service::{
    api::{self, upload::UploadResponse, Metadata},
    database::{Database, DatabaseOptions},
    live, Settings,
};
use dcl_crypto::Identity;
use rand::{distributions::Alphanumeric, Rng};
use s3::{
    bucket_ops::CreateBucketResponse, creds::Credentials, Bucket, BucketConfiguration, Region,
};
use sha256::digest;
use sqlx::Executor;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

pub fn create_test_identity() -> dcl_crypto::Identity {
    dcl_crypto::Identity::from_json(
      r#"{
     "ephemeralIdentity": {
       "address": "0x84452bbFA4ca14B7828e2F3BBd106A2bD495CD34",
       "publicKey": "0x0420c548d960b06dac035d1daf826472eded46b8b9d123294f1199c56fa235c89f2515158b1e3be0874bfb15b42d1551db8c276787a654d0b8d7b4d4356e70fe42",
       "privateKey": "0xbc453a92d9baeb3d10294cbc1d48ef6738f718fd31b4eb8085efe7b311299399"
     },
     "expiration": "3021-10-16T22:32:29.626Z",
     "authChain": [
       {
         "type": "SIGNER",
         "payload": "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5",
         "signature": ""
       },
       {
         "type": "ECDSA_EPHEMERAL",
         "payload": "Decentraland Login\nEphemeral address: 0x84452bbFA4ca14B7828e2F3BBd106A2bD495CD34\nExpiration: 3021-10-16T22:32:29.626Z",
         "signature": "0x39dd4ddf131ad2435d56c81c994c4417daef5cf5998258027ef8a1401470876a1365a6b79810dc0c4a2e9352befb63a9e4701d67b38007d83ffc4cd2b7a38ad51b"
       }
     ]
    }"#,
  ).unwrap()
}

fn create_string() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect::<String>()
        .to_lowercase()
}

async fn create_db(test_database: &str) -> Database {
    let db_url = "postgres://postgres:postgres@localhost:5432";
    let db_opts = DatabaseOptions::new(db_url);
    let db = db_opts.connect().await.unwrap();
    let connection = db.get_connection().await.unwrap();
    connection
        .detach()
        .execute(format!(r#"CREATE DATABASE "{}";"#, test_database).as_str())
        .await
        .expect("Failed to create DB");

    let url = format!("{db_url}/{test_database}");

    Database::from_url(&url).await.unwrap()
}

async fn create_bucket(bucket_name: &str) -> Bucket {
    let region = Region::Custom {
        region: "us-east-1".to_owned(),
        endpoint: "http://localhost:9000".to_owned(),
    };
    let credentials = Credentials::default().unwrap();
    let config = BucketConfiguration::public();

    let CreateBucketResponse { bucket, .. } =
        Bucket::create_with_path_style(bucket_name, region.clone(), credentials.clone(), config)
            .await
            .unwrap();

    bucket
}

fn create_settings(bucket_name: &str) -> Settings {
    Settings {
        port: 5000,
        bucket_url: format!("http://127.0.0.1:9000/{bucket_name}"),
        api_url: "http://localhost:5000".to_owned(),
        authentication: true,
        max_images_per_user: 1000,
    }
}

pub struct TestContext {
    pub settings: Data<Settings>,
    pub database: Data<Database>,
    pub bucket: Data<Bucket>,
}

pub async fn create_context() -> TestContext {
    std::env::set_var("AWS_ACCESS_KEY_ID", "minioadmin");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "minioadmin");

    let test_bucket = format!("camera-reel-{}", create_string());
    TestContext {
        settings: Data::new(create_settings(&test_bucket)),
        database: Data::new(create_db(&test_bucket).await),
        bucket: Data::new(create_bucket(&test_bucket).await),
    }
}

fn initialize_tracing() {
    let directives =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "camera-reel-service=debug".into());
    _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(directives))
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

pub async fn create_test_server() -> TestServer {
    initialize_tracing();
    let context = create_context().await;

    actix_test::start(move || {
        App::new()
            .app_data(context.settings.clone())
            .app_data(context.bucket.clone())
            .app_data(context.database.clone())
            .service(live)
            .configure(api::services)
    })
}

pub async fn upload_test_image(file_name: &str, address: &str) -> String {
    let identity = create_test_identity();
    // prepare image
    let image_bytes = include_bytes!("../resources/image.png").to_vec();
    let image_hash = digest(&image_bytes);
    let image_file_part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name(file_name.to_string())
        .mime_str("image/png")
        .unwrap();

    // prepare image metadata
    let metadata = Metadata {
        user_address: "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string(),
        ..Default::default()
    };
    let metadata_json = serde_json::to_vec(&metadata).unwrap();
    let metadata_hash = digest(&metadata_json);
    let metadata_part = reqwest::multipart::Part::bytes(metadata_json)
        .file_name("metadata.json")
        .mime_str("application/json")
        .unwrap();

    // prepare authchain
    let payload = format!("{image_hash}-{metadata_hash}");
    let authchain = identity.sign_payload(payload);
    let auth_chain_json = serde_json::to_vec(&authchain).unwrap();
    let authchain_part = reqwest::multipart::Part::bytes(auth_chain_json)
        .mime_str("application/json")
        .unwrap();

    // fill form
    let form = reqwest::multipart::Form::new();
    let form = form
        .part("image", image_file_part)
        .part("metadata", metadata_part)
        .part("authchain", authchain_part);

    let response = reqwest::Client::new()
        .post(&format!("http://{}/api/images", address))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let response: UploadResponse = response.json().await.unwrap();

    response.image.id
}

pub fn get_signed_headers(
    identity: Identity,
    method: &str,
    path: &str,
    metadata: &str,
) -> Vec<(String, String)> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let payload = [method, path, &ts.to_string(), metadata]
        .join(":")
        .to_lowercase();

    let authchain = identity.sign_payload(payload);

    vec![
        (
            "X-Identity-Auth-Chain-0".to_string(),
            serde_json::to_string(authchain.get(0).unwrap()).unwrap(),
        ),
        (
            "X-Identity-Auth-Chain-1".to_string(),
            serde_json::to_string(authchain.get(1).unwrap()).unwrap(),
        ),
        (
            "X-Identity-Auth-Chain-2".to_string(),
            serde_json::to_string(authchain.get(2).unwrap()).unwrap(),
        ),
        ("X-Identity-Timestamp".to_string(), ts.to_string()),
        ("X-Identity-Metadata".to_string(), metadata.to_string()),
    ]
}
