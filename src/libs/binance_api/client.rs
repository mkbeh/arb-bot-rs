use std::time::Duration;

use anyhow::{anyhow, bail};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::libs::binance_api::{api::Api, utils::generate_signature};

#[derive(Clone)]
pub struct Client {
    host: String,
    api_key: String,
    secret_key: String,
    inner_client: reqwest::Client,
}

impl Client {
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

    fn build_headers(&self) -> anyhow::Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-mbx-apikey"),
            HeaderValue::from_str(self.api_key.as_str())?,
        );
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

fn build_query(params: &Vec<(String, String)>) -> String {
    let mut query = String::new();
    for (k, v) in params {
        query.push_str(&format!("{k}={v}&"));
    }
    query.pop();
    query
}

#[derive(Default, Clone)]
pub struct ClientConfig {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub http_config: HttpConfig,
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
