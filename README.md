# ⚡ arb-bot-rs

![GitHub CI](https://github.com/mkbeh/arb-bot-rs/actions/workflows/ci.yml/badge.svg)

Arbitrage bot is a high-frequency arbitrage trading system that automatically identifies and executes profitable
triangular arbitrage opportunities on cryptocurrency exchanges.

[📖 Documentation](https://github.com/mkbeh/arb-bot-rs/tree/main/docs) | [📊 Monitoring](https://github.com/mkbeh/arb-bot-rs/tree/main/deploy)

---

### 📖 Table of Contents

- [⚡ Quick Start](#-quick-start)
- [📊 Observability & Monitoring](#-observability--monitoring)
- [🏛 Supported Exchanges](#-supported-exchanges)
- [❤️ Support & Donations](#-support-us--become-part-of-the-magic)

## ⚡ Quick Start

Application is written in Rust, so you'll need to grab a
[Rust installation](https://www.rust-lang.org/) in order to compile it.

#### 🛠 Prerequisites

* **Rust**: 1.91.0 or newer.

* **Linux Users**: Requires `clang` and `lld` for high-performance linking:

**Ubuntu/Debian:**

```shell
sudo apt-get update && sudo apt-get install -y clang lld
```

#### 🏗 Build from Source

```shell
git clone https://github.com/mkbeh/arb-bot-rs.git
cd arb-bot-rs
cargo build -p cli --profile release-lto --all-features
```

#### ✅ Running Tests

The project includes a robust suite of unit and integration tests.

```shell
cargo test --all
```

### 🚀 Execution & CLI

The bot provides a structured command-line interface:

```text
Commands:
  list     List available exchanges
  version  Show version
  run      Run arbitrage bot
  help     Print this message or the help of the given subcommand(s)
```

**Steps to start:**

1. **Configure**: Copy the example config and add your API keys.

```shell
cp config.example.toml config.toml
```

2. **Start Trading**: Execute the run command.

```shell
RUST_LOG=INFO ./release-lto/bot run --exchange binance --config config.toml 2>&1 | tee debug_$(date "+%Y.%m.%d-%H.%M.%S").log
```

## 📊 Observability & Monitoring

![Grafana](https://img.shields.io/badge/-Grafana-orange?logo=grafana&logoColor=white&style=flat)
![Prometheus](https://img.shields.io/badge/-Prometheus-red?logo=prometheus&logoColor=white&style=flat)

The bot's core performance is monitored in real-time using a Grafana dashboard, providing deep insights into market data
processing and arbitrage efficiency.

| Category       | Metric            | Insight                                                      |
|----------------|-------------------|--------------------------------------------------------------|
| 📈 Market Data | Update Rate       | 🔄 Frequency of order book events across all exchanges.      |
|                | Hot Pairs         | 💱 Identification of the most volatile trading pairs.        |
| ⚡ Engine       | Analysis Speed    | ⏱️ Number of arbitrage chains processed per second.          |
|                | Top Chains        | 💰 Most frequent and profitable currency paths.              |
| 🎯 Strategy    | Profit Ratio      | 📊 Success rate of profitable detections vs. total analyzed. |
|                | Opportunity Count | ✅ Total count of viable arbitrage signals identified.        |
| 🛠️ Execution  | Order Status      | 📝 Real-time log of filled, failed, or canceled attempts.    |

### Dashboard Preview

The dashboard provides a live look at the bot's decision-making process and market impact.

![img](https://github.com/user-attachments/assets/9c3cf8fb-6f0e-4576-9584-4da31482ccea)

_Live dashboard showing market data throughput, arbitrage processing rates, and trading performance._

## 🏛 Supported Exchanges

List of supported cryptocurrency exchanges.

| Exchange | Status | Features                             |
|----------|:------:|--------------------------------------|
| Binance  |   ✅    | Spot, Market Orders                  |
| Kucoin   |   ✅    | Spot, Market Orders                  |
| Solana   |   ⏳    | On-chain Dex (Jupiter, Raydium, etc) |

## ❤️ Support Us – Become Part of the Magic!

Open-source projects thrive because of visionaries like you. If this code has sparked a flame of inspiration in your
heart, share the spark! Your crypto support is the fuel for new features, bug fixes, and groundbreaking updates. We
accept donations in BTC, ETH, and USDT – simple, swift, and borderless.

| Crypto             | Address                                               | QR Code                                                                                                      |
|--------------------|-------------------------------------------------------|--------------------------------------------------------------------------------------------------------------|
| **Bitcoin (BTC)**  | `bc1qw0sz039alzpmk2qcg549pwv3vd0e6casj5dstp`          | <img src="https://gist.github.com/user-attachments/assets/831a4129-f074-432a-ab5d-859e9d538308" width="150"> |
| **Ethereum (ETH)** | `0x00875cdA702B0e6fba3AdeaA6bEB585Db3a7f0f1`          | <img src="https://gist.github.com/user-attachments/assets/9b3dec53-c5ff-4e5b-a897-8f94833703c7" width="150"> |
| **Tether (USDT)**  | `0x00875cdA702B0e6fba3AdeaA6bEB585Db3a7f0f1` (ERC-20) | <img src="https://gist.github.com/user-attachments/assets/9b3dec53-c5ff-4e5b-a897-8f94833703c7" width="150"> |

Every satoshi, every ether – it's a step toward something greater. Thank you for believing in openness! 🌍✨

**Become a Star:** A GitHub star is free, but it means the world. ⭐

## License

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This project is **open source** and distributed under the **MIT license**. You are free to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of the project, subject to the conditions of retaining the copyright
notice.

Full details in the [LICENSE](https://github.com/mkbeh/arb-bot-rs/blob/main/LICENSE) file.

> "Code is poetry. Share it generously!" — inspired by Richard Stallman.