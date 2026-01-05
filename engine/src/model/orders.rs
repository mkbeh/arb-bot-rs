use std::fmt::{Display, Formatter};

use rust_decimal::{Decimal, RoundingStrategy, prelude::FromPrimitive};
use tracing::info;
use uuid::Uuid;

use crate::enums::SymbolOrder;

/// Chain of orders for arbitrage (buy/sell sequence).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChainOrders {
    pub ts: u128,
    pub chain_id: Uuid,
    pub fee_percent: Decimal,
    pub orders: Vec<ChainOrder>,
}

impl Display for ChainOrders {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (profit, profit_percent) = self.compute_profit();
        let orders_fmt = self
            .orders
            .iter()
            .map(|o| {
                format!(
                    "{}(base:{:.8}@quote:{:.8}@price:{:.8})",
                    o.symbol, o.base_qty, o.quote_qty, o.price
                )
            })
            .collect::<Vec<_>>()
            .join(" → ");

        write!(
            f,
            "Profit: {profit} ({profit_percent}%), Chain: {orders_fmt}"
        )
    }
}

impl ChainOrders {
    /// Extracts symbols from orders.
    #[must_use]
    pub fn extract_symbols(&self) -> Vec<&str> {
        self.orders.iter().map(|o| o.symbol.as_str()).collect()
    }

    /// Calculates the chain's profit taking into account the fee.
    #[must_use]
    pub fn compute_profit(&self) -> (Decimal, Decimal) {
        if self.orders.is_empty() {
            return (Decimal::ZERO, Decimal::ZERO);
        }

        let input_qty = self.orders.first().unwrap().base_qty;
        let output_qty = self.orders.last().unwrap().quote_qty;

        let hundred = Decimal::from_u8(100).unwrap();
        let fee_rate = self.fee_percent / hundred;
        let scale_factor = Decimal::from_usize(self.orders.len()).unwrap_or(Decimal::ONE);

        let fee = (scale_factor * (input_qty * fee_rate))
            .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero);

        let profit = (output_qty - input_qty - fee)
            .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero);

        let profit_percent = if input_qty.is_zero() {
            Decimal::ZERO
        } else {
            ((profit / input_qty) * hundred)
                .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
        };

        (profit, profit_percent)
    }

    /// Logs information about the chain.
    pub fn print_info(&self, send_orders: bool) {
        info!(
            ts = self.ts,
            chain_id = %self.chain_id,
            send_orders,
            details = %self,
            "📦 [Engine] Chain processed"
        );
    }
}

/// Order in a chain (buy/sell with qty/price).
#[derive(Clone, Debug, PartialEq)]
pub struct ChainOrder {
    pub symbol: String,
    pub symbol_order: SymbolOrder,
    pub price: Decimal,
    pub base_qty: Decimal,
    pub quote_qty: Decimal,
    pub base_increment: Decimal,
    pub quote_increment: Decimal,
}
