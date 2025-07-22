# arb-bot-rs

Cryptocurrency exchanges arbitrage bot.

Documentation of arbitrage algorithm [here](https://github.com/mkbeh/arb-bot-rs/tree/main/docs).

**Support exchanges**

* [Binance](https://www.binance.com)

**Content**

* [Configuration](#configuration)
* [Usage](#usage)

## Configuration

[TODO] config description

## Usage

[TODO] how to

[TODO] from binary

[TODO] docker 

```shell
docker build --build-arg SERVICE_NAME=bot -t arb-bot-rs:latest .
```

```shell
docker run --memory="50m" arb-bot-rs:latest
```

## Safety

This application uses `#![forbid(unsafe_code)]` to ensure everything is implemented in 100% safe Rust.

## Roadmap

| Exchange | Status  |
|:--------:|:-------:|
| Binance  | &check; |