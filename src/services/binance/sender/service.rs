use async_trait::async_trait;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
    libs::binance_api::{Account, OrderSide, OrderType, SendOrderRequest, TimeInForce, Trade},
    services::{Order, enums::SymbolOrder, service::OrderSenderService},
};

pub struct BinanceSenderConfig {
    pub account_api: Account,
    pub trade_api: Trade,
    pub send_orders: bool,
}

pub struct BinanceSender {
    pub account_api: Account,
    pub trade_api: Trade,
    pub send_orders: bool,
}

impl BinanceSender {
    pub fn new(config: BinanceSenderConfig) -> BinanceSender {
        BinanceSender {
            account_api: config.account_api,
            trade_api: config.trade_api,
            send_orders: config.send_orders,
        }
    }
}

#[async_trait]
impl OrderSenderService for BinanceSender {
    async fn send_orders(&self, msg: Vec<Order>) -> anyhow::Result<()> {
        let define_order_side = |order: &Order| -> OrderSide {
            match order.symbol_order {
                SymbolOrder::Asc => OrderSide::Sell,
                SymbolOrder::Desc => OrderSide::Buy,
            }
        };

        let chain_id = Uuid::new_v4();
        info!(chain_id = ?chain_id, orders = ?msg, send_orders = ?self.send_orders, "sending orders");

        if !self.send_orders {
            return Ok(());
        };

        tokio::spawn({
            let trade_api = self.trade_api.clone();

            async move {
                for order in msg.iter() {
                    let mut request = SendOrderRequest {
                        symbol: order.symbol.to_owned(),
                        order_side: define_order_side(order),
                        order_type: OrderType::Market,
                        time_in_force: Some(TimeInForce::Fok),
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
                    };

                    match order.symbol_order {
                        SymbolOrder::Asc => request.quantity = Some(order.base_qty),
                        SymbolOrder::Desc => request.quote_order_qty = Some(order.base_qty),
                    }

                    debug!(chain_id = ?chain_id, order = ?order, request = ?request, "sending
        order request");

                    match trade_api.send_order(request).await {
                        Ok(response) => {
                            info!(chain_id = ?chain_id, order = ?order, response = ?response,
        "successfully send order")
                        }
                        Err(e) => {
                            error!(chain_id = ?chain_id, order = ?order, error = ?e, "error
        during send order");
                            break;
                        }
                    };
                }
            }
        });

        Ok(())
    }
}
