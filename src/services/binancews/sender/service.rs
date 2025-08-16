use std::sync::{Arc, atomic::AtomicBool};

use anyhow::bail;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt, future::err, task::SpawnExt};
use serde_json::Value;
use tokio::{sync::Mutex, task::JoinSet};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info};
use tracing_subscriber::fmt::format;
use crate::{
    libs::{
        binance_api::{
            Account, OrderSide, OrderType, SendOrderRequest, TimeInForce, Trade,
            ws::{PlaceOrderRequest, WebsocketClient, WebsocketClientConfig},
        },
        misc,
    },
    services::{Chain, ORDERS_CHANNEL, Order, enums::SymbolOrder, service::OrderSenderService},
};

pub struct BinanceWsSenderConfig {
    pub account_api: Account,
    pub trade_api: Trade,
    pub send_orders: bool,
}

pub struct BinanceWsSender {
    pub account_api: Account,
    pub trade_api: Trade,
    pub send_orders: bool,
}

impl BinanceWsSender {
    pub fn new(config: BinanceWsSenderConfig) -> BinanceWsSender {
        BinanceWsSender {
            account_api: config.account_api,
            trade_api: config.trade_api,
            send_orders: config.send_orders,
        }
    }
}

#[async_trait]
impl OrderSenderService for BinanceWsSender {
    async fn send_orders(&self, msg: Chain) -> anyhow::Result<()> {
        Ok(())
    }

    async fn send_orders_ws(&self) -> anyhow::Result<()> {
        let define_order_side = |order: &Order| -> OrderSide {
            match order.symbol_order {
                SymbolOrder::Asc => OrderSide::Sell,
                SymbolOrder::Desc => OrderSide::Buy,
            }
        };

        let ws_conf = WebsocketClientConfig {
            ws_url: "wss://ws-api.binance.com:443/ws-api/v3".to_string(),
            api_key: "qdxCjbI7Ezl79SK6gaUZMlvsXB2xzOK8CpGLQ5cW1zk1fIVoqpewHakIyETl02Qj".to_string(),
            secret_key: "Nt8W1MVC5yIvSpFk1mM7IEo4uBCAuXNIxz13Wa72fPqPkfYo33UmfQ70aSBD4tOg"
                .to_string(),
        };
        let mut ws = WebsocketClient::new(ws_conf.clone());

        let mut stream = match connect_async(ws_conf.ws_url.clone()).await {
            Ok((ws_stream, _)) => ws_stream,
            Err(e) => bail!("Failed to connect to websocket: {}", e),
        };

        let mut orders_rx = ORDERS_CHANNEL.rx.lock().await;

        let mut test_flag = AtomicBool::new(false);

        loop {
            tokio::select! {
                msg = orders_rx.recv() => {
                    if let Some(msg) = msg {
                        info!(msg = ?msg, send_orders = ?self.send_orders, "sending orders");

                        if !self.send_orders {
                            continue;
                        };

                        if test_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            println!("skipped order");
                            continue;
                        }

                        for order in msg.orders.iter() {
                            let mut request = PlaceOrderRequest {
                                symbol: order.symbol.to_owned(),
                                order_side: define_order_side(order),
                                order_type: OrderType::Market,
                                // time_in_force: Some(TimeInForce::Fok),
                                time_in_force: None,
                                quantity: None,
                                quote_order_qty: None,
                                price: None,
                                new_client_order_id: None,
                                strategy_id: None,
                                strategy_type: None,
                                stop_price: None,
                                trailing_delta: None,
                                iceberg_qty: None,
                                new_order_resp_type: None,
                                self_trade_prevention_mode: None,
                                recv_window: None,
                                timestamp: None,
                                api_key: None,
                                signature: None,
                            };

                            match order.symbol_order {
                                SymbolOrder::Asc => request.quantity = Some(order.base_qty.to_string()),
                                SymbolOrder::Desc => request.quantity = Some(order.quote_qty.to_string()),
                            }

                            let s = ws.place_order(request).await?;

                            if let Err(e) = stream.send(s.into()).await {
                                error!(err = ?e, "Failed placing order");
                                break;
                            }
                        }

                        test_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                msg = stream.next() => {
                    if let Some(result) = msg {
                        let msg = match result {
                            Ok(msg) => msg,
                            Err(e) => {
                                error!(err = ?e, "Received error");
                                continue;
                            }
                        };

                        match msg {
                            Message::Text(msg) => match serde_json::from_str::<Value>(&msg) {
                                Ok(json) => {
                                    info!("Received: {}", json);
                                }
                                Err(e) => {
                                    error!("Parse error: {}", e);
                                    return Err(anyhow::anyhow!("Parse error: {}", e));
                                }
                            },
                            Message::Ping(ping) => {
                                if let Err(e) = stream.send(Message::Pong(ping.into())).await {
                                    error!("Pong failed: {}", e);
                                    return Err(anyhow::anyhow!("Pong failed: {e}"));
                                };
                            }
                            Message::Close(_) => {
                                info!("Connection closed");
                                return Err(anyhow::anyhow!("Connection closed"));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
