mod config;
mod launcher;
mod ui;

use clap::{Parser, Subcommand, ValueEnum};
use strum_macros::{Display, EnumIter, EnumString};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser)]
#[command(name = ui::app_name())]
#[command(about = ui::build_banner())]
struct Cli {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available exchanges
    List,

    /// Show version
    Version,

    /// Run arbitrage bot
    Run {
        /// Exchange to use
        #[arg(short, long)]
        exchange: ExchangeType,

        /// Path to config.toml file
        #[arg(short, long, default_value = "config.toml")]
        config: std::path::PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, EnumString, Display, ValueEnum, EnumIter)]
pub enum ExchangeType {
    #[value(name = "binance")]
    Binance,
    #[value(name = "kucoin")]
    Kucoin,
    #[value(name = "solana")]
    Solana,
}

#[tools::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.commands {
        Commands::Version => ui::print_version(),
        Commands::List => ui::print_exchanges(),
        Commands::Run { exchange, config } => {
            launcher::start(exchange, config).await?;
        }
    }

    Ok(())
}
