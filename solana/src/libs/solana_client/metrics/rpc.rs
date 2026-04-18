use std::{pin::Pin, time::Instant};

use metrics::{Unit, counter, describe_counter, describe_histogram, histogram};
use solana_client::{client_error::ClientError, rpc_request::RpcRequest};
use solana_rpc_client::{
    http_sender::HttpSender,
    rpc_sender::{RpcSender, RpcTransportStats},
};

const METRIC_RPC_TOTAL: &str = "solana_rpc_requests_total";
const METRIC_RPC_DURATION: &str = "solana_rpc_request_duration_seconds";

const LBL_METHOD: &str = "method";
const LBL_STATUS: &str = "status";

fn init_metrics() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        describe_counter!(
            METRIC_RPC_TOTAL,
            Unit::Count,
            "Total RPC requests by method and status"
        );
        describe_histogram!(METRIC_RPC_DURATION, Unit::Seconds, "RPC request latency");
    });
}

#[derive(Debug, Clone, Copy)]
pub enum RpcStatus {
    Ok,
    Error,
}

impl RpcStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }
}

impl From<bool> for RpcStatus {
    fn from(ok: bool) -> Self {
        if ok { Self::Ok } else { Self::Error }
    }
}

pub struct MeterSender {
    inner: HttpSender,
}

impl MeterSender {
    #[must_use]
    pub fn new(url: String) -> Self {
        init_metrics();
        Self {
            inner: HttpSender::new(url),
        }
    }
}

impl RpcSender for MeterSender {
    fn send<'life0, 'async_trait>(
        &'life0 self,
        request: RpcRequest,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ClientError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        let method = request.to_string();
        let inner = &self.inner;

        Box::pin(async move {
            let start = Instant::now();
            let result = inner.send(request, params).await;

            let elapsed = start.elapsed().as_secs_f64();
            let status = RpcStatus::from(result.is_ok());

            histogram!(METRIC_RPC_DURATION, LBL_METHOD => method.clone()).record(elapsed);
            counter!(
                METRIC_RPC_TOTAL,
                LBL_METHOD => method,
                LBL_STATUS => status.as_str()
            )
            .increment(1);

            result
        })
    }

    fn get_transport_stats(&self) -> RpcTransportStats {
        self.inner.get_transport_stats()
    }

    fn url(&self) -> String {
        self.inner.url()
    }
}
