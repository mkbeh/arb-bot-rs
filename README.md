# arb-bot-rs

![GitHub CI](https://github.com/mkbeh/arb-bot-rs/actions/workflows/ci.yml/badge.svg)

Arbitrage bot is a high-frequency arbitrage trading system that automatically identifies and executes profitable
triangular arbitrage opportunities on cryptocurrency exchanges.

Full documentation of the project can be found [here](https://github.com/mkbeh/arb-bot-rs/tree/main/docs).

### Content

* [Installation](#installation)
* [Usage](#usage)
* [Monitoring](#monitoring)
* [Translations](#translations)

### Supports

List of supported cryptocurrency exchanges.

| Exchange | Status  |
|:--------:|:-------:|
| Binance  | &check; |
|  Kucoin  | &check; |

### ❤️ Support Us – Become Part of the Magic!

Open-source projects thrive because of visionaries like you. If this code has sparked a flame of inspiration in your
heart, share the spark! Your crypto support is the fuel for new features, bug fixes, and groundbreaking updates. We
accept donations in BTC, ETH, and USDT – simple, swift, and borderless.

| Crypto             | Address                                               | QR Code                                       |
|--------------------|-------------------------------------------------------|-----------------------------------------------|
| **Bitcoin (BTC)**  | `bc1qw0sz039alzpmk2qcg549pwv3vd0e6casj5dstp`          | <img src="assets/img/btc_qr.png" width="100"> |
| **Ethereum (ETH)** | `0x00875cdA702B0e6fba3AdeaA6bEB585Db3a7f0f1`          | <img src="assets/img/eth_qr.png" width="100"> |
| **Tether (USDT)**  | `0x00875cdA702B0e6fba3AdeaA6bEB585Db3a7f0f1` (ERC-20) | <img src="assets/img/eth_qr.png" width="100"> |

Every satoshi, every ether – it's a step toward something greater. Thank you for believing in openness! 🌍✨

**Become a Star:** A GitHub star is free, but it means the world. ⭐

## Installation

Application is written in Rust, so you'll need to grab a
[Rust installation](https://www.rust-lang.org/) in order to compile it.
Application compiles with Rust 1.90.0 (stable) or newer.

### Build from source

```shell
git clone https://github.com/mkbeh/arb-bot-rs.git
cd arb-bot-rs
cargo build --release
```

## Usage

Fill in the [example](https://github.com/mkbeh/arb-bot-rs/blob/main/config.example.toml) config file and rename the
file to `config.toml`.

_For demo run you do not need to specify your API tokens. You only need to specify API tokens if you toggle the flag
`send_orders = true` in `config.toml`._

Run app:

```shell
target/release/bot 2>&1 | tee debug_$(date "+%Y.%m.%d-%H.%M.%S").log
```

### Docker

Build image:

```shell
docker build --build-arg SERVICE_NAME=bot --build-arg BUILD_PROFILE=release -t arb-bot-rs:latest .
```

Run app:

```shell
docker run --cpus="1" --cpuset-cpus="0" --memory="512m" arb-bot-rs:latest
```

## Monitoring

![Grafana](https://img.shields.io/badge/-Grafana-orange?logo=grafana&logoColor=white&style=flat)
![Prometheus](https://img.shields.io/badge/-Prometheus-red?logo=prometheus&logoColor=white&style=flat)

The bot's core performance is monitored in real-time using a Grafana dashboard, providing deep insights into market data
processing and arbitrage efficiency.

### 📊 Key Metrics Tracked

| Metric                                                                                      | Description                                                 |
|:--------------------------------------------------------------------------------------------|:------------------------------------------------------------|
| **📈 Market Data Intensity**                                                                |                                                             |
| - Total rate of order book update events from exchanges                                     | 🔄 Rate of updates received from various exchanges.         |
| - The most active trading pairs by update frequency                                         | 💱 Top pairs with the highest volume of order book changes. |
| **⚡ Arbitrage Engine Performance**                                                          |                                                             |
| - How many potential arbitrage chains the engine analyzes per second                        | ⏱️ Chains processed per second for opportunity detection.   |
| - The most frequently processed and profitable currency chains                              | 💰 Top chains by frequency and average profitability.       |
| **🎯 Trading Strategy Effectiveness**                                                       |                                                             |
| - The percentage of profitable chains found versus all chains processed                     | 📊 Success rate of profitable detections (%).               |
| - The absolute count of profitable opportunities identified                                 | ✅ Total number of viable arbitrage opportunities found.     |
| **🛠️ Order Execution Status**                                                              |                                                             |
| - A real-time log of the most recent order execution attempts (success, failure, cancelled) | 📝 Latest executions with status and timestamps.            |            

### Dashboard Preview

The dashboard provides a live look at the bot's decision-making process and market impact.

![img](assets/img/grafana.png)

_Live dashboard showing market data throughput, arbitrage processing rates, and trading performance._

## Running tests

Application is relatively well-tested, including both unit tests and integration tests. To run the full test suite, use:

```shell
cargo test --all
```

## Translations

The following is a list of known translations of application documentation.

* [English](https://github.com/mkbeh/arb-bot-rs/tree/main/docs/en)

## License

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This project is **open source** and distributed under the **MIT license**. You are free to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of the project, subject to the conditions of retaining the copyright
notice.

Full details in the [LICENSE](https://github.com/mkbeh/arb-bot-rs/blob/main/LICENSE) file.

> "Code is poetry. Share it generously!" — inspired by Richard Stallman (with a twist 😉)
