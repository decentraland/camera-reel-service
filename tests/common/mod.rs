use std::time::{SystemTime, UNIX_EPOCH};

use actix_test::TestServer;
use actix_web::{
    web::{scope, Data},
    App,
};
use actix_web_lab::__reexports::serde_json;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_sqs::{Client as SqsClient, Config as SqsConfig};
use camera_reel_service::{
    api::{self, upload::UploadResponse, Metadata, ResponseError},
    database::{Database, DatabaseOptions},
    live,
    sns::SNSPublisher,
    Environment, Settings,
};
use dcl_crypto::Identity;
use rand::{distributions::Alphanumeric, Rng};
use s3::{
    bucket_ops::CreateBucketResponse, creds::Credentials, Bucket, BucketConfiguration,
    Region as S3Region,
};
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
    let region = S3Region::Custom {
        region: "us-west-2".to_owned(),
        endpoint: "http://localhost:4566".to_owned(),
    };
    let credentials = Credentials::default().unwrap();
    let config = BucketConfiguration::default();

    let CreateBucketResponse { bucket, .. } =
        Bucket::create_with_path_style(bucket_name, region.clone(), credentials.clone(), config)
            .await
            .unwrap();

    bucket
}

async fn create_sqs_setup() -> (SqsClient, String) {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("us-west-2"))
        .endpoint_url("http://localhost:4566")
        .load()
        .await;

    let sqs_config = SqsConfig::builder()
        .credentials_provider(config.credentials_provider().unwrap().clone())
        .region(config.region().unwrap().clone())
        .endpoint_url("http://localhost:4566")
        .behavior_version(BehaviorVersion::latest())
        .build();

    let sqs_client = SqsClient::from_conf(sqs_config);

    // Create a unique queue name for this test
    let queue_name = format!("test-events-{}", create_string());

    let create_queue_result = sqs_client
        .create_queue()
        .queue_name(&queue_name)
        .send()
        .await
        .unwrap();

    let queue_url = create_queue_result.queue_url().unwrap().to_string();

    // Subscribe the queue to the SNS topic
    subscribe_queue_to_sns(&sqs_client, &queue_url).await;

    (sqs_client, queue_url)
}

async fn subscribe_queue_to_sns(sqs_client: &SqsClient, queue_url: &String) {
    use aws_sdk_sns::Client as SnsClient;

    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("us-west-2"))
        .endpoint_url("http://localhost:4566")
        .load()
        .await;

    let sns_config = aws_sdk_sns::Config::builder()
        .credentials_provider(config.credentials_provider().unwrap().clone())
        .region(config.region().unwrap().clone())
        .endpoint_url("http://localhost:4566")
        .behavior_version(BehaviorVersion::latest())
        .build();

    let sns_client = SnsClient::from_conf(sns_config);

    // Get queue attributes to get the ARN
    let queue_attrs = sqs_client
        .get_queue_attributes()
        .queue_url(queue_url)
        .attribute_names(aws_sdk_sqs::types::QueueAttributeName::QueueArn)
        .send()
        .await
        .unwrap();

    let queue_arn = queue_attrs
        .attributes()
        .unwrap()
        .get(&aws_sdk_sqs::types::QueueAttributeName::QueueArn)
        .unwrap();

    // Subscribe the queue to the SNS topic
    sns_client
        .subscribe()
        .topic_arn("arn:aws:sns:us-west-2:000000000000:events")
        .protocol("sqs")
        .endpoint(queue_arn)
        .send()
        .await
        .unwrap();

    // Set queue policy to allow SNS to send messages
    let policy = format!(
        r#"{{
            "Version": "2012-10-17",
            "Statement": [
                {{
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "sqs:SendMessage",
                    "Resource": "{}",
                    "Condition": {{
                        "ArnEquals": {{
                            "aws:SourceArn": "arn:aws:sns:us-west-2:000000000000:events"
                        }}
                    }}
                }}
            ]
        }}"#,
        queue_arn
    );

    sqs_client
        .set_queue_attributes()
        .queue_url(queue_url)
        .attributes(aws_sdk_sqs::types::QueueAttributeName::Policy, policy)
        .send()
        .await
        .unwrap();
}

fn create_settings(bucket_name: &str) -> Settings {
    Settings {
        port: 5000,
        bucket_url: format!("http://localhost:4566/{bucket_name}"),
        api_url: "http://localhost:5000".to_owned(),
        max_images_per_user: 1000,
        aws_sns_arn: "arn:aws:sns:us-west-2:000000000000:events".to_owned(),
        aws_sns_endpoint: "http://localhost:4566".to_owned(),
        env: Environment::Dev,
    }
}

pub struct TestContext {
    pub settings: Data<Settings>,
    pub database: Data<Database>,
    pub bucket: Data<Bucket>,
    pub sns_publisher: Data<SNSPublisher>,
    pub sqs_client: SqsClient,
    pub queue_url: String,
}

pub async fn create_context() -> TestContext {
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");

    let test_bucket = format!("camera-reel-{}", create_string());

    // Create SNS publisher for tests
    let sns_publisher = SNSPublisher::new(
        "arn:aws:sns:us-west-2:000000000000:events".to_string(),
        "http://localhost:4566".to_string(),
        "us-west-2".to_string(),
    )
    .await
    .unwrap();

    // Create SQS client and queue for testing SNS messages
    let (sqs_client, queue_url) = create_sqs_setup().await;

    TestContext {
        settings: Data::new(create_settings(&test_bucket)),
        database: Data::new(create_db(&test_bucket).await),
        bucket: Data::new(create_bucket(&test_bucket).await),
        sns_publisher: Data::new(sns_publisher),
        sqs_client,
        queue_url,
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

pub async fn create_test_server() -> (TestServer, TestContext) {
    initialize_tracing();
    let context = create_context().await;

    let server = actix_test::start({
        let context_clone = TestContext {
            settings: context.settings.clone(),
            database: context.database.clone(),
            bucket: context.bucket.clone(),
            sns_publisher: context.sns_publisher.clone(),
            sqs_client: context.sqs_client.clone(),
            queue_url: context.queue_url.clone(),
        };

        move || {
            App::new()
                .app_data(context_clone.settings.clone())
                .app_data(context_clone.bucket.clone())
                .app_data(context_clone.database.clone())
                .app_data(context_clone.sns_publisher.clone())
                .service(scope("/health").service(live))
                .configure(api::services)
        }
    });

    (server, context)
}

async fn upload_image(file_name: &str, address: &str, is_public: bool, place_id: &str) -> String {
    let identity = create_test_identity();
    // prepare image
    let image_bytes = include_bytes!("../resources/image.png").to_vec();
    let image_file_part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name(file_name.to_string())
        .mime_str("image/png")
        .unwrap();

    // prepare image metadata
    let metadata = Metadata {
        user_address: "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string(),
        place_id: place_id.to_string(),
        realm: "https://realm.org/v1".to_string(),
        ..Default::default()
    };
    let metadata_json = serde_json::to_vec(&metadata).unwrap();
    let metadata_part = reqwest::multipart::Part::bytes(metadata_json)
        .file_name("metadata.json")
        .mime_str("application/json")
        .unwrap();

    // fill form
    let form = reqwest::multipart::Form::new();
    let form = form
        .part("image", image_file_part)
        .part("metadata", metadata_part)
        .part(
            "is_public",
            reqwest::multipart::Part::text(is_public.to_string()),
        );

    let path = "/api/images";
    let headers = get_signed_headers(identity, "post", path, "");
    let response = reqwest::Client::new()
        .post(&format!("http://{address}{path}"))
        .multipart(form)
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let response: UploadResponse = response.json().await.unwrap();

    response.image.id
}

pub async fn upload_test_image(file_name: &str, address: &str, place_id: &str) -> String {
    upload_image(file_name, address, false, place_id).await
}

pub async fn upload_public_test_image(file_name: &str, address: &str, place_id: &str) -> String {
    upload_image(file_name, address, true, place_id).await
}

pub async fn upload_test_failing_image(file_name: &str, address: &str) -> String {
    let identity = create_test_identity();
    // prepare image
    let image_bytes = include_bytes!("../resources/image.png").to_vec();
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
    let metadata_part = reqwest::multipart::Part::bytes(metadata_json)
        .file_name("metadata.json")
        .mime_str("application/json")
        .unwrap();

    // fill form
    let form = reqwest::multipart::Form::new();
    let form = form
        .part("image", image_file_part)
        .part("metadata", metadata_part);

    let path = "/api/images";
    let headers = get_signed_headers(identity, "post", path, "");
    let response = reqwest::Client::new()
        .post(&format!("http://{address}{path}"))
        .multipart(form)
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    assert!(response.status().is_client_error());

    let response: ResponseError = response.json().await.unwrap();

    response.get_message().to_string()
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

pub fn get_place_id() -> String {
    "f888b899-c509-44d1-af21-717a4cef654e".to_string()
}

pub async fn poll_sqs_for_message_with_filter(
    sqs_client: &SqsClient,
    queue_url: &str,
    timeout_seconds: u64,
    filter_subtype: Option<&str>,
) -> Option<serde_json::Value> {
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    let start_time = Instant::now();
    let timeout_duration = Duration::from_secs(timeout_seconds);

    'outer: while start_time.elapsed() < timeout_duration {
        let receive_result = sqs_client
            .receive_message()
            .queue_url(queue_url)
            .max_number_of_messages(1)
            .wait_time_seconds(1)
            .send()
            .await;

        if let Ok(output) = receive_result {
            let messages = output.messages();
            if !messages.is_empty() {
                let message = &messages[0];
                if let Some(body) = message.body() {
                    // Parse the SNS message wrapper
                    if let Ok(sns_message) = serde_json::from_str::<serde_json::Value>(body) {
                        // Extract the actual message from SNS wrapper
                        if let Some(message_str) = sns_message.get("Message") {
                            if let Some(message_text) = message_str.as_str() {
                                if let Ok(actual_message) =
                                    serde_json::from_str::<serde_json::Value>(message_text)
                                {
                                    // Check if we need to filter by subtype
                                    if let Some(expected_subtype) = filter_subtype {
                                        if let Some(subtype) = actual_message.get("subType") {
                                            if subtype.as_str() != Some(expected_subtype) {
                                                // Delete this message and continue looking
                                                if let Some(receipt_handle) =
                                                    message.receipt_handle()
                                                {
                                                    let _ = sqs_client
                                                        .delete_message()
                                                        .queue_url(queue_url)
                                                        .receipt_handle(receipt_handle)
                                                        .send()
                                                        .await;
                                                }
                                                continue 'outer;
                                            }
                                        }
                                    }

                                    // Delete the message from the queue
                                    if let Some(receipt_handle) = message.receipt_handle() {
                                        let _ = sqs_client
                                            .delete_message()
                                            .queue_url(queue_url)
                                            .receipt_handle(receipt_handle)
                                            .send()
                                            .await;
                                    }
                                    return Some(actual_message);
                                }
                            }
                        }
                    }
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    None
}
