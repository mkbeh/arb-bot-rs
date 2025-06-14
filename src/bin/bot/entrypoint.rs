use anyhow::anyhow;
use app::{cron::bot::BotProcess, libs::http_server::Server};

pub struct Config {
    //
}

impl Config {
    pub fn parse(_filename: &str) -> Self {
        Self {}
    }
}

pub struct Entrypoint {
    //
}

impl Entrypoint {
    pub fn from_config(_cfg: Config) -> Self {
        Self {}
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let bot_ps = BotProcess::new();

        Server::new()
            .with_processes(&vec![bot_ps])
            .run()
            .await
            .map_err(|err| anyhow!("handling server error: {}", err))?;

        Ok(())
    }
}
