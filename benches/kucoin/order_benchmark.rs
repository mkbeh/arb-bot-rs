use std::hint::black_box;

use arb_bot_rs::services::{
    enums::SymbolOrder,
    kucoin::{
        exchange::order::{OrderBuilder, OrderSymbol},
        storage::BookTickerEvent,
    },
};
use criterion::{Criterion, criterion_group};
use rust_decimal::{Decimal, prelude::FromPrimitive};

pub fn calculate_chain_profit_benchmark(c: &mut Criterion) {
    let order_book_1 = BookTickerEvent {
        sequence_id: 0,
        symbol: "BTCUSDT".to_string(),
        price: Decimal::from_f64(109615.46000000).unwrap(),
        qty: Decimal::from_f64(7.27795000).unwrap(),
    };

    let order_book_2 = BookTickerEvent {
        sequence_id: 0,
        symbol: "ETHUSDT".to_string(),
        price: Decimal::from_f64(2585.70000000).unwrap(),
        qty: Decimal::from_f64(14.64600000).unwrap(),
    };

    let order_book_3 = BookTickerEvent {
        sequence_id: 0,
        symbol: "ETHBTC".to_string(),
        price: Decimal::from_f64(0.02858000).unwrap(),
        qty: Decimal::from_f64(105.74550000).unwrap(),
    };

    let order_symbols = vec![
        OrderSymbol {
            symbol: "BTCUSDT".to_string(),
            symbol_order: SymbolOrder::Asc,
            min_profit_qty: Decimal::from_f64(0.000030),
            max_order_qty: Decimal::from_f64(0.00030),
            order_book: &order_book_1,
            base_min_size: Default::default(),
            quote_min_size: Default::default(),
            base_max_size: Default::default(),
            quote_max_size: Default::default(),
            base_increment: Decimal::from_f64(0.00000001).unwrap(),
            quote_increment: Decimal::from_f64(0.00000001).unwrap(),
            price_increment: Decimal::from_f64(0.00000001).unwrap(),
        },
        OrderSymbol {
            symbol: "ETHUSDT".to_string(),
            symbol_order: SymbolOrder::Desc,
            min_profit_qty: None,
            max_order_qty: None,
            order_book: &order_book_2,
            base_min_size: Default::default(),
            quote_min_size: Default::default(),
            base_max_size: Default::default(),
            quote_max_size: Default::default(),
            base_increment: Decimal::from_f64(0.00000001).unwrap(),
            quote_increment: Decimal::from_f64(0.00000001).unwrap(),
            price_increment: Decimal::from_f64(0.00000001).unwrap(),
        },
        OrderSymbol {
            symbol: "ETHBTC".to_string(),
            symbol_order: SymbolOrder::Asc,
            min_profit_qty: None,
            max_order_qty: None,
            order_book: &order_book_3,
            base_min_size: Default::default(),
            quote_min_size: Default::default(),
            base_max_size: Default::default(),
            quote_max_size: Default::default(),
            base_increment: Decimal::from_f64(0.00000001).unwrap(),
            quote_increment: Decimal::from_f64(0.00000001).unwrap(),
            price_increment: Decimal::from_f64(0.00000001).unwrap(),
        },
    ];

    let market_depth_limit: usize = 1;
    let fee_percent: Decimal = Decimal::from_f64(0.075).unwrap();

    c.bench_function("kucoin::calculate_chain_profit", |b| {
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
