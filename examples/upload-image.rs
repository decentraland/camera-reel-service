use std::time::{SystemTime, UNIX_EPOCH};

use actix_web_lab::__reexports::serde_json;
use camera_reel_service::api::{upload::UploadResponse, Metadata};
use dcl_crypto::Identity;

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

#[actix_web::main]
async fn main() {
    let identity = create_test_identity();

    // prepare image
    let image_bytes = include_bytes!("../tests/resources/fall-autumn-red-season.jpg").to_vec();
    let image_file_part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name("fall-autumn-red-season.jpg")
        .mime_str("image/jpeg")
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

    let address = "https://camera-reel-service.decentraland.zone";
    // let address = "http://127.0.0.1:3000";

    let path = "/api/images";
    let headers = get_signed_headers(identity, "post", path, "");
    let response = reqwest::Client::new()
        .post(&format!("{address}{path}"))
        .multipart(form)
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    let response: UploadResponse = response.json().await.unwrap();
    println!("image upload response: {:?}", response);
    let image_id = response.image.id;

    let identity = create_test_identity();
    let path = "/api/users/0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5/images";
    let headers = get_signed_headers(identity, "get", path, "");
    let response = reqwest::Client::new()
        .get(&format!("{address}{path}"))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    println!("get images response: {}", response.text().await.unwrap());

    let identity = create_test_identity();
    let path = format!("/api/images/{image_id}");
    let headers = get_signed_headers(identity, "delete", &path, "");
    let response = reqwest::Client::new()
        .delete(&format!("{address}{path}"))
        .header(headers[0].0.clone(), headers[0].1.clone())
        .header(headers[1].0.clone(), headers[1].1.clone())
        .header(headers[2].0.clone(), headers[2].1.clone())
        .header(headers[3].0.clone(), headers[3].1.clone())
        .header(headers[4].0.clone(), headers[4].1.clone())
        .send()
        .await
        .unwrap();

    println!("delete image response: {}", response.text().await.unwrap());
}

fn get_signed_headers(
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
