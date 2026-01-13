use std::sync::Arc;

use tools::http::http_server::HttpServerProcess;

use crate::{Exchange, Sender, ServiceFactory, runtime::process::GenericProcess};

pub async fn build_services<P, C>(
    config: &C,
) -> anyhow::Result<(Arc<dyn Exchange>, Arc<dyn Sender>)>
where
    // Используем прямой синтаксис ассоциированного типа Config = C
    P: ServiceFactory<dyn Exchange, Config = C> + ServiceFactory<dyn Sender, Config = C>,
{
    let exchange = P::from_config(config).await?;
    let sender = P::from_config(config).await?;
    Ok((exchange, sender))
}

pub fn build_processes(
    exchange: Arc<dyn Exchange>,
    sender: Arc<dyn Sender>,
) -> Vec<Arc<dyn HttpServerProcess>> {
    vec![
        Arc::new(GenericProcess::new(exchange)),
        Arc::new(GenericProcess::new(sender)),
    ]
}
