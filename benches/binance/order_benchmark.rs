use std::{hint::black_box, str::FromStr};

use arb_bot_rs::services::{
    binance::{
        exchange::order::{OrderBuilder, OrderSymbol, SymbolFilter},
        storage::BookTickerEvent,
    },
    enums::SymbolOrder,
};
use criterion::{Criterion, criterion_group};
use rust_decimal::{Decimal, prelude::FromPrimitive};

pub fn calculate_chain_profit_benchmark(c: &mut Criterion) {
    let order_book_1 = BookTickerEvent {
        update_id: 1,
        symbol: "BTCUSDT".to_string(),
        best_bid_price: Decimal::from_f64(109615.46000000).unwrap(),
        best_bid_qty: Decimal::from_f64(7.27795000).unwrap(),
        best_ask_price: Decimal::from_f64(109615.47000000).unwrap(),
        best_ask_qty: Decimal::from_f64(2.22969000).unwrap(),
    };

    let order_book_2 = BookTickerEvent {
        update_id: 1,
        symbol: "ETHUSDT".to_string(),
        best_bid_price: Decimal::from_f64(2585.70000000).unwrap(),
        best_bid_qty: Decimal::from_f64(14.64600000).unwrap(),
        best_ask_price: Decimal::from_f64(2585.71000000).unwrap(),
        best_ask_qty: Decimal::from_f64(19.28810000).unwrap(),
    };

    let order_book_3 = BookTickerEvent {
        update_id: 1,
        symbol: "ETHBTC".to_string(),
        best_bid_price: Decimal::from_f64(0.02858000).unwrap(),
        best_bid_qty: Decimal::from_f64(105.74550000).unwrap(),
        best_ask_price: Decimal::from_f64(0.02359000).unwrap(),
        best_ask_qty: Decimal::from_f64(25.63400000).unwrap(),
    };

    let order_symbols = vec![
        OrderSymbol {
            symbol: "BTCUSDT".to_string(),
            base_asset_precision: 8,
            quote_precision: 8,
            symbol_order: SymbolOrder::Asc,
            min_profit_qty: Decimal::from_f64(0.000030),
            max_order_qty: Decimal::from_f64(0.00030),
            order_book: &order_book_1,
            symbol_filter: SymbolFilter {
                lot_size_step: 5,
                tick_size: 2,
                lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
            },
        },
        OrderSymbol {
            symbol: "ETHUSDT".to_string(),
            base_asset_precision: 8,
            quote_precision: 8,
            symbol_order: SymbolOrder::Desc,
            min_profit_qty: None,
            max_order_qty: None,
            order_book: &order_book_2,
            symbol_filter: SymbolFilter {
                lot_size_step: 4,
                tick_size: 2,
                lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
            },
        },
        OrderSymbol {
            symbol: "ETHBTC".to_string(),
            base_asset_precision: 8,
            quote_precision: 8,
            symbol_order: SymbolOrder::Asc,
            min_profit_qty: None,
            max_order_qty: None,
            order_book: &order_book_3,
            symbol_filter: SymbolFilter {
                lot_size_step: 4,
                tick_size: 5,
                lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
            },
        },
    ];

    let market_depth_limit: usize = 1;
    let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

    c.bench_function("calculate_chain_profit", |b| {
        b.iter(|| {
            OrderBuilder::calculate_chain_profit(
                black_box(&order_symbols),
                black_box(market_depth_limit),
                black_box(fee_percent),
            )
        })
    });
}

criterion_group!(benches, calculate_chain_profit_benchmark);
