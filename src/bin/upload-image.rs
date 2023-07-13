use actix_web_lab::__reexports::serde_json;
use camera_reel_service::Metadata;
use ipfs_hasher::IpfsHasher;

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
    let ipfs_hasher = IpfsHasher::default();
    let identity = create_test_identity();

    // prepare image
    let image_bytes = include_bytes!("../../tests/resources/scene-thumbnail.png").to_vec();
    let image_hash = ipfs_hasher.compute(&image_bytes);
    let image_file_part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name("image.png")
        .mime_str("image/png")
        .unwrap();

    // prepare image metadata
    let metadata = Metadata {
        user_address: "0x7949f9f239d1a0816ce5eb364a1f588ae9cc1bf5".to_string(),
        ..Default::default()
    };
    let metadata_json = serde_json::to_vec(&metadata).unwrap();
    let metadata_hash = ipfs_hasher.compute(&metadata_json);
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

    let address = "https://camera-reel-service.decentraland.zone";
    let response = reqwest::Client::new()
        .post(&format!("{}/api/images", address))
        .multipart(form)
        .send()
        .await
        .unwrap();

    println!("image upload response: {}", response.text().await.unwrap())
}
