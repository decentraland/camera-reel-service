use std::fmt;
use std::time::Duration;

use moka::future::Cache;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct PlacesApiResponse {
    #[allow(dead_code)]
    ok: bool,
    total: usize,
    data: Vec<PlaceEntry>,
}

#[derive(Deserialize, Debug)]
struct PlaceEntry {
    id: String,
}

#[derive(Debug)]
pub enum PlacesClientError {
    RequestFailed(reqwest::Error),
    ApiError(u16),
    ParseError(reqwest::Error),
}

impl fmt::Display for PlacesClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacesClientError::RequestFailed(e) => write!(f, "request failed: {e}"),
            PlacesClientError::ApiError(status) => {
                write!(f, "places API returned status {status}")
            }
            PlacesClientError::ParseError(e) => write!(f, "failed to parse response: {e}"),
        }
    }
}

pub struct PlacesClient {
    client: reqwest::Client,
    base_url: String,
    cache: Cache<String, Vec<String>>,
}

impl PlacesClient {
    pub fn new(base_url: String, ttl_seconds: u64, max_size: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");

        let cache = Cache::builder()
            .max_capacity(max_size)
            .time_to_live(Duration::from_secs(ttl_seconds))
            .build();

        Self {
            client,
            base_url,
            cache,
        }
    }

    pub async fn get_world_place_ids(
        &self,
        world_name: &str,
    ) -> Result<Vec<String>, PlacesClientError> {
        if let Some(cached) = self.cache.get(world_name).await {
            return Ok(cached);
        }

        let mut all_ids = Vec::new();
        let mut offset: usize = 0;
        let limit: usize = 100;

        loop {
            let url = format!(
                "{}/api/places?names={}&limit={}&offset={}",
                self.base_url, world_name, limit, offset
            );

            let response = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(PlacesClientError::RequestFailed)?;

            let status = response.status().as_u16();
            if !response.status().is_success() {
                return Err(PlacesClientError::ApiError(status));
            }

            let body: PlacesApiResponse = response
                .json()
                .await
                .map_err(PlacesClientError::ParseError)?;

            for entry in &body.data {
                all_ids.push(entry.id.clone());
            }

            offset += body.data.len();
            if offset >= body.total || body.data.is_empty() {
                break;
            }
        }

        self.cache
            .insert(world_name.to_string(), all_ids.clone())
            .await;

        Ok(all_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn places_response(ok: bool, total: usize, ids: Vec<&str>) -> serde_json::Value {
        serde_json::json!({
            "ok": ok,
            "total": total,
            "data": ids.into_iter().map(|id| serde_json::json!({"id": id})).collect::<Vec<_>>()
        })
    }

    #[tokio::test]
    async fn test_single_page() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("names", "my-world.eth"))
            .and(query_param("offset", "0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 2, vec!["id-1", "id-2"])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);
        let ids = client.get_world_place_ids("my-world.eth").await.unwrap();

        assert_eq!(ids, vec!["id-1", "id-2"]);
    }

    #[tokio::test]
    async fn test_pagination() {
        let mock_server = MockServer::start().await;

        // Generate 100 IDs for page 1
        let page1_ids: Vec<String> = (0..100).map(|i| format!("id-{i}")).collect();
        let page1_refs: Vec<&str> = page1_ids.iter().map(|s| s.as_str()).collect();

        // Generate 100 IDs for page 2
        let page2_ids: Vec<String> = (100..200).map(|i| format!("id-{i}")).collect();
        let page2_refs: Vec<&str> = page2_ids.iter().map(|s| s.as_str()).collect();

        // Generate 50 IDs for page 3
        let page3_ids: Vec<String> = (200..250).map(|i| format!("id-{i}")).collect();
        let page3_refs: Vec<&str> = page3_ids.iter().map(|s| s.as_str()).collect();

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("names", "big-world.eth"))
            .and(query_param("offset", "0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 250, page1_refs)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("names", "big-world.eth"))
            .and(query_param("offset", "100"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 250, page2_refs)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("names", "big-world.eth"))
            .and(query_param("offset", "200"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 250, page3_refs)),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);
        let ids = client.get_world_place_ids("big-world.eth").await.unwrap();

        assert_eq!(ids.len(), 250);
        assert_eq!(ids[0], "id-0");
        assert_eq!(ids[249], "id-249");
    }

    #[tokio::test]
    async fn test_caching() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("names", "cached.eth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 1, vec!["cached-id"])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);

        let ids1 = client.get_world_place_ids("cached.eth").await.unwrap();
        let ids2 = client.get_world_place_ids("cached.eth").await.unwrap();

        assert_eq!(ids1, vec!["cached-id"]);
        assert_eq!(ids2, vec!["cached-id"]);
        // wiremock .expect(1) verifies only 1 request was made
    }

    #[tokio::test]
    async fn test_different_keys_not_shared() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("names", "world-a.eth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 1, vec!["id-a"])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("names", "world-b.eth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 1, vec!["id-b"])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);

        let ids_a = client.get_world_place_ids("world-a.eth").await.unwrap();
        let ids_b = client.get_world_place_ids("world-b.eth").await.unwrap();

        assert_eq!(ids_a, vec!["id-a"]);
        assert_eq!(ids_b, vec!["id-b"]);
    }

    #[tokio::test]
    async fn test_api_error_500() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);
        let err = client
            .get_world_place_ids("fail.eth")
            .await
            .unwrap_err();

        match err {
            PlacesClientError::ApiError(status) => assert_eq!(status, 500),
            other => panic!("expected ApiError(500), got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_api_error_404() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);
        let err = client
            .get_world_place_ids("missing.eth")
            .await
            .unwrap_err();

        match err {
            PlacesClientError::ApiError(status) => assert_eq!(status, 404),
            other => panic!("expected ApiError(404), got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_parse_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("not valid json{{{"),
            )
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);
        let err = client
            .get_world_place_ids("bad-json.eth")
            .await
            .unwrap_err();

        match err {
            PlacesClientError::ParseError(_) => {}
            other => panic!("expected ParseError, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_connection_refused() {
        // Use a URL that nothing is listening on
        let client = PlacesClient::new("http://127.0.0.1:1".to_string(), 300, 100);
        let err = client
            .get_world_place_ids("offline.eth")
            .await
            .unwrap_err();

        match err {
            PlacesClientError::RequestFailed(_) => {}
            other => panic!("expected RequestFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_empty_result() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 0, vec![])),
            )
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);
        let ids = client.get_world_place_ids("empty.eth").await.unwrap();

        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_error_mid_pagination() {
        let mock_server = MockServer::start().await;

        // Page 1 succeeds
        let page1_ids: Vec<String> = (0..100).map(|i| format!("id-{i}")).collect();
        let page1_refs: Vec<&str> = page1_ids.iter().map(|s| s.as_str()).collect();

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("offset", "0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 200, page1_refs)),
            )
            .mount(&mock_server)
            .await;

        // Page 2 fails
        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = PlacesClient::new(mock_server.uri(), 300, 100);
        let err = client
            .get_world_place_ids("mid-fail.eth")
            .await
            .unwrap_err();

        match err {
            PlacesClientError::ApiError(500) => {}
            other => panic!("expected ApiError(500), got: {other}"),
        }

        // Verify nothing was cached — a fresh call should hit the API again
        // (reset mock to succeed this time)
        mock_server.reset().await;

        Mock::given(method("GET"))
            .and(path("/api/places"))
            .and(query_param("offset", "0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(places_response(true, 1, vec!["recovered-id"])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let ids = client
            .get_world_place_ids("mid-fail.eth")
            .await
            .unwrap();
        assert_eq!(ids, vec!["recovered-id"]);
    }
}
