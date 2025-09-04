extern crate arb_bot_rs as app;

use crate::entrypoint::Entrypoint;

mod entrypoint;
use app::libs::setup_application;
use tracing::error;

#[tokio::main]
async fn main() {
    setup_application(env!("CARGO_PKG_NAME"));

    let entry = Entrypoint;

    match entry.run().await {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            error!("{:?}", e);
            std::process::exit(1)
        }
    };
}
