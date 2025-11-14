use std::time::{Duration, SystemTime};

use anyhow::bail;
use axum::http::StatusCode;
use reqwest::{
    Method, RequestBuilder, Response,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde::de::DeserializeOwned;
use tracing::warn;

use crate::libs::kucoin_api::{api::Api, utils};

#[derive(Clone)]
pub struct ClientConfig {
    pub host: String,
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub http_config: HttpConfig,
}

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

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: Api,
        query: Option<&Vec<(&str, &str)>>,
        private: bool,
    ) -> anyhow::Result<T> {
        self.process_request(Method::GET, path, query, None, private)
            .await
    }

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
