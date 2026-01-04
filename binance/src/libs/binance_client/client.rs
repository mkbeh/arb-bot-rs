use std::time::Duration;

use anyhow::{anyhow, bail};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::libs::binance_client::{api::Api, utils::generate_signature};

/// Primary client for interacting with the Binance API.
///
/// Supports authenticated and public requests via GET/POST methods.
/// Handles URL construction, signature generation, headers, and response parsing.
/// Uses a configurable `reqwest::Client` for HTTP transport.
#[derive(Clone)]
pub struct Client {
    /// Base host URL for API requests (e.g., "https://api.binance.com").
    host: String,
    /// API key for authentication.
    api_key: String,
    /// Secret key for HMAC signature generation.
    secret_key: String,
    /// Inner HTTP client with configured timeouts and connection pooling.
    inner_client: reqwest::Client,
}

impl Client {
    /// Creates a new `Client` from configuration.
    ///
    /// Builds the inner `reqwest::Client` with HTTP settings from `ClientConfig`.
    ///
    /// # Arguments
    /// * `cfg` - Configuration including API credentials and HTTP params.
    ///
    /// # Errors
    /// Returns an error if the inner client builder fails (e.g., invalid timeouts).
    pub fn from_config(cfg: ClientConfig) -> anyhow::Result<Self, anyhow::Error> {
        let client = Self {
            host: cfg.api_url.clone(),
            api_key: cfg.api_token.clone(),
            secret_key: cfg.api_secret_key.clone(),
            inner_client: reqwest::Client::builder()
                .connect_timeout(cfg.http_config.connect_timeout)
                .pool_idle_timeout(cfg.http_config.pool_idle_timeout)
                .pool_max_idle_per_host(cfg.http_config.pool_max_idle_per_host)
                .tcp_keepalive(cfg.http_config.tcp_keepalive)
                .tcp_keepalive_interval(cfg.http_config.tcp_keepalive_interval)
                .tcp_keepalive_retries(cfg.http_config.tcp_keepalive_retries)
                .timeout(cfg.http_config.timeout)
                .build()?,
        };

        Ok(client)
    }

    /// Performs a GET request to the Binance API.
    ///
    /// Constructs the URL with optional query params and signature if required.
    /// Deserializes the JSON response into the target type.
    ///
    /// # Type Parameters
    /// * `T` - Deserializable response type (implements `serde::de::DeserializeOwned`).
    ///
    /// # Arguments
    /// * `path` - API endpoint (from `binance_api::api::Api`).
    /// * `query` - Optional query parameters as `Vec<(String, String)>`.
    /// * `with_signature` - Whether to include HMAC signature (for private endpoints).
    ///
    /// # Errors
    /// Returns an error for HTTP failures, invalid responses, or deserialization issues.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: Api,
        query: Option<&Vec<(String, String)>>,
        with_signature: bool,
    ) -> anyhow::Result<T> {
        let url = self.build_url(path, query, with_signature)?;
        let request = if with_signature {
            self.inner_client
                .get(url)
                .headers(self.build_headers()?)
                .build()?
        } else {
            self.inner_client.get(url).build()?
        };

        let response = self.inner_client.execute(request).await?;
        response_handler(response).await
    }

    /// Performs a POST request to the Binance API.
    ///
    /// Constructs the URL with optional query params and signature if required.
    /// Deserializes the JSON response into the target type.
    ///
    /// # Type Parameters
    /// * `T` - Deserializable response type (implements `serde::de::DeserializeOwned`).
    ///
    /// # Arguments
    /// * `path` - API endpoint (from `binance_api::api::Api`).
    /// * `query` - Optional query parameters as `Vec<(String, String)>` (for POST body/query).
    /// * `with_signature` - Whether to include HMAC signature (for private endpoints).
    ///
    /// # Errors
    /// Returns an error for HTTP failures, invalid responses, or deserialization issues.
    pub async fn post<T: DeserializeOwned>(
        &self,
        path: Api,
        query: Option<&Vec<(String, String)>>,
        with_signature: bool,
    ) -> anyhow::Result<T> {
        let url = self.build_url(path, query, with_signature)?;
        let request = if with_signature {
            self.inner_client
                .post(url)
                .headers(self.build_headers()?)
                .build()?
        } else {
            self.inner_client.post(url).build()?
        };

        let response = self.inner_client.execute(request).await?;
        response_handler(response).await
    }

    /// Builds the full API URL with query params and optional signature.
    ///
    /// Appends the path to the host, adds query string, and generates signature if needed.
    ///
    /// # Arguments
    /// * `path` - API endpoint path.
    /// * `query` - Optional query parameters.
    /// * `with_signature` - Whether to append signature.
    ///
    /// # Errors
    /// Returns an error if URL construction fails (unlikely).
    fn build_url(
        &self,
        path: Api,
        query: Option<&Vec<(String, String)>>,
        with_signature: bool,
    ) -> anyhow::Result<String> {
        let mut url = format!("{}{}", self.host, String::from(path));
        let mut query_params = String::new();

        if let Some(v) = query {
            query_params.push_str(build_query(v).as_str());
        }

        if with_signature {
            url.push_str(format!("?{}", self.build_signature(query_params)).as_str());
        } else {
            url.push_str(format!("?{query_params}").as_str());
        }

        Ok(url)
    }

    /// Generates the HMAC signature query string for authenticated requests.
    ///
    /// Uses `generate_signature` from Binance utils; appends to existing query or starts new.
    ///
    /// # Arguments
    /// * `query_params` - Existing query string (without leading '?').
    fn build_signature(&self, query_params: String) -> String {
        let signature = if query_params.is_empty() {
            generate_signature(&self.secret_key, None)
        } else {
            generate_signature(&self.secret_key, Some(&query_params))
        };

        if query_params.is_empty() {
            format!("?signature={signature}")
        } else {
            format!("{query_params}&signature={signature}")
        }
    }

    /// Builds authentication headers for signed requests.
    fn build_headers(&self) -> anyhow::Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-mbx-apikey"),
            HeaderValue::from_str(self.api_key.as_str())?,
        );
        Ok(headers)
    }
}

/// Handles HTTP responses from Binance API.
async fn response_handler<T: DeserializeOwned>(resp: Response) -> anyhow::Result<T> {
    match resp.status() {
        StatusCode::OK => {
            let body = resp.bytes().await?;
            Ok(serde_json::from_slice::<T>(&body)?)
        }
        StatusCode::INTERNAL_SERVER_ERROR => bail!("Internal Server Error"),
        StatusCode::SERVICE_UNAVAILABLE => bail!("Service Unavailable"),
        StatusCode::UNAUTHORIZED => bail!("Unauthorized"),
        code => {
            bail!(format!(
                "Received error: code={} msg={}",
                code,
                resp.text().await.map_err(|e| anyhow!(e))?
            ));
        }
    }
}

/// Builds a query string from key-value pairs.
fn build_query(params: &Vec<(String, String)>) -> String {
    let mut query = String::new();
    for (k, v) in params {
        query.push_str(&format!("{k}={v}&"));
    }
    query.pop();
    query
}

/// Configuration for the Binance API client.
///
/// Includes credentials and HTTP transport settings.
#[derive(Default, Clone)]
pub struct ClientConfig {
    /// Base API URL.
    pub api_url: String,
    /// API key.
    pub api_token: String,
    /// API secret key.
    pub api_secret_key: String,
    /// HTTP client configuration.
    pub http_config: HttpConfig,
}

/// HTTP configuration for the inner `reqwest::Client`.
///
/// Controls timeouts, pooling, and TCP keepalive.
#[derive(Clone)]
pub struct HttpConfig {
    pub connect_timeout: Duration,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    pub tcp_keepalive: Duration,
    pub tcp_keepalive_interval: Duration,
    pub tcp_keepalive_retries: u32,
    pub timeout: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            pool_idle_timeout: Duration::from_secs(120),
            pool_max_idle_per_host: 5,
            tcp_keepalive: Duration::from_secs(120),
            tcp_keepalive_interval: Duration::from_secs(30),
            tcp_keepalive_retries: 5,
            timeout: Duration::from_secs(10),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mockito::Server;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::libs::binance_client::api::Spot;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestResponse {
        symbol: String,
        price: String,
    }

    fn create_test_client(server_url: &str) -> Client {
        let config = ClientConfig {
            api_url: server_url.to_string(),
            api_token: "test_api_key".to_string(),
            api_secret_key: "test_secret_key".to_string(),
            http_config: HttpConfig::default(),
        };

        Client::from_config(config).unwrap()
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = ClientConfig {
            api_url: "https://api.binance.com".to_string(),
            api_token: "test_key".to_string(),
            api_secret_key: "test_secret".to_string(),
            http_config: HttpConfig::default(),
        };

        let client = Client::from_config(config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_get_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::AnyOf(vec![
                    mockito::Matcher::Exact("/api/v3/ticker/price?".to_string()),
                    mockito::Matcher::Regex(r"^/api/v3/ticker/price\?".to_string()),
                ]),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"symbol": "BTCUSDT", "price": "50000.0"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let result: anyhow::Result<TestResponse> =
            client.get(Api::Spot(Spot::Price), None, false).await;

        mock.assert();
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.symbol, "BTCUSDT");
        assert_eq!(response.price, "50000.0");
    }

    #[tokio::test]
    async fn test_get_with_query_params() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(
                    r"^/api/v3/ticker/price\?.*symbol=BTCUSDT.*interval=1h".to_string(),
                ),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"symbol": "BTCUSDT", "price": "50000.0"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let query_params = vec![
            ("symbol".to_string(), "BTCUSDT".to_string()),
            ("interval".to_string(), "1h".to_string()),
        ];

        let result: anyhow::Result<TestResponse> = client
            .get(Api::Spot(Spot::Price), Some(&query_params), false)
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_with_signature() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/api/v3/account\?.*signature=[^&]+".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"symbol": "BTCUSDT", "price": "50000.0"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let result: anyhow::Result<TestResponse> =
            client.get(Api::Spot(Spot::Account), None, true).await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_with_query_and_signature() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(
                    r"^/api/v3/order\?.*symbol=BTCUSDT.*side=BUY.*signature=[^&]+".to_string(),
                ),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"symbol": "BTCUSDT", "price": "50000.0"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let query_params = vec![
            ("symbol".to_string(), "BTCUSDT".to_string()),
            ("side".to_string(), "BUY".to_string()),
        ];

        let result: anyhow::Result<TestResponse> = client
            .get(Api::Spot(Spot::Order), Some(&query_params), true)
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_post_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock(
                "POST",
                mockito::Matcher::Regex(r"^/api/v3/order\?".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"symbol": "BTCUSDT", "price": "50000.0"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let result: anyhow::Result<TestResponse> =
            client.post(Api::Spot(Spot::Order), None, false).await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_response_handler_success() {
        let expected_response = TestResponse {
            symbol: "BTCUSDT".to_string(),
            price: "50000.0".to_string(),
        };

        let response = reqwest::Response::from(
            http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(serde_json::to_string(&expected_response).unwrap())
                .unwrap(),
        );

        let result: anyhow::Result<TestResponse> = response_handler(response).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_response);
    }

    #[tokio::test]
    async fn test_response_handler_internal_server_error() {
        let response = reqwest::Response::from(
            http::Response::builder()
                .status(500)
                .body("Internal Server Error")
                .unwrap(),
        );

        let result: anyhow::Result<TestResponse> = response_handler(response).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Internal Server Error")
        );
    }

    #[tokio::test]
    async fn test_response_handler_service_unavailable() {
        let response = reqwest::Response::from(
            http::Response::builder()
                .status(503)
                .body("Service Unavailable")
                .unwrap(),
        );

        let result: anyhow::Result<TestResponse> = response_handler(response).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Service Unavailable")
        );
    }

    #[tokio::test]
    async fn test_response_handler_unauthorized() {
        let response = reqwest::Response::from(
            http::Response::builder()
                .status(401)
                .body("Unauthorized")
                .unwrap(),
        );

        let result: anyhow::Result<TestResponse> = response_handler(response).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unauthorized"));
    }

    #[tokio::test]
    async fn test_response_handler_other_error() {
        let response = reqwest::Response::from(
            http::Response::builder()
                .status(400)
                .body("Bad Request: Invalid symbol")
                .unwrap(),
        );

        let result: anyhow::Result<TestResponse> = response_handler(response).await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Received error"));
        assert!(error_msg.contains("code=400"));
        assert!(error_msg.contains("Bad Request: Invalid symbol"));
    }

    #[test]
    fn test_build_query() {
        let params = vec![
            ("key1".to_string(), "value1".to_string()),
            ("key2".to_string(), "value2".to_string()),
        ];

        let query = build_query(&params);
        assert!(query.contains("key1=value1"));
        assert!(query.contains("key2=value2"));
        assert!(query.contains('&'));
        assert_eq!(query.len(), "key1=value1&key2=value2".len());
    }

    #[test]
    fn test_build_query_empty() {
        let params: Vec<(String, String)> = vec![];
        let query = build_query(&params);
        assert_eq!(query, "");
    }

    #[test]
    fn test_build_query_single_param() {
        let params = vec![("key1".to_string(), "value1".to_string())];
        let query = build_query(&params);
        assert_eq!(query, "key1=value1");
    }

    #[test]
    fn test_build_headers() {
        let client = create_test_client("https://api.binance.com");
        let headers = client.build_headers().unwrap();

        assert!(headers.contains_key("x-mbx-apikey"));
        let api_key_value = headers.get("x-mbx-apikey").unwrap();
        assert_eq!(api_key_value, "test_api_key");
    }

    #[test]
    fn test_build_url_without_query_and_signature() {
        let client = create_test_client("https://api.binance.com");
        let url = client
            .build_url(Api::Spot(Spot::Ping), None, false)
            .unwrap();

        assert_eq!(url, "https://api.binance.com/api/v3/ping?");
    }

    #[test]
    fn test_build_url_with_query_without_signature() {
        let client = create_test_client("https://api.binance.com");
        let query_params = vec![("symbol".to_string(), "BTCUSDT".to_string())];

        let url = client
            .build_url(Api::Spot(Spot::Price), Some(&query_params), false)
            .unwrap();

        assert_eq!(
            url,
            "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT"
        );
    }

    #[test]
    fn test_build_url_with_query_and_signature() {
        let client = create_test_client("https://api.binance.com");
        let query_params = vec![("symbol".to_string(), "BTCUSDT".to_string())];

        let url = client
            .build_url(Api::Spot(Spot::Order), Some(&query_params), true)
            .unwrap();

        assert!(url.starts_with("https://api.binance.com/api/v3/order?"));
        assert!(url.contains("symbol=BTCUSDT"));
        assert!(url.contains("signature="));
        assert!(url.matches("signature=").count() == 1);
    }

    #[test]
    fn test_build_url_without_query_with_signature() {
        let client = create_test_client("https://api.binance.com");
        let url = client
            .build_url(Api::Spot(Spot::Account), None, true)
            .unwrap();

        assert!(url.starts_with("https://api.binance.com/api/v3/account?"));
        assert!(url.contains("signature="));
        assert!(url.contains("?signature="));
    }

    #[test]
    fn test_http_config_default() {
        let config = HttpConfig::default();

        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.pool_idle_timeout, Duration::from_secs(120));
        assert_eq!(config.pool_max_idle_per_host, 5);
        assert_eq!(config.tcp_keepalive, Duration::from_secs(120));
        assert_eq!(config.tcp_keepalive_interval, Duration::from_secs(30));
        assert_eq!(config.tcp_keepalive_retries, 5);
        assert_eq!(config.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();

        assert_eq!(config.api_url, "");
        assert_eq!(config.api_token, "");
        assert_eq!(config.api_secret_key, "");
        assert_eq!(config.http_config.connect_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_build_headers_invalid_api_key() {
        let client = Client {
            host: "https://api.binance.com".to_string(),
            api_key: "invalid\nkey".to_string(),
            secret_key: "test_secret".to_string(),
            inner_client: reqwest::Client::new(),
        };

        let result = client.build_headers();
        assert!(result.is_err());
    }
}
