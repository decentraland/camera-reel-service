use aws_config::{BehaviorVersion, Region};
use aws_sdk_sns as sns;
use aws_sdk_sns::{Client, Config};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Camera,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::Camera => write!(f, "camera"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSubtype {
    PhotoTaken,
    PhotoPrivacyChanged,
}

impl std::fmt::Display for EventSubtype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSubtype::PhotoTaken => write!(f, "photo-taken"),
            EventSubtype::PhotoPrivacyChanged => write!(f, "photo-privacy-changed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub event_type: EventType,
    #[serde(rename = "subType")]
    pub sub_type: EventSubtype,
    pub key: String,
    pub timestamp: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

pub struct SNSPublisher {
    client: Client,
    topic_arn: String,
}

impl SNSPublisher {
    pub async fn new(
        topic_arn: String,
        endpoint: Option<String>,
        region: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Load AWS config with specified region
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(region))
            .load()
            .await;

        let mut sns_config_builder = Config::builder()
            .credentials_provider(config.credentials_provider().unwrap().clone())
            .region(config.region().unwrap().clone())
            .behavior_version(BehaviorVersion::latest());

        if let Some(endpoint_url) = endpoint {
            sns_config_builder = sns_config_builder.endpoint_url(endpoint_url);
        }

        let sns_config = sns_config_builder.build();

        let client = Client::from_conf(sns_config);

        Ok(SNSPublisher { client, topic_arn })
    }

    pub async fn publish(
        &self,
        event: &Event,
    ) -> Result<sns::operation::publish::PublishOutput, Box<dyn std::error::Error>> {
        let message_json = serde_json::to_string(event)?;

        let response = self
            .client
            .publish()
            .topic_arn(&self.topic_arn)
            .message(&message_json)
            .message_attributes(
                "type",
                sns::types::MessageAttributeValue::builder()
                    .data_type("String")
                    .string_value(event.event_type.to_string())
                    .build()?,
            )
            .message_attributes(
                "subType",
                sns::types::MessageAttributeValue::builder()
                    .data_type("String")
                    .string_value(event.sub_type.to_string())
                    .build()?,
            )
            .send()
            .await?;

        Ok(response)
    }
}
