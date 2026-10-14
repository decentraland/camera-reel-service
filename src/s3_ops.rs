use std::fmt;
use std::future::Future;
use std::time::Duration;

use s3::creds::Credentials;
use s3::error::S3Error;

// rust-s3 0.33 applied `request_timeout` as reqwest's full-request timeout; the
// 0.35 hyper backend only uses it for the TCP connect, so the bound is reapplied
// here. It does not cover the credential refresh, which blocks inside `poll`.
pub const S3_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

// Bounds the credential provider's own HTTP calls. Process-global in aws-creds,
// so unlike the provider choice it survives a refresh.
pub const CREDENTIALS_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub enum S3OpError {
    S3(S3Error),
    Timeout(Duration),
}

impl fmt::Display for S3OpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::S3(error) => write!(formatter, "{error}"),
            Self::Timeout(after) => write!(formatter, "timed out after {}s", after.as_secs()),
        }
    }
}

impl std::error::Error for S3OpError {}

pub async fn with_s3_timeout<T>(
    operation: impl Future<Output = Result<T, S3Error>>,
) -> Result<T, S3OpError> {
    match tokio::time::timeout(S3_REQUEST_TIMEOUT, operation).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(S3OpError::S3(error)),
        Err(_) => Err(S3OpError::Timeout(S3_REQUEST_TIMEOUT)),
    }
}

pub fn s3_credentials(
    access_key_id: Option<&str>,
    secret_access_key: Option<&str>,
) -> Result<Credentials, Box<dyn std::error::Error>> {
    match (access_key_id, secret_access_key) {
        (Some(id), Some(secret)) => Ok(Credentials::new(Some(id), Some(secret), None, None, None)?),
        (Some(_), None) | (None, Some(_)) => {
            Err("S3_ACCESS_KEY_ID and S3_SECRET_ACCESS_KEY must be set together".into())
        }
        // The full chain probes EC2 IMDSv2 before the ECS container endpoint, and
        // on Fargate that probe is a dead end taken while holding the write lock.
        (None, None) => {
            if std::env::var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI").is_ok() {
                Ok(Credentials::from_instance_metadata()?)
            } else {
                Ok(Credentials::default()?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_static_pair_when_both_are_set() {
        let credentials = s3_credentials(Some("AKIAEXAMPLE123"), Some("example-secret")).unwrap();

        assert_eq!(credentials.access_key.as_deref(), Some("AKIAEXAMPLE123"));
        assert_eq!(credentials.secret_key.as_deref(), Some("example-secret"));
    }

    #[test]
    fn rejects_a_half_set_pair() {
        for (id, secret) in [(Some("AKIAEXAMPLE123"), None), (None, Some("example-secret"))] {
            let error = s3_credentials(id, secret).unwrap_err().to_string();

            assert!(error.contains("must be set together"), "got: {error}");
        }
    }

    #[tokio::test]
    async fn reports_a_timeout_rather_than_waiting_forever() {
        let never = async {
            std::future::pending::<()>().await;
            Ok::<(), S3Error>(())
        };

        let error = tokio::time::timeout(Duration::from_millis(50), with_s3_timeout(never)).await;

        assert!(error.is_err(), "the helper should outlive a 50ms probe");
    }

    #[tokio::test]
    async fn passes_through_a_successful_call() {
        assert_eq!(with_s3_timeout(async { Ok::<_, S3Error>(7) }).await.unwrap(), 7);
    }

    #[tokio::test]
    async fn preserves_the_underlying_s3_error() {
        let failing = async { Err::<(), _>(S3Error::HttpFailWithBody(503, "slow down".into())) };

        let error = with_s3_timeout(failing).await.unwrap_err();

        assert!(matches!(error, S3OpError::S3(_)), "got: {error}");
    }
}
