extern crate arb_bot_rs as app;

use crate::entrypoint::{Config, Entrypoint};

mod entrypoint;

use app::libs::setup_application;
use tracing::error;

const CONFIG_FILE: &str = "config.yml";

#[tokio::main]
async fn main() {
    setup_application(env!("CARGO_PKG_NAME"));

    let config = Config::parse(CONFIG_FILE);
    let entry = Entrypoint::from_config(config);

    match entry.run().await {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            error!("{:?}", e);
            std::process::exit(1)
        }
    };
}
