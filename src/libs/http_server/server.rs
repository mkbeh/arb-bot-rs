use std::{fmt::Display, future::ready, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::{Router, routing::get};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tokio::{signal, task::JoinHandle, time::timeout};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[async_trait]
pub trait ServerProcess: Send + Sync + 'static {
    async fn pre_run(&self) -> Result<()>;
    async fn run(&self, token: CancellationToken) -> Result<()>;
}

#[derive(Default)]
pub struct Server {
    addr: String,
    metrics_addr: String,
    pre_run_tasks_timeout: Duration,
    processes: Option<Vec<Arc<dyn ServerProcess>>>,
}

impl Server {
    pub fn new(addr: String, metrics_addr: String) -> Self {
        Self {
            addr,
            metrics_addr,
            pre_run_tasks_timeout: Duration::from_secs(60),
            processes: None,
        }
    }

    pub fn with_processes(mut self, processes: Vec<Arc<dyn ServerProcess>>) -> Self {
        self.processes = Some(processes);
        self
    }

    pub async fn run(&self) -> Result<()> {
        // Pre-run processes
        let empty_vec = Vec::new();
        let processes = self.processes.as_ref().unwrap_or(&empty_vec);
        Self::pre_run_processes(processes, self.pre_run_tasks_timeout).await?;

        // Setup panic hook
        Self::setup_panic_hook();

        // Spawn run processes early (concurrent with servers)
        let shutdown = CancellationToken::new();
        let mut runnable_tasks = Self::run_processes(processes, shutdown.clone());

        // Bootstrap servers
        let app_server =
            bootstrap_server(&self.addr, get_default_router(), ServerKind::Application);
        let metrics_server = bootstrap_server(
            &self.metrics_addr,
            get_metrics_router(),
            ServerKind::Metrics,
        );

        // Run servers
        tokio::try_join!(app_server, metrics_server).context("Failed to bootstrap servers")?;

        // Signal shutdown (cancels token for processes)
        shutdown.cancel();

        // Wait for run tasks to finish gracefully
        Self::shutdown_processes(&mut runnable_tasks).await;

        Ok(())
    }

    async fn pre_run_processes(
        processes: &[Arc<dyn ServerProcess>],
        tasks_timeout: Duration,
    ) -> Result<()> {
        let tasks: Vec<_> = processes
            .iter()
            .map(|p| {
                let p = Arc::clone(p);
                tokio::spawn(async move { timeout(tasks_timeout, p.pre_run()).await })
            })
            .collect();

        for task in tasks {
            let result = task.await?.with_context(|| "Pre-run task failed")?;
            result?;
        }

        Ok(())
    }

    fn run_processes(
        processes: &[Arc<dyn ServerProcess>],
        token: CancellationToken,
    ) -> Vec<JoinHandle<Result<()>>> {
        processes
            .iter()
            .map(|p| {
                let p = Arc::clone(p);
                let token = token.clone();
                tokio::spawn(async move { p.run(token).await })
            })
            .collect()
    }

    async fn shutdown_processes(tasks: &mut [JoinHandle<Result<()>>]) {
        for task in tasks.iter_mut() {
            if let Err(e) = task.await {
                error!("Failed to await run task: {:?}", e);
            }
        }
    }

    fn setup_panic_hook() {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            default_hook(panic_info);
            if let Some(location) = panic_info.location() {
                error!(
                    message = %panic_info,
                    panic.file = %location.file(),
                    panic.line = %location.line(),
                    panic.column = %location.column(),
                );
            } else {
                error!(message = %panic_info);
            }
        }));
    }
}

async fn bootstrap_server(addr: &str, router: Router, server_kind: ServerKind) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to address: {}", addr))?;

    info!("Listening {server_kind} server on {}", addr);

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .with_context(|| format!("Failed to start {server_kind} server on {}", addr))?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    let quit = async {
        signal::unix::signal(signal::unix::SignalKind::quit())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = quit => {},
    }
}

#[derive(Copy, Clone)]
enum ServerKind {
    Application,
    Metrics,
}

impl Display for ServerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Application => write!(f, "app"),
            Self::Metrics => write!(f, "metrics"),
        }
    }
}

fn get_default_router() -> Router {
    Router::new()
        .route("/readiness", get(|| async { "OK" }))
        .route("/liveness", get(|| async { "OK" }))
}

fn get_metrics_router() -> Router {
    let recorder_handle = setup_metrics_recorder();
    get_default_router().route("/metrics", get(move || ready(recorder_handle.render())))
}

fn setup_metrics_recorder() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
}
