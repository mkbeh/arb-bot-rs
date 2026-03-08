# ⚡ arb-bot-rs

![GitHub CI](https://github.com/mkbeh/arb-bot-rs/actions/workflows/ci.yml/badge.svg)

Arbitrage bot is a high-frequency arbitrage trading system that automatically identifies and executes profitable
arbitrage opportunities on cryptocurrency exchanges.

[📖 Full Documentation](https://github.com/mkbeh/arb-bot-rs/tree/main/docs) | [📊 Live Monitoring Setup](https://github.com/mkbeh/arb-bot-rs/tree/main/deploy)

***

### 📖 Table of Contents

- [⚡ Quick Start](#-quick-start)
- [📊 Observability & Monitoring](#-observability--monitoring)
- [🏛 Supported Exchanges](#-supported-exchanges)
- [❤️ Support & Donations](#-support-us--become-part-of-the-magic)

## ⚡ Quick Start

Get up and running in minutes! This bot is optimized for Linux but works on macOS/Windows with minor tweaks.

#### 🛠 Prerequisites

* **Rust**: Version 1.94.0 or newer. Install via [rustup](https://rustup.rs/?referrer=grok.com).

#### 🏗 Build from Source

Clone and build the CLI binary:

```shell
git clone https://github.com/mkbeh/arb-bot-rs.git
cd arb-bot-rs
cargo build -p cli --release --all-features
```

#### ✅ Running Tests

Verify everything works with our full test suite:

```shell
cargo test --all
```

#### 🚀 Execution & CLI

The CLI is intuitive—check available commands:

```shell
./target/release/bot --help
```

**Core Commands:**

* `list`: List supported exchanges.
* `version`: Show bot version.
* `run`: Start the arbitrage engine.

**Configure**: Copy the example config and add your API keys.

```shell
cp config.example.toml config.toml
```

**Launch Example (Binance spot trading):**

```shell
RUST_LOG=INFO ./target/release/bot run --exchange binance --config config.toml
```

## 📊 Observability & Monitoring

![Grafana](https://img.shields.io/badge/-Grafana-orange?logo=grafana&logoColor=white&style=flat)
![Prometheus](https://img.shields.io/badge/-Prometheus-red?logo=prometheus&logoColor=white&style=flat)

Track every tick with Prometheus + Grafana. Setup instructions
in [/deploy](https://github.com/mkbeh/arb-bot-rs/tree/main/deploy).

**Key Metrics Dashboard**:

| Category           | Metric            | Insight                                                      |
|--------------------|-------------------|--------------------------------------------------------------|
| 📈 **Market Data** | Update Rate       | 🔄 Frequency of order book events across all exchanges.      |
|                    | Hot Pairs         | 💱 Identification of the most volatile trading pairs.        |
| ⚡ **Engine**       | Analysis Speed    | ⏱️ Number of arbitrage chains processed per second.          |
|                    | Top Chains        | 💰 Most frequent and profitable currency paths.              |
| 🎯 **Strategy**    | Profit Ratio      | 📊 Success rate of profitable detections vs. total analyzed. |
|                    | Opportunity Count | ✅ Total count of viable arbitrage signals identified.        |
| 🛠️ **Execution**  | Order Status      | 📝 Real-time log of filled, failed, or canceled attempts.    |

### Dashboard Preview

The dashboard provides a live look at the bot's decision-making process and market impact.

![img](https://github.com/user-attachments/assets/29bfa2db-cac9-4f1e-8af8-8ac1f3c5b374)

_Live dashboard showing market data throughput, arbitrage processing rates, and trading performance._

## 🏛 Supported Exchanges

List of supported cryptocurrency exchanges.

|  Exchange   | Status | Features                                     |
|:-----------:|:------:|----------------------------------------------|
| **Binance** | ✅ Live | Spot trading, market orders, WebSocket feeds |
| **Kucoin**  | ✅ Live | Spot trading, market orders, WebSocket feeds |
| **Solana**  | ⏳ WIP  | On-chain swaps (Jupiter, Raydium, Orca, etc) |

## ❤️ Support Us – Become Part of the Magic!

Open-source projects thrive because of visionaries like you. If this code has sparked a flame of inspiration in your
heart, share the spark! Your crypto support is the fuel for new features, bug fixes, and groundbreaking updates. We
accept donations in BTC, ETH, and USDT – simple, swift, and borderless.

| Crypto             | Address                                               | QR Code                                                                                                 |
|--------------------|-------------------------------------------------------|---------------------------------------------------------------------------------------------------------|
| **Bitcoin (BTC)**  | `bc1qw0sz039alzpmk2qcg549pwv3vd0e6casj5dstp`          | <img src="https://github.com/user-attachments/assets/55b48540-bd38-4567-a409-775dd9400052" width="150"> |
| **Ethereum (ETH)** | `0x00875cdA702B0e6fba3AdeaA6bEB585Db3a7f0f1`          | <img src="https://github.com/user-attachments/assets/6e8b9ce8-86f1-4d94-ba17-0c9bca16718f" width="150"> |
| **Tether (USDT)**  | `0x00875cdA702B0e6fba3AdeaA6bEB585Db3a7f0f1` (ERC-20) | <img src="https://github.com/user-attachments/assets/6e8b9ce8-86f1-4d94-ba17-0c9bca16718f" width="150"> |
| **Solana (SOL)**   | `CP9nNp1jMZS6QK4aoQuRGXKRwHdQ72Rr6ZK3kZcANAz9`        | <img src="https://github.com/user-attachments/assets/eaa382e3-a405-4a13-8534-8365ee02257e" width="150"> |

Every satoshi, every ether – it's a step toward something greater. Thank you for believing in openness! 🌍✨

**Become a Star:** A GitHub star is free, but it means the world. ⭐

## License

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This project is open-source under the [MIT License](https://github.com/mkbeh/arb-bot-rs/blob/main/LICENSE). Use it
freely, but trade responsibly.

> "Code is poetry. Share it generously!" — inspired by Richard Stallman.
