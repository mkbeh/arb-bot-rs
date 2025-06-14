use std::{net::SocketAddr, sync::LazyLock, time::Duration};

use anyhow::anyhow;
use async_trait::async_trait;
use axum::{Router, routing::get};
use tokio::{signal, time::timeout};
use tokio_util::sync::CancellationToken;

const ADDR: &str = "127.0.0.1:9000";
const PROCESS_PRE_RUN_TIMEOUT: Duration = Duration::from_secs(60);
static SHUTDOWN_TOKEN: LazyLock<CancellationToken> = LazyLock::new(CancellationToken::new);

#[async_trait]
pub trait Process: Send + Sync {
    async fn pre_run(&self) -> anyhow::Result<()>;
    async fn run(&self, token: CancellationToken) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct Server<'a> {
    addr: String,
    processes: Option<&'a Vec<&'static dyn Process>>,
}

impl<'a> Server<'a> {
    pub fn new() -> Self {
        Self {
            addr: ADDR.to_owned(),
            processes: None,
        }
    }

    pub fn with_processes(mut self, processes: &'a Vec<&'static dyn Process>) -> Self {
        self.processes = Some(processes);
        self
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let srv = bootstrap_server(self.addr.clone(), get_default_router());

        let processes = match self.processes {
            Some(processes) => processes,
            _ => &vec![],
        };

        // pre run processes
        {
            let tasks: Vec<_> = processes
                .iter()
                .map(|p| {
                    tokio::spawn(timeout(PROCESS_PRE_RUN_TIMEOUT, async {
                        p.pre_run().await
                    }))
                })
                .collect();

            for task in tasks {
                if let Err(e) = task.await? {
                    return Err(anyhow!("error while pre run process: {}", e));
                }
            }
        }

        // disable failure in the custom panic hook when there is a panic,
        // because we can't handle the panic in the panic middleware (exit(1) trouble)
        setup_panic_hook();

        {
            // run processes
            let runnable_tasks: Vec<_> = processes
                .iter()
                .map(|p| tokio::spawn(async { p.run(SHUTDOWN_TOKEN.clone()).await }))
                .collect();

            tokio::try_join!(srv)
                .map_err(|e| anyhow!("Failed to bootstrap server. Reason: {:?}", e))?;

            SHUTDOWN_TOKEN.cancel();

            for task in runnable_tasks {
                if let Err(e) = task.await? {
                    tracing::error!("Failed to shutdown processes. Reason: {:?}", e);
                }
            }
        }

        Ok(())
    }
}

async fn bootstrap_server(addr: String, router: Router) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr.clone())
        .await
        .map_err(|e| anyhow!("failed to bind to address: {e}"))?;

    tracing::info!("listening server on {addr}");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| anyhow!("failed to start server on address {addr}: {e}"))?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    let quit = async {
        signal::unix::signal(signal::unix::SignalKind::quit())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = quit => {},
    }
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(move |panic_info| {
        // If the panic has a source location, record it as structured fields.
        if let Some(location) = panic_info.location() {
            tracing::error!(
                message = %panic_info,
                panic.file = location.file(),
                panic.line = location.line(),
                panic.column = location.column(),
            );
        } else {
            tracing::error!(message = %panic_info);
        }
    }))
}

fn get_default_router() -> Router {
    Router::new()
        .route("/readiness", get(|| async {}))
        .route("/liveness", get(|| async {}))
}
