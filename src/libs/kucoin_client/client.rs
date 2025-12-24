//! KuCoin API client module.
//!
//! # Usage
//!
//! ```rust,no_run
//! use std::time::Duration;
//!
//! use anyhow::Result;
//! use arb_bot_rs::libs::kucoin_api::{
//!     Client, ClientConfig,
//!     api::{Api, Spot},
//!     models::{RestResponse, Token},
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let config = ClientConfig {
//!         host: "https://api.kucoin.com".to_string(),
//!         api_key: "your-api-key".to_string(),
//!         api_secret: "your-api-secret".to_string(),
//!         api_passphrase: "your-passphrase".to_string(),
//!         http_config: Default::default(),
//!     };
//!
//!     let client = Client::from_config(config)?;
//!     // Example public POST request.
//!     let response: RestResponse<Token> = client
//!         .post(Api::Spot(Spot::GetBulletPublic), None, None, false)
//!         .await?;
//!     println!("Response: {:?}", response);
//!     Ok(())
//! }
//! ```

use std::time::{Duration, SystemTime};

use anyhow::bail;
use axum::http::StatusCode;
use reqwest::{
    Method, RequestBuilder, Response,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde::de::DeserializeOwned;
use tracing::warn;

use crate::libs::kucoin_client::{api::Api, utils};

/// Configuration for the KuCoin API client.
///
/// Holds credentials and HTTP settings for client initialization.
#[derive(Clone)]
pub struct ClientConfig {
    /// The base host URL for KuCoin API
    pub host: String,
    /// API key for authentication.
    pub api_key: String,
    /// API secret for signature generation.
    pub api_secret: String,
    /// API passphrase (signed with secret if provided).
    pub api_passphrase: String,
    /// HTTP client configuration (timeouts, pooling, etc.).
    pub http_config: HttpConfig,
}

/// Primary client struct for making KuCoin API requests.
#[derive(Clone)]
pub struct Client {
    host: String,
    api_key: String,
    api_secret: String,
    api_passphrase: String,
    inner_client: reqwest::Client,
}

impl Client {
    pub fn from_config(conf: ClientConfig) -> anyhow::Result<Self, anyhow::Error> {
        let signed_passphrase = if !conf.api_passphrase.is_empty() && !conf.api_secret.is_empty() {
            utils::sign(&conf.api_passphrase, &conf.api_secret)
        } else {
            conf.api_passphrase.clone()
        };

        if conf.api_key.is_empty() || conf.api_secret.is_empty() || signed_passphrase.is_empty() {
            warn!("API credentials incomplete. Public endpoints only.");
        }

        let client = Self {
            host: conf.host,
            api_key: conf.api_key,
            api_secret: conf.api_secret,
            api_passphrase: signed_passphrase,
            inner_client: reqwest::Client::builder()
                .connect_timeout(conf.http_config.connect_timeout)
                .pool_idle_timeout(conf.http_config.pool_idle_timeout)
                .pool_max_idle_per_host(conf.http_config.pool_max_idle_per_host)
                .tcp_keepalive(conf.http_config.tcp_keepalive)
                .tcp_keepalive_interval(conf.http_config.tcp_keepalive_interval)
                .tcp_keepalive_retries(conf.http_config.tcp_keepalive_retries)
                .timeout(conf.http_config.timeout)
                .build()?,
        };

        Ok(client)
    }

    /// Performs a GET request to the specified API endpoint.
    ///
    /// Deserializes the response into the provided type `T`.
    ///
    /// # Arguments
    ///
    /// * `path` - The API endpoint (e.g., `Api::Spot(Spot::ServerTime)`).
    /// * `query` - Optional query parameters as `Vec<(&str, &str)>`.
    /// * `private` - Whether to authenticate the request.
    ///
    /// # Returns
    ///
    /// A deserialized `T` on success.
    ///
    /// # Errors
    ///
    /// Propagates errors from request processing, response handling, or deserialization.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: Api,
        query: Option<&Vec<(&str, &str)>>,
        private: bool,
    ) -> anyhow::Result<T> {
        self.process_request(Method::GET, path, query, None, private)
            .await
    }

    /// Performs a POST request to the specified API endpoint.
    ///
    /// Deserializes the response into the provided type `T`.
    ///
    /// # Arguments
    ///
    /// * `path` - The API endpoint (e.g., `Api::Spot(Spot::SomeEndpoint)`).
    /// * `query` - Optional query parameters as `Vec<(&str, &str)>`.
    /// * `body` - Optional JSON body as `&str`.
    /// * `private` - Whether to authenticate the request.
    ///
    /// # Returns
    ///
    /// A deserialized `T` on success.
    ///
    /// # Errors
    ///
    /// Propagates errors from request processing, response handling, or deserialization.
    pub async fn post<T: DeserializeOwned>(
        &self,
        path: Api,
        query: Option<&Vec<(&str, &str)>>,
        body: Option<&str>,
        private: bool,
    ) -> anyhow::Result<T> {
        self.process_request(Method::POST, path, query, body, private)
            .await
    }

    /// Internal method to process a generic HTTP request.
    ///
    /// Builds the URL, adds authentication headers if private, executes the request,
    /// and handles the response.
    ///
    /// # Arguments
    ///
    /// * `method` - The HTTP method (GET or POST).
    /// * `path` - The API endpoint.
    /// * `query` - Optional query parameters.
    /// * `body` - Optional request body.
    /// * `private` - Authentication flag.
    ///
    /// # Returns
    ///
    /// A deserialized `T` on success.
    ///
    /// # Errors
    ///
    /// - URL building failures (e.g., encoding errors).
    /// - Header construction errors (e.g., invalid values).
    /// - Request execution or response handling errors.
    async fn process_request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: Api,
        query: Option<&Vec<(&str, &str)>>,
        body: Option<&str>,
        private: bool,
    ) -> anyhow::Result<T> {
        let (full_url, raw_url) = self.build_urls(&path, query)?;
        let mut req_builder: RequestBuilder = self.inner_client.request(method.clone(), full_url);

        if private {
            let headers = self.build_headers(&method, &raw_url, body)?;
            req_builder = req_builder.headers(headers);
        }

        if let Some(body_str) = body {
            req_builder = req_builder.body(body_str.to_string());
        }

        let request = req_builder.build()?;

        let response = self.inner_client.execute(request).await?;
        response_handler(response).await
    }

    /// Builds the full and raw URLs for the request.
    ///
    /// Encodes query parameters if provided.
    ///
    /// # Arguments
    ///
    /// * `path` - The API path as string.
    /// * `query` - Optional query parameters.
    ///
    /// # Returns
    ///
    /// A tuple of `(full_url, raw_url)` on success.
    ///
    /// # Errors
    ///
    /// - Query encoding failures.
    fn build_urls(
        &self,
        path: &Api,
        query: Option<&Vec<(&str, &str)>>,
    ) -> anyhow::Result<(String, String)> {
        let path_str = path.as_str();
        let mut full_url = format!("{}{}", self.host, path_str);
        let mut raw_url = path_str.to_string();

        if let Some(v) = query {
            let encoded = serde_urlencoded::to_string(v)?;
            full_url.push_str(format!("?{encoded}").as_str());
            raw_url.push_str(format!("?{encoded}").as_str());
        };

        Ok((full_url, raw_url))
    }

    /// Builds authentication headers for private requests.
    ///
    /// Generates timestamp, payload, signature, and sets KuCoin-specific headers.
    ///
    /// # Arguments
    ///
    /// * `method` - The HTTP method.
    /// * `raw_url` - The raw URL path (for payload).
    /// * `body` - Optional body (for payload).
    ///
    /// # Returns
    ///
    /// A `HeaderMap` with authentication headers.
    ///
    /// # Errors
    ///
    /// - Timestamp generation failures.
    /// - Invalid header values (e.g., non-UTF8).
    fn build_headers(
        &self,
        method: &Method,
        raw_url: &str,
        body: Option<&str>,
    ) -> anyhow::Result<HeaderMap> {
        let method_str = method.as_str().to_uppercase();
        let body_str = body.unwrap_or("");
        let payload = format!("{}{}{}", method_str, raw_url, body_str);

        let timestamp = utils::get_timestamp(SystemTime::now())?;
        let timestamp_str = timestamp.to_string();
        let message = format!("{}{}", timestamp_str, payload);
        let signature = utils::sign(&message, &self.api_secret);

        let mut headers = HeaderMap::new();
        headers.insert("KC-API-KEY", self.api_key.parse::<HeaderValue>()?);
        headers.insert(
            "KC-API-PASSPHRASE",
            self.api_passphrase.parse::<HeaderValue>()?,
        );
        headers.insert("KC-API-TIMESTAMP", timestamp_str.parse::<HeaderValue>()?);
        headers.insert("KC-API-SIGN", signature.parse::<HeaderValue>()?);
        headers.insert("KC-API-KEY-VERSION", "2".parse::<HeaderValue>()?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        Ok(headers)
    }
}

/// Handles HTTP responses and deserializes successful ones.
///
/// Bails with contextual errors for common failure codes.
async fn response_handler<T: DeserializeOwned>(resp: Response) -> anyhow::Result<T> {
    match resp.status() {
        StatusCode::OK => {
            let body = resp.bytes().await?;
            Ok(serde_json::from_slice::<T>(&body)?)
        }
        StatusCode::INTERNAL_SERVER_ERROR => bail!("Internal Server Error"),
        StatusCode::SERVICE_UNAVAILABLE => bail!("Service Unavailable"),
        StatusCode::UNAUTHORIZED => {
            let err_body = resp.text().await.unwrap_or_default();
            bail!("Unauthorized: {}", err_body)
        }
        code => {
            let err_body = resp.text().await.unwrap_or_default();
            bail!("Error {}: {}", code, err_body)
        }
    }
}

/// HTTP configuration for the client.
///
/// Provides tunable settings for connection pooling, timeouts, and TCP keepalive.
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
    use crate::libs::kucoin_client::api::Spot;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestResponse {
        code: String,
        data: String,
    }

    fn create_test_client(server_url: &str) -> Client {
        let config = ClientConfig {
            host: server_url.to_string(),
            api_key: "test_api_key".to_string(),
            api_secret: "test_api_secret".to_string(),
            api_passphrase: "test_passphrase".to_string(),
            http_config: HttpConfig::default(),
        };

        Client::from_config(config).unwrap()
    }

    fn create_client_without_credentials(server_url: &str) -> Client {
        let config = ClientConfig {
            host: server_url.to_string(),
            api_key: "".to_string(),
            api_secret: "".to_string(),
            api_passphrase: "".to_string(),
            http_config: HttpConfig::default(),
        };

        Client::from_config(config).unwrap()
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = ClientConfig {
            host: "https://api.kucoin.com".to_string(),
            api_key: "test_key".to_string(),
            api_secret: "test_secret".to_string(),
            api_passphrase: "test_pass".to_string(),
            http_config: HttpConfig::default(),
        };

        let client = Client::from_config(config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_client_creation_with_incomplete_credentials() {
        let config = ClientConfig {
            host: "https://api.kucoin.com".to_string(),
            api_key: "".to_string(),
            api_secret: "test_secret".to_string(),
            api_passphrase: "test_pass".to_string(),
            http_config: HttpConfig::default(),
        };

        let client = Client::from_config(config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_get_public_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v2/symbols")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"code": "200000", "data": "success"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let result: anyhow::Result<TestResponse> = client
            .get(Api::Spot(Spot::GetAllSymbols), None, false)
            .await;

        mock.assert();
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.code, "200000");
        assert_eq!(response.data, "success");
    }

    #[tokio::test]
    async fn test_get_private_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v2/symbols")
            .match_header("KC-API-KEY", "test_api_key")
            .match_header("KC-API-PASSPHRASE", mockito::Matcher::Any)
            .match_header("KC-API-TIMESTAMP", mockito::Matcher::Any)
            .match_header("KC-API-SIGN", mockito::Matcher::Any)
            .match_header("KC-API-KEY-VERSION", "2")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(r#"{"code": "200000", "data": "accounts"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let result: anyhow::Result<TestResponse> = client
            .get(
                Api::Spot(Spot::GetAllSymbols),
                None,
                true, // private request
            )
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_with_query_params() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v2/symbols?symbol=BTC-USDT&status=active")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"code": "200000", "data": "orders"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let query_params = vec![("symbol", "BTC-USDT"), ("status", "active")];

        let result: anyhow::Result<TestResponse> = client
            .get(Api::Spot(Spot::GetAllSymbols), Some(&query_params), false)
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_post_public_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v2/symbols")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"code": "200000", "data": "order_created"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let body = r#"{"symbol": "BTC-USDT", "side": "buy", "price": "50000"}"#;
        let result: anyhow::Result<TestResponse> = client
            .post(Api::Spot(Spot::GetAllSymbols), None, Some(body), false)
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_post_private_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v2/symbols")
            .match_header("KC-API-KEY", "test_api_key")
            .match_header("KC-API-PASSPHRASE", mockito::Matcher::Any)
            .match_header("KC-API-TIMESTAMP", mockito::Matcher::Any)
            .match_header("KC-API-SIGN", mockito::Matcher::Any)
            .match_header("KC-API-KEY-VERSION", "2")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(r#"{"code": "200000", "data": "order_created"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let body = r#"{"symbol": "BTC-USDT", "side": "buy", "price": "50000"}"#;
        let result: anyhow::Result<TestResponse> = client
            .post(Api::Spot(Spot::GetAllSymbols), None, Some(body), true)
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_post_with_query_and_body() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/v2/symbols?type=limit")
            .match_header("KC-API-KEY", "test_api_key")
            .match_header("KC-API-SIGN", mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"code": "200000", "data": "order"}"#)
            .create_async()
            .await;

        let client = create_test_client(&server.url());
        let query_params = vec![("type", "limit")];
        let body = r#"{"symbol": "BTC-USDT", "side": "buy"}"#;

        let result: anyhow::Result<TestResponse> = client
            .post(
                Api::Spot(Spot::GetAllSymbols),
                Some(&query_params),
                Some(body),
                true,
            )
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_response_handler_success() {
        let expected_response = TestResponse {
            code: "200000".to_string(),
            data: "test_data".to_string(),
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
                .body("Invalid API key")
                .unwrap(),
        );

        let result: anyhow::Result<TestResponse> = response_handler(response).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Unauthorized"));
        assert!(error_msg.contains("Invalid API key"));
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
        assert!(error_msg.contains("Error 400"));
        assert!(error_msg.contains("Bad Request: Invalid symbol"));
    }

    #[tokio::test]
    async fn test_response_handler_empty_body_error() {
        let response =
            reqwest::Response::from(http::Response::builder().status(429).body("").unwrap());

        let result: anyhow::Result<TestResponse> = response_handler(response).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Error 429"));
    }

    #[test]
    fn test_build_urls_without_query() {
        let client = create_test_client("https://api.kucoin.com");
        let (full_url, raw_url) = client
            .build_urls(&Api::Spot(Spot::GetAllSymbols), None)
            .unwrap();

        assert_eq!(full_url, "https://api.kucoin.com/api/v2/symbols");
        assert_eq!(raw_url, "/api/v2/symbols");
    }

    #[test]
    fn test_build_urls_with_query() {
        let client = create_test_client("https://api.kucoin.com");
        let query_params = vec![("symbol", "BTC-USDT"), ("limit", "10")];

        let (full_url, raw_url) = client
            .build_urls(&Api::Spot(Spot::GetAllSymbols), Some(&query_params))
            .unwrap();

        assert!(full_url.starts_with("https://api.kucoin.com/api/v2/symbols?"));
        assert!(full_url.contains("symbol=BTC-USDT"));
        assert!(full_url.contains("limit=10"));

        assert!(raw_url.starts_with("/api/v2/symbols?"));
        assert!(raw_url.contains("symbol=BTC-USDT"));
        assert!(raw_url.contains("limit=10"));
    }

    #[test]
    fn test_build_headers_get() {
        let client = create_test_client("https://api.kucoin.com");
        let headers = client
            .build_headers(&Method::GET, "/api/v1/accounts", None)
            .unwrap();

        assert_eq!(headers.get("KC-API-KEY").unwrap(), "test_api_key");
        assert!(headers.get("KC-API-PASSPHRASE").is_some());
        assert!(headers.get("KC-API-TIMESTAMP").is_some());
        assert!(headers.get("KC-API-SIGN").is_some());
        assert_eq!(headers.get("KC-API-KEY-VERSION").unwrap(), "2");
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
    }

    #[test]
    fn test_build_headers_post_with_body() {
        let client = create_test_client("https://api.kucoin.com");
        let body = r#"{"symbol": "BTC-USDT"}"#;
        let headers = client
            .build_headers(&Method::POST, "/api/v1/orders", Some(body))
            .unwrap();

        assert_eq!(headers.get("KC-API-KEY").unwrap(), "test_api_key");
        assert!(headers.get("KC-API-SIGN").is_some());
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
    }

    #[test]
    fn test_build_headers_with_query_in_url() {
        let client = create_test_client("https://api.kucoin.com");
        let headers = client
            .build_headers(
                &Method::GET,
                "/api/v1/orders?symbol=BTC-USDT&limit=10",
                None,
            )
            .unwrap();

        assert_eq!(headers.get("KC-API-KEY").unwrap(), "test_api_key");
        assert!(headers.get("KC-API-SIGN").is_some());
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
    fn test_client_config_creation() {
        let config = ClientConfig {
            host: "https://api.kucoin.com".to_string(),
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            api_passphrase: "pass".to_string(),
            http_config: HttpConfig::default(),
        };

        assert_eq!(config.host, "https://api.kucoin.com");
        assert_eq!(config.api_key, "key");
        assert_eq!(config.api_secret, "secret");
        assert_eq!(config.api_passphrase, "pass");
    }

    #[tokio::test]
    async fn test_network_error() {
        let client = create_test_client("http://invalid-url-that-does-not-exist:9999");

        let result: anyhow::Result<TestResponse> = client
            .get(Api::Spot(Spot::GetAllSymbols), None, false)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("error") || error_msg.contains("fail"));
    }

    #[tokio::test]
    async fn test_public_only_with_empty_credentials() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v2/symbols")
            .with_status(200)
            .with_body(r#"{"code": "200000", "data": "time"}"#)
            .create_async()
            .await;

        let client = create_client_without_credentials(&server.url());
        let result: anyhow::Result<TestResponse> = client
            .get(Api::Spot(Spot::GetAllSymbols), None, false)
            .await;

        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_different_methods() {
        let mut server = Server::new_async().await;

        let _ = server
            .mock("DELETE", "/api/v2symbols/123")
            .match_header("KC-API-KEY", "test_api_key")
            .with_status(200)
            .with_body(r#"{"code": "200000", "data": "deleted"}"#)
            .create_async()
            .await;

        let _ = create_test_client(&server.url());
    }
}
