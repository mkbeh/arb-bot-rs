use std::time::Duration;

use anyhow::{anyhow, bail};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::libs::binance_api::api::Api;

pub struct Client {
    host: String,
    api_key: String,
    secret_key: String,
    inner_client: reqwest::Client,
}

impl Client {
    pub fn from_config(cfg: Config) -> anyhow::Result<Self, anyhow::Error> {
        let client = Self {
            host: cfg.host.clone(),
            api_key: cfg.api_key.clone(),
            secret_key: cfg.secret_key.clone(),
            inner_client: reqwest::Client::builder()
                .connect_timeout(cfg.http_config.connect_timeout)
                .pool_idle_timeout(cfg.http_config.pool_idle_timeout)
                .pool_max_idle_per_host(cfg.http_config.pool_max_idle_per_host)
                .tcp_keepalive(cfg.http_config.tcp_keepalive)
                .tcp_keepalive_interval(cfg.http_config.tcp_keepalive_interval)
                .tcp_keepalive_retries(cfg.http_config.tcp_keepalive_retries)
                .build()?,
        };

        Ok(client)
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: Api,
        query: Option<String>,
        q: &Vec<(&str, &str)>,
    ) -> anyhow::Result<T> {
        let mut url = format!("{}{}", self.host, String::from(path));
        if let Some(s) = query {
            if !s.is_empty() {
                url.push_str(format!("?{}", s).as_str());
            }
        };

        let response = self.inner_client.get(url).query(q).send().await?;
        response_handler(response).await
    }
}

async fn response_handler<T: DeserializeOwned>(resp: Response) -> anyhow::Result<T> {
    match resp.status() {
        StatusCode::OK => resp.json::<T>().await.map_err(|e| anyhow!(e)),
        StatusCode::INTERNAL_SERVER_ERROR => {
            bail!("Internal Server Error");
        }
        StatusCode::SERVICE_UNAVAILABLE => {
            bail!("Service Unavailable");
        }
        StatusCode::UNAUTHORIZED => {
            bail!("Unauthorized");
        }
        code => {
            bail!(format!(
                "Received error: code={} msg={}",
                code,
                resp.text().await.map_err(|e| anyhow!(e))?
            ));
        }
    }
}

pub struct Config {
    pub host: String,
    pub api_key: String,
    pub secret_key: String,
    pub http_config: HttpConfig,
}

pub struct HttpConfig {
    pub connect_timeout: Duration,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    pub tcp_keepalive: Duration,
    pub tcp_keepalive_interval: Duration,
    pub tcp_keepalive_retries: u32,
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
        }
    }
}
