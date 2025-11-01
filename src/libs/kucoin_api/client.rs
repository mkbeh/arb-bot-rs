use std::time::Duration;

use anyhow::{anyhow, bail};
use axum::http::StatusCode;
use reqwest::Response;
use serde::de::DeserializeOwned;

use crate::libs::kucoin_api::api::Api;

pub struct ClientConfig {
    pub host: String,
    pub http_config: HttpConfig,
}

#[derive(Clone)]
pub struct Client {
    host: String,
    inner_client: reqwest::Client,
}

impl Client {
    pub fn from_config(conf: ClientConfig) -> anyhow::Result<Self, anyhow::Error> {
        let client = Self {
            host: conf.host,
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
    ) -> anyhow::Result<T> {
        let url = self.build_url(path, query);
        let response = self.inner_client.get(url).send().await?;
        response_handler(response).await
    }

    fn build_url(&self, path: Api, query: Option<&Vec<(&str, &str)>>) -> String {
        let mut url = format!("{}{}", self.host, String::from(path));
        let mut query_params = String::new();

        if let Some(v) = query {
            query_params.push_str(build_query(v).as_str());
            url.push_str(format!("?{query_params}").as_str());
        }

        url
    }
}

fn build_query(params: &Vec<(&str, &str)>) -> String {
    let mut query = String::new();
    for (k, v) in params {
        query.push_str(&format!("{k}={v}&"));
    }
    query.pop();
    query
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
