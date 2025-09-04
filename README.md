# arb-bot-rs

![GitHub CI](https://github.com/mkbeh/arb-bot-rs/actions/workflows/ci.yml/badge.svg)

Arbitrage bot is a high-frequency arbitrage trading system that automatically identifies and executes profitable
triangular arbitrage opportunities on cryptocurrency exchanges.

Full description of the algorithm used can be found [here](https://github.com/mkbeh/arb-bot-rs/tree/main/docs).

### Supports

List of supported cryptocurrency exchanges.

| Exchange | Status  |
|:--------:|:-------:|
| Binance  | &check; |

### Content

* [System overview](#system-overview)
* [Installation](#installation)
* [Usage](#usage)
* [Translations](#translations)

## System overview

### 📊 Architecture Levels

#### Level 1 - Context

* **Core System:** Arbitrage Bot
* **External Dependencies:** Cryptocurrency Exchange (REST+WebSocket), Configuration File
* **Primary Flow:** Real-time market data → Arbitrage detection → Order execution

#### Level 2 - Containers

* **Main Process:** Orchestrates all components
* **Cryptocurrency API Client:** Handles REST API communications
* **WebSocket Client:** Manages real-time data streams
* **Config Manager:** Processes configuration settings
* **HTTP Server:** Provides health monitoring endpoints

#### Level 3 - Components

* **Arbitrage Job:** Core arbitrage detection logic
* **Order Sender Job:** Order execution management
* **Exchange Service:** Market data processing
* **Sender Service:** Risk-managed order execution
* **Orders Channel:** Pub/Sub communication bus

#### Level 4 - Code (Arbitrage Job)

* **Ticker Builder:** Processes real-time price data from WebSocket
* **Chain Builder:** Identifies arbitrage chains using symbol information
* **Profit Calculator:** Validates profitability of arbitrage opportunities
* **Order Builder:** Generates executable orders (uses Ticker Builder for prices, Chain Builder for chains, and Profit
  Calculator for validation)

### ⚡ Key Data Flows

* **Market Data:** WebSocket → Ticker Builder → Order Builder
* **Arbitrage Detection:** Chain analysis → Profit validation → Order generation
* **Order Execution:** Orders Channel → Sender Service → Cryptocurrency Exchange API
* **Monitoring:** Continuous status checks → Performance metrics

### 🛡️ Quality Attributes

* **Performance:** Low-latency real-time processing
* **Reliability:** Automatic reconnections and error handling
* **Maintainability:** Modular design with clear separation of concerns
* **Monitorability:** Comprehensive logging and metrics

### 🎪 Deployment

* **Single Container:** Docker-based deployment
* **External Dependencies:** Cryptocurrency Exchange API endpoints
* **Monitoring:** Integrated health checks and metrics

### Context Diagram (Level 1)

```mermaid
flowchart TD
    subgraph ExternalSystems[External Systems]
        CryptoExchange[Crypto Exchange<br/>REST API & WebSocket]
        ConfigFile[Configuration File<br/>config.toml]
    end

    subgraph ArbBotRS[Bot System]
        ArbitrageBot[Arbitrage Bot]
    end

    ArbitrageBot -->|REST API requests| CryptoExchange
    ArbitrageBot -->|WebSocket connections| CryptoExchange
    ArbitrageBot -->|Reads configuration| ConfigFile
    style ArbBotRS fill: #e1f5fe, color: #000000
    style ExternalSystems fill: #f3e5f5, color: #000000
    style ArbitrageBot fill: #c8e6c9, color: #000000
    style CryptoExchange fill: #ffcdd2, color: #000000
    style ConfigFile fill: #d7ccc8, color: #000000
```

### Container Diagram (Level 2)

```mermaid
flowchart TB
    subgraph ArbBotRS[Bot]
        MainProcess[Main Process]
        CryptoAPIClient[Crypto Exchange API Client<br/>HTTP/REST client]
        WSClient[WebSocket Client]
        ConfigManager[Config Manager]
        HTTPServer[HTTP Server<br/>Monitoring server]
        MainProcess -->|manages| CryptoAPIClient
        MainProcess -->|manages| WSClient
        MainProcess -->|uses| ConfigManager
        MainProcess -->|starts| HTTPServer
    end

    subgraph ExternalSystems[External Systems]
        Crypto[Crypto Exchange]
        ConfigFile[config.toml file]
    end

    CryptoAPIClient -->|REST API| Crypto
    WSClient -->|WebSocket| Crypto
    ConfigManager -->|reads| ConfigFile
    style MainProcess fill: #bbdefb, color: #000000
    style CryptoAPIClient fill: #c8e6c9, color: #000000
    style WSClient fill: #ffecb3, color: #000000
    style ConfigManager fill: #ffcdd2, color: #000000
    style HTTPServer fill: #d7ccc8, color: #000000
    style Crypto fill: #e1f5fe, color: #000000
    style ConfigFile fill: #f3e5f5, color: #000000
```

### Component Diagram (Level 3)

```mermaid
flowchart TB
    subgraph MainProcess[Main Process]
        Entrypoint[Entrypoint]
        JobScheduler[Job Scheduler]
        ServicesManager[Services Manager]
        Entrypoint -->|initializes| JobScheduler
        Entrypoint -->|initializes| ServicesManager
    end

    subgraph Jobs[Jobs]
        ArbitrageJob[Arbitrage Job]
        OrderSenderJob[Order Sender Job]
    end

    subgraph Services[Services]
        ExchangeService[Exchange Service]
        SenderService[Sender Service]
    end

    subgraph Communication[Communication]
        OrdersChannel[Orders Channel]
    end

    JobScheduler -->|starts| ArbitrageJob
    JobScheduler -->|starts| OrderSenderJob
    ServicesManager -->|manages| ExchangeService
    ServicesManager -->|manages| SenderService
    ArbitrageJob -->|publishes to| OrdersChannel
    ArbitrageJob -->|uses| ExchangeService
    OrderSenderJob -->|subscribes to| OrdersChannel
    OrderSenderJob -->|uses| SenderService
    style Entrypoint fill: #e1f5fe, color: #000000
    style ArbitrageJob fill: #c8e6c9, color: #000000
    style OrderSenderJob fill: #ffecb3, color: #000000
    style ExchangeService fill: #ffcdd2, color: #000000
    style SenderService fill: #d1c4e9, color: #000000
    style OrdersChannel fill: #f3e5f5, color: #000000
    style JobScheduler fill: #bbdefb, color: #000000
    style ServicesManager fill: #c8e6c9, color: #000000
```

### Arbitrage Job Component Diagram (Level 4)

```mermaid
flowchart TB
    subgraph ArbitrageJob[Arbitrage Job]
        TickerBuilder[Ticker Builder]
        ChainBuilder[Chain Builder]
        OrderBuilder[Order Builder]
        ProfitCalculator[Profit Calculator]
        WSStreams[WebSocket Streams]
        SymbolChains[Symbol Chains Generator]
    %% Corrected connections based on your feedback
        TickerBuilder -->|uses| WSStreams
        ChainBuilder -->|uses| SymbolChains
        OrderBuilder -->|uses| TickerBuilder
        OrderBuilder -->|uses| ChainBuilder
        OrderBuilder -->|uses| ProfitCalculator
    end

    subgraph External[External Components]
        Broadcast[Ticker Broadcast]
        OrdersChan[Orders Channel]
    end

    WSStreams -->|subscribes to| Broadcast
    OrderBuilder -->|publishes to| OrdersChan
    style TickerBuilder fill: #bbdefb, color: #000000
    style ChainBuilder fill: #c8e6c9, color: #000000
    style OrderBuilder fill: #ffecb3, color: #000000
    style ProfitCalculator fill: #e1f5fe, color: #000000
    style WSStreams fill: #ffcdd2, color: #000000
    style SymbolChains fill: #d7ccc8, color: #000000
    style Broadcast fill: #f3e5f5, color: #000000
    style OrdersChan fill: #d1c4e9, color: #000000
```

### Arbitrage Operation Sequence Diagram

```mermaid
sequenceDiagram
    participant W as WebSocket
    participant T as Ticker Builder
    participant C as Chain Builder
    participant P as Profit Calculator
    participant O as Order Builder
    participant S as Order Sender
    Note over W, S: Arbitrage Opportunity Detection
    W ->> T: Market data (realtime)
    T ->> C: Updated tickers
    C ->> P: Calculate chain profitability
    P ->> O: Profitable orders
    Note over W, S: Arbitrage Operation Execution
    O ->> S: Send orders via channel
    S ->> S: Send orders to Exchange API
    S ->> S: Check execution
```

### Technology Stack

```mermaid
quadrantChart
    title "Application Technology Stack"
    x-axis "Low-level Infrastructure" --> "High-level Application"
    y-axis "Foundation" --> "Specialized"
    "Rust": [0.2, 0.8]
    "Tokio": [0.3, 0.7]
    "Reqwest": [0.4, 0.6]
    "Serde": [0.5, 0.5]
    "Axum": [0.6, 0.4]
    "Tracing": [0.7, 0.3]
    "WebSocket": [0.8, 0.2]
    "Dashmap": [0.4, 0.4]
    "Redis": [0.3, 0.3]
```

This architecture enables high-frequency arbitrage trading with robust error handling, real-time processing, and
comprehensive monitoring capabilities while maintaining modularity and scalability.

## Installation

Application is written in Rust, so you'll need to grab a
[Rust installation](https://www.rust-lang.org/) in order to compile it.
Application compiles with Rust 1.88.0 (stable) or newer.

### Build from source

```shell
git clone https://github.com/mkbeh/arb-bot-rs.git
cd arb-bot-rs
cargo build --release
```

## Usage

Fill in the [example](https://github.com/mkbeh/arb-bot-rs/blob/main/config.example.toml) config file and rename the
file to `config.toml`.

_For a test run, you do not need to specify your API tokens. You only need to specify API tokens if you toggle the flag
`send_orders = true`._

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
docker run --cpus="1" --cpuset-cpus="0" --memory="256m" arb-bot-rs:latest
```

## Running tests

Application is relatively well-tested, including both unit tests and integration tests. To run the full test suite, use:

```shell
cargo test --all
```

## Translations

The following is a list of known translations of application documentation.

* [English](https://github.com/mkbeh/arb-bot-rs/tree/main/docs/en)
