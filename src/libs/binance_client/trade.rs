use std::time::SystemTime;

use crate::libs::binance_client::{
    SendOrderRequest, SendOrderResponse,
    api::{Api, Spot},
    client::Client,
    utils,
};

#[derive(Clone)]
pub struct Trade {
    pub client: Client,
}

impl Trade {
    /// Send in a new order.
    pub async fn send_order(&self, request: SendOrderRequest) -> anyhow::Result<SendOrderResponse> {
        const ARGS_NUM: usize = 15;

        let mut params: Vec<(String, String)> = Vec::with_capacity(ARGS_NUM);

        params.push(("symbol".to_owned(), request.symbol));

        let ts = utils::get_timestamp(SystemTime::now())?;
        params.push(("timestamp".to_owned(), ts.to_string()));

        let order_side = request.order_side.to_string();
        params.push(("side".to_owned(), order_side));

        let order_type = request.order_type.to_string();
        params.push(("type".to_owned(), order_type));

        if let Some(v) = request.quantity {
            params.push(("quantity".to_owned(), v.to_string()));
        }

        if let Some(v) = request.quote_order_qty {
            params.push(("quoteOrderQty".to_owned(), v.to_string()));
        }

        if let Some(v) = request.price {
            params.push(("price".to_owned(), v.to_string()));
        }

        if let Some(v) = request.new_client_order_id {
            params.push(("newClientOrderId".to_owned(), v));
        }

        if let Some(v) = request.strategy_id {
            params.push(("strategyId".to_owned(), v.to_string()));
        }

        if let Some(v) = request.strategy_type {
            params.push(("strategyType".to_owned(), v.to_string()));
        }

        if let Some(v) = request.stop_price {
            params.push(("stopPrice".to_owned(), v.to_string()));
        }

        if let Some(v) = request.trailing_delta {
            params.push(("trailingDelta".to_owned(), v.to_string()));
        }

        if let Some(v) = request.iceberg_qty {
            params.push(("icebergQty".to_owned(), v.to_string()));
        }

        if let Some(v) = request.new_order_resp_type {
            params.push(("newOrderRespType".to_owned(), v.to_string()));
        }

        if let Some(v) = request.self_trade_prevention_mode {
            params.push(("selfTradePreventionMode".to_owned(), v.to_string()));
        }

        if let Some(v) = request.recv_window {
            params.push(("recvWindow".to_owned(), v.to_string()));
        }

        if let Some(ref v) = request.time_in_force {
            params.push(("timeInForce".to_owned(), v.to_string()));
        }

        self.client
            .post(Api::Spot(Spot::Order), Some(&params), true)
            .await
    }
}
